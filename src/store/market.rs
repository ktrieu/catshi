use serenity::all::{GenericChannelId, MessageId};
use sqlx::{Executor, Sqlite, query, query_as};

use crate::store::user::DbUser;

#[derive(Debug, sqlx::Type, Clone, Copy, PartialEq, Eq)]
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

pub async fn get_market_by_id(
    exec: impl Executor<'_, Database = Sqlite>,
    id: i64,
) -> anyhow::Result<Market> {
    let market = query_as!(
        Market,
        r#"
        SELECT
             id, 
            description, 
            state as "state: MarketState", 
            owner_id, 
            message_id,
            channel_id
        FROM
            markets
        WHERE
            id = $1
        "#,
        id
    )
    .fetch_one(exec)
    .await?;

    Ok(market)
}

pub async fn get_market_by_instrument_id(
    exec: impl Executor<'_, Database = Sqlite>,
    instrument_id: i64,
) -> anyhow::Result<Market> {
    let market = query_as!(
        Market,
        r#"
        SELECT
            markets.id, 
            markets.description, 
            markets.state as "state: MarketState", 
            markets.owner_id, 
            markets.message_id,
            markets.channel_id
        FROM
            markets
        JOIN
            instruments ON instruments.market_id = markets.id
        WHERE
            instruments.id = $1
        "#,
        instrument_id
    )
    .fetch_one(exec)
    .await?;

    Ok(market)
}

pub async fn set_market_state(
    exec: impl Executor<'_, Database = Sqlite>,
    market: &Market,
    state: MarketState,
) -> anyhow::Result<()> {
    query!(
        r#"
        UPDATE
            markets
        SET
            state = $1
        WHERE
            id = $2
        "#,
        state,
        market.id,
    )
    .execute(exec)
    .await?;

    Ok(())
}
