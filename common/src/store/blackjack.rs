use serenity::all::{ChannelId, MessageId};
use sqlx::{query, query_as};
use trait_variant::make;

use crate::{currency::Currency, store::DbExecutor};

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum BlackjackState {
    Betting,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct DbBlackjack {
    pub id: i64,
    pub dealer: String,
    pub player: String,
    pub state: BlackjackState,
    pub channel_id: String,
    pub message_id: String,
    pub owner_id: i64,
    pub staked: Currency,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateBlackjack {
    pub dealer: String,
    pub player: String,
    pub staked: Currency,
    pub state: BlackjackState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateBlackjack {
    pub dealer: String,
    pub player: String,
    pub owner_id: i64,
    pub state: BlackjackState,
    pub staked: Currency,
    pub channel_id: String,
    pub message_id: String,
}

#[make(Send)]
pub trait BlackjackStore {
    async fn get_from_message(
        &self,
        db: &mut impl DbExecutor,
        channel_id: ChannelId,
        message_id: MessageId,
    ) -> anyhow::Result<DbBlackjack>;

    async fn update(
        &self,
        db: &mut impl DbExecutor,
        id: i64,
        u: &UpdateBlackjack,
    ) -> anyhow::Result<()>;

    async fn create(
        &self,
        db: &mut impl DbExecutor,
        c: &CreateBlackjack,
    ) -> anyhow::Result<DbBlackjack>;
}

pub struct DbBlackjackStore {}

impl BlackjackStore for DbBlackjackStore {
    async fn get_from_message(
        &self,
        db: &mut impl DbExecutor,
        channel_id: ChannelId,
        message_id: MessageId,
    ) -> anyhow::Result<DbBlackjack> {
        let channel_id = channel_id.to_string();
        let message_id = message_id.to_string();

        let blackjack = query_as(
            r#"
            SELECT
                CAST(id AS BIGINT) as id,
                dealer,
                player,
                state,
                channel_id,
                message_id,
                CAST(owner_id AS BIGINT) as owner_id,
                CAST(staked AS BIGINT) as staked
            FROM
                blackjacks
            WHERE
                channel_id = $1 AND message_id = $2"#,
        )
        .bind(&channel_id)
        .bind(&message_id)
        .fetch_one(db.psql())
        .await?;

        Ok(blackjack)
    }

    async fn update(
        &self,
        db: &mut impl DbExecutor,
        id: i64,
        u: &UpdateBlackjack,
    ) -> anyhow::Result<()> {
        query(
            r#"
            UPDATE blackjacks SET
                dealer = $1,
                player = $2,
                staked = $3,
                state = $4
            WHERE
                id = $5
            "#,
        )
        .bind(&u.dealer)
        .bind(&u.player)
        .bind(u.staked)
        .bind(u.state)
        .bind(id)
        .execute(db.psql())
        .await?;

        Ok(())
    }

    async fn create(
        &self,
        db: &mut impl DbExecutor,
        c: &CreateBlackjack,
    ) -> anyhow::Result<DbBlackjack> {
        let created = query_as(
            r#"
            INSERT INTO blackjacks (
                dealer,
                player,
                state,
                owner_id,
                staked,
                channel_id,
                message_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING
                CAST(id AS BIGINT) as id,
                dealer,
                player,
                state,
                channel_id,
                message_id,
                CAST(owner_id AS BIGINT) as owner_id,
                CAST(staked AS BIGINT) as staked
            "#,
        )
        .bind(&c.dealer)
        .bind(&c.player)
        .bind(c.state)
        .bind(c.owner_id)
        .bind(c.staked)
        .bind(&c.channel_id)
        .bind(&c.message_id)
        .fetch_one(db.psql())
        .await?;

        Ok(created)
    }
}
