use anyhow::anyhow;
use serenity::all::{GenericChannelId, MessageId, ThreadId};
use sqlx::{query, query_as};
use trait_variant::make;

use crate::store::{
    DbExecutor,
    instrument::{InstrumentStore, InstrumentWithShares},
    user::{DbUser, UserStore},
};

#[derive(Debug, sqlx::Type, Clone, Copy, PartialEq, Eq)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum MarketState {
    Open,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
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

#[make(Send)]
pub trait MarketStore {
    async fn create_new_market(
        &self,
        db: &mut impl DbExecutor,
        description: &str,
        owner: &DbUser,
    ) -> anyhow::Result<Market>;

    async fn set_market_message_id(
        &self,
        db: &mut impl DbExecutor,
        market_id: i64,
        message_id: MessageId,
        channel_id: GenericChannelId,
        thread_id: ThreadId,
        details_msg_id: MessageId,
    ) -> anyhow::Result<()>;

    async fn get_market_by_id(&self, db: &mut impl DbExecutor, id: i64) -> anyhow::Result<Market>;

    async fn get_market_by_instrument_id(
        &self,
        db: &mut impl DbExecutor,
        instrument_id: i64,
    ) -> anyhow::Result<Market>;

    async fn get_markets_by_state(
        &self,
        db: &mut impl DbExecutor,
        state: MarketState,
    ) -> anyhow::Result<Vec<Market>>;

    async fn set_market_state(
        &self,
        db: &mut impl DbExecutor,
        market: &Market,
        state: MarketState,
    ) -> anyhow::Result<()>;
}

pub struct DbMarketStore {}

impl MarketStore for DbMarketStore {
    async fn create_new_market(
        &self,
        db: &mut impl DbExecutor,
        description: &str,
        owner: &DbUser,
    ) -> anyhow::Result<Market> {
        let market = query_as(
            r#"
            INSERT INTO markets (
                description,
                state,
                owner_id
            )
            VALUES ($1, $2, $3)
            RETURNING
                CAST(id AS BIGINT) as id,
                description,
                state,
                CAST(owner_id AS BIGINT) as owner_id,
                message_id,
                channel_id,
                thread_id,
                details_msg_id
            "#,
        )
        .bind(description)
        .bind(MarketState::Open)
        .bind(owner.id)
        .fetch_one(db.psql())
        .await?;

        Ok(market)
    }

    async fn set_market_message_id(
        &self,
        db: &mut impl DbExecutor,
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

        query(
            "UPDATE markets SET message_id = $1, channel_id = $2, thread_id = $3, details_msg_id = $4 WHERE id = $5",
        )
        .bind(&message_id)
        .bind(&channel_id)
        .bind(&thread_id)
        .bind(&details_msg_id)
        .bind(market_id)
        .execute(db.psql())
        .await?;

        Ok(())
    }

    async fn get_market_by_id(&self, db: &mut impl DbExecutor, id: i64) -> anyhow::Result<Market> {
        let market = query_as(
            r#"
            SELECT
                CAST(id AS BIGINT) as id,
                description,
                state,
                CAST(owner_id AS BIGINT) as owner_id,
                message_id,
                channel_id,
                thread_id,
                details_msg_id
            FROM
                markets
            WHERE
                id = $1
            "#,
        )
        .bind(id)
        .fetch_one(db.psql())
        .await?;

        Ok(market)
    }

    async fn get_market_by_instrument_id(
        &self,
        db: &mut impl DbExecutor,
        instrument_id: i64,
    ) -> anyhow::Result<Market> {
        let market = query_as(
            r#"
            SELECT
                CAST(markets.id AS BIGINT) as id,
                markets.description,
                markets.state,
                CAST(markets.owner_id AS BIGINT) as owner_id,
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
        )
        .bind(instrument_id)
        .fetch_one(db.psql())
        .await?;

        Ok(market)
    }

    async fn get_markets_by_state(
        &self,
        db: &mut impl DbExecutor,
        state: MarketState,
    ) -> anyhow::Result<Vec<Market>> {
        let markets = query_as(
            r#"
            SELECT
                CAST(id AS BIGINT) as id,
                description,
                state,
                CAST(owner_id AS BIGINT) as owner_id,
                message_id,
                channel_id,
                thread_id,
                details_msg_id
            FROM
                markets
            WHERE
                state = $1
            ORDER BY id ASC
            "#,
        )
        .bind(state)
        .fetch_all(db.psql())
        .await?;

        Ok(markets)
    }

    async fn set_market_state(
        &self,
        db: &mut impl DbExecutor,
        market: &Market,
        state: MarketState,
    ) -> anyhow::Result<()> {
        query(
            r#"
            UPDATE
                markets
            SET
                state = $1
            WHERE
                id = $2
            "#,
        )
        .bind(state)
        .bind(market.id)
        .execute(db.psql())
        .await?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct FullMarket {
    pub row: Market,
    pub instruments: Vec<InstrumentWithShares>,
    pub owner: DbUser,
}

impl FullMarket {
    pub async fn new_from_instrument_id(
        exec: &mut impl DbExecutor,
        market_store: &impl MarketStore,
        instrument_store: &impl InstrumentStore,
        user_store: &impl UserStore,
        id: i64,
    ) -> anyhow::Result<Self> {
        let row = market_store.get_market_by_instrument_id(exec, id).await?;

        let instruments = instrument_store
            .get_instruments_with_share_counts_for_market(exec, row.id)
            .await?;

        let owner = user_store.get_by_id(exec, row.owner_id).await?;

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
