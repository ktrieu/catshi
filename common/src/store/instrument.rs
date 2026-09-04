use sqlx::{QueryBuilder, query, query_as};
use trait_variant::make;

use crate::store::{DbExecutor, market::Market};

#[derive(Debug, sqlx::Type, Clone, Copy, PartialEq, Eq)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum InstrumentState {
    Open,
    Winner,
    Loser,
}

#[derive(Debug, sqlx::FromRow, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct Instrument {
    pub id: i64,
    pub name: String,
    pub state: InstrumentState,
    pub market_id: i64,
}

pub type InstrumentWithShares = (Instrument, i64);

#[make(Send)]
pub trait InstrumentStore {
    async fn insert_market_instruments(
        &self,
        db: &mut impl DbExecutor,
        market: &Market,
        names: &[&str],
    ) -> anyhow::Result<Vec<Instrument>>;

    async fn get_instrument_by_id(
        &self,
        db: &mut impl DbExecutor,
        id: i64,
    ) -> anyhow::Result<Instrument>;

    async fn set_instrument_state(
        &self,
        db: &mut impl DbExecutor,
        instrument: &Instrument,
        state: InstrumentState,
    ) -> anyhow::Result<()>;

    async fn get_instruments_with_share_counts_for_market(
        &self,
        db: &mut impl DbExecutor,
        market_id: i64,
    ) -> anyhow::Result<Vec<InstrumentWithShares>>;

    async fn get_all_open_instruments_with_share_counts(
        &self,
        db: &mut impl DbExecutor,
    ) -> anyhow::Result<Vec<InstrumentWithShares>>;
}

pub struct DbInstrumentStore {}

impl InstrumentStore for DbInstrumentStore {
    async fn insert_market_instruments(
        &self,
        db: &mut impl DbExecutor,
        market: &Market,
        names: &[&str],
    ) -> anyhow::Result<Vec<Instrument>> {
        let mut builder = QueryBuilder::new("INSERT INTO instruments (name, state, market_id) ");

        builder.push_values(names.iter(), |mut b, name| {
            b.push_bind(*name);
            b.push_bind(InstrumentState::Open);
            b.push_bind(market.id);
        });

        builder.push(
            " RETURNING CAST(id AS BIGINT) as id, name, state, CAST(market_id AS BIGINT) as market_id",
        );

        let instruments = builder
            .build_query_as::<Instrument>()
            .fetch_all(db.psql())
            .await?;

        Ok(instruments)
    }

    async fn get_instrument_by_id(
        &self,
        db: &mut impl DbExecutor,
        id: i64,
    ) -> anyhow::Result<Instrument> {
        let instrument = query_as(
            r#"
            SELECT
                CAST(id AS BIGINT) as id,
                name,
                state,
                CAST(market_id AS BIGINT) as market_id
            FROM
                instruments
            WHERE
                id = $1
            "#,
        )
        .bind(id)
        .fetch_one(db.psql())
        .await?;

        Ok(instrument)
    }

    async fn set_instrument_state(
        &self,
        db: &mut impl DbExecutor,
        instrument: &Instrument,
        state: InstrumentState,
    ) -> anyhow::Result<()> {
        query(
            r#"
            UPDATE
                instruments
            SET
                state = $1
            WHERE
                id = $2
            "#,
        )
        .bind(state)
        .bind(instrument.id)
        .execute(db.psql())
        .await?;

        Ok(())
    }

    async fn get_instruments_with_share_counts_for_market(
        &self,
        db: &mut impl DbExecutor,
        market_id: i64,
    ) -> anyhow::Result<Vec<InstrumentWithShares>> {
        // Maybe one day we'll cache this data on the instrument but it seems fine for now?
        let rows = query_as::<_, PgInstrumentWithSharesRow>(
            r#"
            SELECT
                CAST(instruments.id AS BIGINT) as id,
                instruments.name,
                instruments.state,
                CAST(instruments.market_id AS BIGINT) as market_id,
                COALESCE(SUM(quantity), 0) as shares
            FROM
                instruments
            LEFT JOIN
                positions ON instruments.id = positions.instrument_id
            WHERE
                instruments.market_id = $1
            GROUP BY instruments.id
            "#,
        )
        .bind(market_id)
        .fetch_all(db.psql())
        .await?;

        let instruments = rows
            .into_iter()
            .map(PgInstrumentWithSharesRow::into_pair)
            .collect();

        Ok(instruments)
    }

    async fn get_all_open_instruments_with_share_counts(
        &self,
        db: &mut impl DbExecutor,
    ) -> anyhow::Result<Vec<InstrumentWithShares>> {
        // Maybe one day we'll cache this data on the instrument but it seems fine for now?
        let rows = query_as::<_, PgInstrumentWithSharesRow>(
            r#"
            SELECT
                CAST(instruments.id AS BIGINT) as id,
                instruments.name,
                instruments.state,
                CAST(instruments.market_id AS BIGINT) as market_id,
                COALESCE(SUM(quantity), 0) as shares
            FROM
                instruments
                LEFT JOIN
                positions ON instruments.id = positions.instrument_id
            WHERE instruments.state = $1
            GROUP BY instruments.id
            "#,
        )
        .bind(InstrumentState::Open)
        .fetch_all(db.psql())
        .await?;

        let instruments = rows
            .into_iter()
            .map(PgInstrumentWithSharesRow::into_pair)
            .collect();

        Ok(instruments)
    }
}

#[derive(sqlx::FromRow)]
struct PgInstrumentWithSharesRow {
    id: i64,
    name: String,
    state: InstrumentState,
    market_id: i64,
    shares: i64,
}

impl PgInstrumentWithSharesRow {
    fn into_pair(self) -> InstrumentWithShares {
        (
            Instrument {
                id: self.id,
                name: self.name,
                state: self.state,
                market_id: self.market_id,
            },
            self.shares,
        )
    }
}
