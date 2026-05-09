use serenity::all::{GenericChannelId, MessageId};
use sqlx::{SqliteConnection, query_as};

use crate::{
    currency::Currency,
    store::{transfer::Transfer, user::DbUser},
};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Tip {
    id: i64,
    created_at: i64,
    channel_id: String,
    message_id: String,
    amount: Currency,
    user_id: i64,
    transfer_id: i64,
}

pub async fn get_tip_by_message_and_user(
    conn: &mut SqliteConnection,
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
    .fetch_optional(conn)
    .await?;

    Ok(tip)
}

pub async fn create_tip(
    conn: &mut SqliteConnection,
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
    .fetch_one(conn)
    .await?;

    Ok(tip)
}
