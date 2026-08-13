use serenity::all::{ChannelId, MessageId};
use sqlx::{query, query_as};
use trait_variant::make;

use crate::{
    currency::Currency,
    store::{DbExecutor, log_pg_compare_result, log_pg_write_err},
};

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

        let blackjack = query_as!(
            DbBlackjack,
            r#"
            SELECT
                id,
                dealer,
                player,
                state as "state: BlackjackState",
                channel_id,
                message_id,
                owner_id,
                staked as "staked: Currency"
            FROM
                blackjacks
            WHERE
                channel_id = $1 AND message_id = $2"#,
            channel_id,
            message_id,
        )
        .fetch_one(db.sqlite())
        .await?;

        let pg_blackjack: sqlx::Result<DbBlackjack> = query_as(
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
        .await;

        log_pg_compare_result(pg_blackjack, &blackjack, "blackjack get_from_message");

        Ok(blackjack)
    }

    async fn update(
        &self,
        db: &mut impl DbExecutor,
        id: i64,
        u: &UpdateBlackjack,
    ) -> anyhow::Result<()> {
        query!(
            r#"
            UPDATE blackjacks SET
                dealer = $1,
                player = $2,
                staked = $3,
                state = $4
            WHERE
                id = $5
            "#,
            u.dealer,
            u.player,
            u.staked,
            u.state,
            id
        )
        .execute(db.sqlite())
        .await?;

        let pg_result = query(
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
        .await;

        log_pg_write_err(pg_result, "blackjack update");

        Ok(())
    }

    async fn create(
        &self,
        db: &mut impl DbExecutor,
        c: &CreateBlackjack,
    ) -> anyhow::Result<DbBlackjack> {
        let created = query_as!(
            DbBlackjack,
            r#"
            INSERT INTO blackjacks (
                dealer,
                player,
                state,
                owner_id,
                staked,
                channel_id,
                message_id
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING
                id,
                dealer,
                player,
                state as "state: BlackjackState",
                channel_id,
                message_id,
                owner_id,
                staked as "staked: Currency"
            "#,
            c.dealer,
            c.player,
            c.state,
            c.owner_id,
            c.staked,
            c.channel_id,
            c.message_id
        )
        .fetch_one(db.sqlite())
        .await?;

        // Mirror the insert to postgres, forcing `id` to match the sqlite row in case a
        // future table ends up referencing `blackjacks(id)` by foreign key.
        let pg_result: sqlx::Result<DbBlackjack> = query_as(
            r#"
            INSERT INTO blackjacks (
                id,
                dealer,
                player,
                state,
                owner_id,
                staked,
                channel_id,
                message_id
            )
            OVERRIDING SYSTEM VALUE
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
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
        .bind(created.id)
        .bind(&c.dealer)
        .bind(&c.player)
        .bind(c.state)
        .bind(c.owner_id)
        .bind(c.staked)
        .bind(&c.channel_id)
        .bind(&c.message_id)
        .fetch_one(db.psql())
        .await;

        log_pg_compare_result(pg_result, &created, "blackjack create");

        Ok(created)
    }
}
