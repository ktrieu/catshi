use sqlx::{Executor, QueryBuilder, Sqlite, query, query_as};

use crate::store::market::Market;

#[derive(Debug, sqlx::Type, Clone, Copy, PartialEq, Eq)]
#[sqlx(rename_all = "lowercase")]
pub enum InstrumentState {
    Open,
    Winner,
    Loser,
}

#[derive(Debug, sqlx::FromRow, Clone)]
#[allow(dead_code)]
pub struct Instrument {
    pub id: i64,
    pub name: String,
    pub state: InstrumentState,
    pub market_id: i64,
}

pub async fn insert_market_instruments(
    exec: impl Executor<'_, Database = Sqlite>,
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

    let query = builder.build_query_as::<Instrument>();

    let rows = query.fetch_all(exec).await?;

    Ok(rows)
}

pub async fn get_instrument_by_id(
    exec: impl Executor<'_, Database = Sqlite>,
    id: i64,
) -> anyhow::Result<Option<Instrument>> {
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
    .fetch_optional(exec)
    .await?;

    Ok(instrument)
}

pub async fn set_instrument_state(
    exec: impl Executor<'_, Database = Sqlite>,
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
    .execute(exec)
    .await?;

    Ok(())
}

pub type InstrumentWithShares = (Instrument, i64);

pub async fn get_instruments_with_share_counts_for_market(
    exec: impl Executor<'_, Database = Sqlite>,
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
    .fetch_all(exec)
    .await?;

    Ok(rows
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
        .collect())
}
