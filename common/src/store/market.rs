use anyhow::anyhow;
use serenity::all::{GenericChannelId, MessageId, ThreadId};
use sqlx::{Executor, Sqlite, SqliteConnection, query, query_as};

use crate::store::{
    instrument::{self, InstrumentWithShares},
    user::{self, DbUser},
};

#[derive(Debug, sqlx::Type, Clone, Copy, PartialEq, Eq)]
#[sqlx(rename_all = "lowercase")]
pub enum MarketState {
    Open,
    Closed,
}

#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Market {
    pub id: i64,
    pub description: String,
    pub state: MarketState,
    pub owner_id: i64,
    pub message_id: Option<String>,
    pub channel_id: Option<String>,
    pub thread_id: Option<String>,
    pub details_msg_id: Option<String>,
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
                channel_id,
                markets.thread_id,
                markets.details_msg_id
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
    thread_id: ThreadId,
    details_msg_id: MessageId,
) -> anyhow::Result<()> {
    let message_id = message_id.to_string();
    let channel_id = channel_id.to_string();
    let thread_id = thread_id.to_string();
    let details_msg_id = details_msg_id.to_string();

    query!(
        "UPDATE markets SET message_id = $1, channel_id = $2, thread_id = $3, details_msg_id = $4 WHERE id = $5",
        message_id,
        channel_id,
        thread_id,
        details_msg_id,
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
            channel_id,
            markets.thread_id,
            markets.details_msg_id
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
            markets.channel_id,
            markets.thread_id,
            markets.details_msg_id
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

pub async fn get_markets_by_state(
    exec: &mut SqliteConnection,
    state: MarketState,
) -> anyhow::Result<Vec<Market>> {
    let markets = query_as!(
        Market,
        r#"
        SELECT
            markets.id, 
            markets.description, 
            markets.state as "state: MarketState", 
            markets.owner_id, 
            markets.message_id,
            markets.channel_id,
            markets.thread_id,
            markets.details_msg_id
        FROM
            markets
        WHERE
            state = $1
        ORDER BY id ASC
        "#,
        state
    )
    .fetch_all(exec)
    .await?;

    Ok(markets)
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

#[derive(Debug, Clone)]
pub struct FullMarket {
    pub row: Market,
    pub instruments: Vec<InstrumentWithShares>,
    pub owner: DbUser,
}

impl FullMarket {
    pub async fn new_from_instrument_id(
        conn: &mut SqliteConnection,
        id: i64,
    ) -> anyhow::Result<Self> {
        let row = get_market_by_instrument_id(&mut *conn, id).await?;

        let instruments =
            instrument::get_instruments_with_share_counts_for_market(&mut *conn, row.id).await?;

        let owner = user::get_user_by_id(&mut *conn, row.owner_id).await?;

        Ok(Self {
            row,
            instruments,
            owner,
        })
    }

    pub fn get_instrument(&self, id: i64) -> anyhow::Result<&InstrumentWithShares> {
        // We expect markets to have very few instruments - just linear search.
        self.instruments
            .iter()
            .find(|(i, _)| i.id == id)
            .ok_or(anyhow!(
                "instrument {} not found for market {}",
                id,
                self.row.id
            ))
    }
}
