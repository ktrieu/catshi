use std::collections::HashMap;

use serenity::all::{GenericChannelId, MessageId, UserId};
use sqlx::{Executor, QueryBuilder, Sqlite, query, query_as};

use crate::currency::Currency;

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
pub struct DbUser {
    id: i64,
    discord_id: String,
    name: String,
}

pub async fn insert_user_if_not_exists(
    exec: impl Executor<'_, Database = Sqlite>,
    discord_id: &str,
    name: &str,
) -> anyhow::Result<Option<DbUser>> {
    let user = query_as!(
        DbUser,
        "INSERT INTO users(discord_id, name) VALUES ($1, $2) ON CONFLICT (discord_id) DO NOTHING RETURNING *",
        discord_id,
        name
    )
    .fetch_optional(exec)
    .await?;

    Ok(user)
}

pub async fn get_user_by_discord_id(
    exec: impl Executor<'_, Database = Sqlite>,
    discord_id: &UserId,
) -> anyhow::Result<Option<DbUser>> {
    let discord_id = discord_id.to_string();

    let user = query_as!(
        DbUser,
        "SELECT * FROM users WHERE discord_id = $1",
        discord_id,
    )
    .fetch_optional(exec)
    .await?;

    Ok(user)
}

#[derive(Debug, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub enum MarketState {
    Open,
    Closed,
}

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Market {
    pub id: i64,
    pub description: String,
    pub state: MarketState,
    pub owner_id: i64,
    pub message_id: Option<String>,
    pub channel_id: Option<String>,
}

pub async fn create_new_market(
    exec: impl Executor<'_, Database = Sqlite>,
    description: &str,
    owner: &DbUser,
) -> anyhow::Result<Market> {
    let result = query_as!(
        Market,
        r#"
            INSERT INTO markets(
                description,
                state,
                owner_id
            ) 
            VALUES ($1, $2, $3) 
            RETURNING 
                id, 
                description, 
                state as "state: MarketState", 
                owner_id, 
                message_id,
                channel_id
        "#,
        description,
        MarketState::Open,
        owner.id
    )
    .fetch_one(exec)
    .await?;

    Ok(result)
}

pub async fn set_market_message_id(
    exec: impl Executor<'_, Database = Sqlite>,
    market_id: i64,
    message_id: MessageId,
    channel_id: GenericChannelId,
) -> anyhow::Result<()> {
    let message_id = message_id.to_string();
    let channel_id = channel_id.to_string();

    query!(
        "UPDATE markets SET message_id = $1, channel_id = $2 WHERE id = $3",
        message_id,
        channel_id,
        market_id,
    )
    .execute(exec)
    .await?;

    Ok(())
}

#[derive(Debug, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub enum InstrumentState {
    Open,
    Winner,
    Loser,
}

#[derive(Debug, sqlx::FromRow)]
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

pub async fn get_outstanding_shares_for_market(
    exec: impl Executor<'_, Database = Sqlite>,
    market_id: i64,
) -> anyhow::Result<HashMap<i64, i64>> {
    // Maybe one day we'll cache this data on the instrument but it seems fine for now?
    let rows = query!(
        r#"
            SELECT
                instruments.id,
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

    Ok(rows.iter().map(|r| (r.id, r.shares)).collect())
}

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Position {
    id: i64,
    quantity: i64,
    cost_basis: Currency,
    instrument_id: i64,
    owner_id: i64,
}

pub async fn get_user_position(
    exec: impl Executor<'_, Database = Sqlite>,
    instrument: &Instrument,
    owner: &DbUser,
) -> anyhow::Result<Option<Position>> {
    let position = query_as!(
        Position,
        r#"
        SELECT
            id,
            quantity,
            cost_basis,
            instrument_id,
            owner_id
        FROM positions
        WHERE
            instrument_id = $1 AND owner_id = $2
        "#,
        instrument.id,
        owner.id
    )
    .fetch_optional(exec)
    .await?;

    Ok(position)
}

pub async fn create_new_position(
    exec: impl Executor<'_, Database = Sqlite>,
    quantity: i64,
    cost_basis: Currency,
    instrument: &Instrument,
    owner: &DbUser,
) -> anyhow::Result<Position> {
    let position = query_as!(
        Position,
        r#"
            INSERT INTO positions (
                quantity,
                cost_basis,
                instrument_id,
                owner_id
            ) VALUES ($1, $2, $3, $4)
            RETURNING *
        "#,
        quantity,
        cost_basis,
        instrument.id,
        owner.id
    )
    .fetch_one(exec)
    .await?;

    Ok(position)
}

pub async fn increase_position(
    exec: impl Executor<'_, Database = Sqlite>,
    quantity: i64,
    price_paid: Currency,
    instrument: &Instrument,
    owner: &DbUser,
) -> anyhow::Result<Position> {
    let position = query_as!(
        Position,
        r#"
            UPDATE positions
            SET
                quantity = quantity + $1,
                cost_basis = cost_basis + $2
            WHERE
                instrument_id = $3 AND owner_id = $4
            RETURNING *
        "#,
        quantity,
        price_paid,
        instrument.id,
        owner.id
    )
    .fetch_one(exec)
    .await?;

    Ok(position)
}
