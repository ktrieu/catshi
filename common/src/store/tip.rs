use serenity::all::{GenericChannelId, MessageId};
use sqlx::query_as;
use trait_variant::make;

use crate::{
    currency::Currency,
    store::{DbExecutor, log_pg_compare_result, transfer::Transfer, user::DbUser},
};

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Tip {
    pub id: i64,
    pub created_at: i64,
    pub channel_id: String,
    pub message_id: String,
    pub amount: Currency,
    pub user_id: i64,
    pub transfer_id: i64,
}

#[make(Send)]
pub trait TipStore {
    async fn get_by_message_and_user(
        &self,
        db: &mut impl DbExecutor,
        user: &DbUser,
        channel_id: GenericChannelId,
        message_id: MessageId,
    ) -> anyhow::Result<Option<Tip>>;

    async fn create(
        &self,
        db: &mut impl DbExecutor,
        amount: Currency,
        transfer: &Transfer,
        user: &DbUser,
        channel_id: GenericChannelId,
        message_id: MessageId,
    ) -> anyhow::Result<Tip>;
}

pub struct DbTipStore {}

impl TipStore for DbTipStore {
    async fn get_by_message_and_user(
        &self,
        db: &mut impl DbExecutor,
        user: &DbUser,
        channel_id: GenericChannelId,
        message_id: MessageId,
    ) -> anyhow::Result<Option<Tip>> {
        let channel_id = channel_id.to_string();
        let message_id = message_id.to_string();

        let tip = query_as!(
            Tip,
            r#"
            SELECT
                id,
                created_at,
                channel_id,
                message_id,
                amount as "amount: Currency",
                user_id,
                transfer_id
            FROM
                tips
            WHERE
                channel_id = $1 AND message_id = $2 AND user_id = $3
            "#,
            channel_id,
            message_id,
            user.id
        )
        .fetch_optional(db.sqlite())
        .await?;

        let pg_tip: sqlx::Result<Option<Tip>> = query_as(
            r#"
            SELECT
                CAST(id AS BIGINT) as id,
                EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at,
                channel_id,
                message_id,
                CAST(amount AS BIGINT) as amount,
                CAST(user_id AS BIGINT) as user_id,
                CAST(transfer_id AS BIGINT) as transfer_id
            FROM
                tips
            WHERE
                channel_id = $1 AND message_id = $2 AND user_id = $3
            "#,
        )
        .bind(&channel_id)
        .bind(&message_id)
        .bind(user.id)
        .fetch_optional(db.psql())
        .await;

        log_pg_compare_result(pg_tip, &tip, "tip get_by_message_and_user");

        Ok(tip)
    }

    async fn create(
        &self,
        db: &mut impl DbExecutor,
        amount: Currency,
        transfer: &Transfer,
        user: &DbUser,
        channel_id: GenericChannelId,
        message_id: MessageId,
    ) -> anyhow::Result<Tip> {
        let channel_id = channel_id.to_string();
        let message_id = message_id.to_string();

        let tip = query_as!(
            Tip,
            r#"
            INSERT INTO tips (
                amount,
                transfer_id,
                channel_id,
                message_id,
                user_id
            ) VALUES ($1, $2, $3, $4, $5)
            RETURNING
                id,
                created_at,
                channel_id,
                message_id,
                amount as "amount: Currency",
                transfer_id,
                user_id
            "#,
            amount,
            transfer.id,
            channel_id,
            message_id,
            user.id,
        )
        .fetch_one(db.sqlite())
        .await?;

        // Mirror the insert to postgres, forcing `id` to match the sqlite row in case a
        // future table ends up referencing `tips(id)` by foreign key.
        let pg_result: sqlx::Result<Tip> = query_as(
            r#"
            INSERT INTO tips (
                id,
                amount,
                transfer_id,
                channel_id,
                message_id,
                user_id
            )
            OVERRIDING SYSTEM VALUE
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING
                CAST(id AS BIGINT) as id,
                EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at,
                channel_id,
                message_id,
                CAST(amount AS BIGINT) as amount,
                CAST(transfer_id AS BIGINT) as transfer_id,
                CAST(user_id AS BIGINT) as user_id
            "#,
        )
        .bind(tip.id)
        .bind(amount)
        .bind(transfer.id)
        .bind(&channel_id)
        .bind(&message_id)
        .bind(user.id)
        .fetch_one(db.psql())
        .await;

        log_pg_compare_result(pg_result, &tip, "tip create");

        Ok(tip)
    }
}
