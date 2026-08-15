use sqlx::{QueryBuilder, query, query_as};
use trait_variant::make;

use crate::store::{DbExecutor, log_pg_compare_result, log_pg_write_err, market::Market};

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
            b.push_bind(name);
            b.push_bind(InstrumentState::Open);
            b.push_bind(market.id);
        });

        builder.push(" RETURNING *");

        let instruments = builder
            .build_query_as::<Instrument>()
            .fetch_all(db.sqlite())
            .await?;

        // Mirror the insert to postgres, forcing `id` to match the sqlite rows since
        // `positions` references `instruments(id)` by foreign key.
        let mut pg_builder = QueryBuilder::new(
            "INSERT INTO instruments (id, name, state, market_id) OVERRIDING SYSTEM VALUE ",
        );

        pg_builder.push_values(instruments.iter(), |mut b, instrument| {
            b.push_bind(instrument.id);
            b.push_bind(&instrument.name);
            b.push_bind(instrument.state);
            b.push_bind(instrument.market_id);
        });

        pg_builder.push(
            " RETURNING CAST(id AS BIGINT) as id, name, state, CAST(market_id AS BIGINT) as market_id",
        );

        let pg_result: sqlx::Result<Vec<Instrument>> = pg_builder
            .build_query_as::<Instrument>()
            .fetch_all(db.psql())
            .await;

        log_pg_compare_result(
            pg_result,
            &instruments,
            "instrument insert_market_instruments",
        );

        Ok(instruments)
    }

    async fn get_instrument_by_id(
        &self,
        db: &mut impl DbExecutor,
        id: i64,
    ) -> anyhow::Result<Instrument> {
        let instrument = query_as!(
            Instrument,
            r#"
                SELECT
                    id,
                    name,
                    state as "state: InstrumentState",
                    market_id
                FROM
                    instruments
                WHERE id = $1
            "#,
            id
        )
        .fetch_one(db.sqlite())
        .await?;

        let pg_instrument = query_as(
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
        .await;

        log_pg_compare_result(
            pg_instrument,
            &instrument,
            "instrument get_instrument_by_id",
        );

        Ok(instrument)
    }

    async fn set_instrument_state(
        &self,
        db: &mut impl DbExecutor,
        instrument: &Instrument,
        state: InstrumentState,
    ) -> anyhow::Result<()> {
        query!(
            r#"
            UPDATE
                instruments
            SET
                state = $1
            WHERE
                id = $2
            "#,
            state,
            instrument.id
        )
        .execute(db.sqlite())
        .await?;

        let pg_result = query(
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
        .await;

        log_pg_write_err(pg_result, "instrument set_instrument_state");

        Ok(())
    }

    async fn get_instruments_with_share_counts_for_market(
        &self,
        db: &mut impl DbExecutor,
        market_id: i64,
    ) -> anyhow::Result<Vec<InstrumentWithShares>> {
        // Maybe one day we'll cache this data on the instrument but it seems fine for now?
        let rows = query!(
            r#"
                SELECT
                    instruments.id,
                    instruments.name,
                    instruments.state as "state: InstrumentState",
                    instruments.market_id,
                    COALESCE(SUM(quantity), 0) as shares
                FROM
                    instruments
                LEFT JOIN
                    positions ON instruments.id = positions.instrument_id
                WHERE
                    instruments.market_id = $1
                GROUP BY instruments.id
            "#,
            market_id,
        )
        .fetch_all(db.sqlite())
        .await?;

        let instruments: Vec<InstrumentWithShares> = rows
            .iter()
            .map(|r| {
                (
                    Instrument {
                        id: r.id,
                        name: r.name.clone(),
                        state: r.state,
                        market_id: r.market_id,
                    },
                    r.shares,
                )
            })
            .collect();

        let pg_rows: sqlx::Result<Vec<PgInstrumentWithSharesRow>> = query_as(
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
        .await;

        let pg_instruments: sqlx::Result<Vec<InstrumentWithShares>> = pg_rows.map(|rows| {
            rows.into_iter()
                .map(PgInstrumentWithSharesRow::into_pair)
                .collect()
        });

        log_pg_compare_result(
            pg_instruments,
            &instruments,
            "instrument get_instruments_with_share_counts_for_market",
        );

        Ok(instruments)
    }

    async fn get_all_open_instruments_with_share_counts(
        &self,
        db: &mut impl DbExecutor,
    ) -> anyhow::Result<Vec<InstrumentWithShares>> {
        // Maybe one day we'll cache this data on the instrument but it seems fine for now?
        let rows = query!(
            r#"
                SELECT
                    instruments.id,
                    instruments.name,
                    instruments.state as "state: InstrumentState",
                    instruments.market_id,
                    COALESCE(SUM(quantity), 0) as shares
                FROM
                    instruments
                    LEFT JOIN
                    positions ON instruments.id = positions.instrument_id
                WHERE instruments.state = $1
                GROUP BY instruments.id
            "#,
            InstrumentState::Open
        )
        .fetch_all(db.sqlite())
        .await?;

        let instruments: Vec<InstrumentWithShares> = rows
            .iter()
            .map(|r| {
                (
                    Instrument {
                        id: r.id,
                        name: r.name.clone(),
                        state: r.state,
                        market_id: r.market_id,
                    },
                    r.shares,
                )
            })
            .collect();

        let pg_rows: sqlx::Result<Vec<PgInstrumentWithSharesRow>> = query_as(
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
        .await;

        let pg_instruments: sqlx::Result<Vec<InstrumentWithShares>> = pg_rows.map(|rows| {
            rows.into_iter()
                .map(PgInstrumentWithSharesRow::into_pair)
                .collect()
        });

        log_pg_compare_result(
            pg_instruments,
            &instruments,
            "instrument get_all_open_instruments_with_share_counts",
        );

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
