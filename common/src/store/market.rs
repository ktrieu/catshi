use std::collections::{HashMap, HashSet};

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
    pub id: i32,
    pub description: String,
    pub state: MarketState,
    pub owner_id: i32,
    pub message_id: Option<String>,
    pub channel_id: Option<String>,
    pub thread_id: Option<String>,
    pub details_msg_id: Option<String>,
    pub tag: Option<String>,
}

#[make(Send)]
pub trait MarketStore {
    async fn create_new_market(
        &self,
        db: &mut impl DbExecutor,
        description: &str,
        owner: &DbUser,
    ) -> anyhow::Result<Market>;

    /// Creates a market with the given tag unless one already exists.
    /// Returns `None` if a market with that tag was already present.
    async fn create_market_if_tag_absent(
        &self,
        db: &mut impl DbExecutor,
        tag: &str,
        description: &str,
        owner: &DbUser,
    ) -> anyhow::Result<Option<Market>>;

    async fn set_market_message_id(
        &self,
        db: &mut impl DbExecutor,
        market_id: i32,
        message_id: MessageId,
        channel_id: GenericChannelId,
        thread_id: ThreadId,
        details_msg_id: MessageId,
    ) -> anyhow::Result<()>;

    async fn get_market_by_id(&self, db: &mut impl DbExecutor, id: i32) -> anyhow::Result<Market>;

    /// Fetches the market for every given tag. Errors if any tag has no market.
    async fn get_markets_by_tags(
        &self,
        db: &mut impl DbExecutor,
        tags: &[String],
    ) -> anyhow::Result<Vec<Market>>;

    async fn get_market_by_instrument_id(
        &self,
        db: &mut impl DbExecutor,
        instrument_id: i32,
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

#[derive(Debug, Clone)]
pub struct DbMarketStore {}

impl MarketStore for DbMarketStore {
    async fn create_new_market(
        &self,
        db: &mut impl DbExecutor,
        description: &str,
        owner: &DbUser,
    ) -> anyhow::Result<Market> {
        let market = query_as!(
            Market,
            r#"
            INSERT INTO markets (
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
                thread_id,
                details_msg_id,
                tag
            "#,
            description,
            MarketState::Open as MarketState,
            owner.id,
        )
        .fetch_one(db.psql())
        .await?;

        Ok(market)
    }

    async fn create_market_if_tag_absent(
        &self,
        db: &mut impl DbExecutor,
        tag: &str,
        description: &str,
        owner: &DbUser,
    ) -> anyhow::Result<Option<Market>> {
        let market = query_as!(
            Market,
            r#"
            INSERT INTO markets (
                description,
                state,
                owner_id,
                tag
            )
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (tag) DO NOTHING
            RETURNING
                id,
                description,
                state as "state: MarketState",
                owner_id,
                message_id,
                channel_id,
                thread_id,
                details_msg_id,
                tag
            "#,
            description,
            MarketState::Open as MarketState,
            owner.id,
            tag,
        )
        .fetch_optional(db.psql())
        .await?;

        Ok(market)
    }

    async fn set_market_message_id(
        &self,
        db: &mut impl DbExecutor,
        market_id: i32,
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
        .execute(db.psql())
        .await?;

        Ok(())
    }

    async fn get_market_by_id(&self, db: &mut impl DbExecutor, id: i32) -> anyhow::Result<Market> {
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
                thread_id,
                details_msg_id,
                tag
            FROM
                markets
            WHERE
                id = $1
            "#,
            id,
        )
        .fetch_one(db.psql())
        .await?;

        Ok(market)
    }

    async fn get_markets_by_tags(
        &self,
        db: &mut impl DbExecutor,
        tags: &[String],
    ) -> anyhow::Result<Vec<Market>> {
        let markets = sqlx::query_as::<_, Market>(
            r#"
            SELECT
                id,
                description,
                state,
                owner_id,
                message_id,
                channel_id,
                thread_id,
                details_msg_id,
                tag
            FROM
                markets
            WHERE
                tag = ANY($1)
            ORDER BY id ASC
            "#,
        )
        .bind(tags)
        .fetch_all(db.psql())
        .await?;

        if markets.len() != tags.len() {
            let found: HashSet<&str> = markets.iter().filter_map(|m| m.tag.as_deref()).collect();
            let missing: Vec<&str> = tags
                .iter()
                .map(String::as_str)
                .filter(|t| !found.contains(t))
                .collect();
            return Err(anyhow!(
                "expected {} markets but found {}; missing tags: {}",
                tags.len(),
                markets.len(),
                missing.join(", ")
            ));
        }

        Ok(markets)
    }

    async fn get_market_by_instrument_id(
        &self,
        db: &mut impl DbExecutor,
        instrument_id: i32,
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
                markets.details_msg_id,
                markets.tag
            FROM
                markets
            JOIN
                instruments ON instruments.market_id = markets.id
            WHERE
                instruments.id = $1
            "#,
            instrument_id,
        )
        .fetch_one(db.psql())
        .await?;

        Ok(market)
    }

    async fn get_markets_by_state(
        &self,
        db: &mut impl DbExecutor,
        state: MarketState,
    ) -> anyhow::Result<Vec<Market>> {
        let markets = query_as!(
            Market,
            r#"
            SELECT
                id,
                description,
                state as "state: MarketState",
                owner_id,
                message_id,
                channel_id,
                thread_id,
                details_msg_id,
                tag
            FROM
                markets
            WHERE
                state = $1
            ORDER BY id ASC
            "#,
            state as MarketState,
        )
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
        query!(
            r#"
            UPDATE
                markets
            SET
                state = $1
            WHERE
                id = $2
            "#,
            state as MarketState,
            market.id,
        )
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
        id: i32,
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

    pub async fn new_from_tags(
        exec: &mut impl DbExecutor,
        market_store: &impl MarketStore,
        instrument_store: &impl InstrumentStore,
        user_store: &impl UserStore,
        tags: &[String],
    ) -> anyhow::Result<Vec<Self>> {
        let rows = market_store.get_markets_by_tags(exec, tags).await?;

        let market_ids: Vec<i32> = rows.iter().map(|m| m.id).collect();
        let mut instruments_by_market: HashMap<i32, Vec<InstrumentWithShares>> = HashMap::new();
        for i in instrument_store
            .get_instruments_with_share_counts_for_markets(exec, &market_ids)
            .await?
        {
            instruments_by_market.entry(i.0.market_id).or_default().push(i);
        }

        let mut markets = Vec::with_capacity(rows.len());
        for row in rows {
            let owner = user_store.get_by_id(exec, row.owner_id).await?;
            let instruments = instruments_by_market.remove(&row.id).unwrap_or_default();
            markets.push(Self {
                row,
                instruments,
                owner,
            });
        }

        Ok(markets)
    }

    pub fn get_instrument(&self, id: i32) -> anyhow::Result<&InstrumentWithShares> {
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
