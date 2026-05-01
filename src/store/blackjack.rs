use serenity::all::{ChannelId, MessageId};
use sqlx::{SqliteConnection, query, query_as};

use crate::currency::Currency;

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub enum BlackjackState {
    Betting,
    Closed,
}

#[derive(Debug, Clone)]
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

pub async fn get_blackjack_from_message(
    conn: &mut SqliteConnection,
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
    .fetch_one(conn)
    .await?;

    Ok(blackjack)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateBlackjack {
    pub dealer: String,
    pub player: String,
    pub staked: Currency,
    pub state: BlackjackState,
}

pub async fn update_blackjack(
    conn: &mut SqliteConnection,
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
    .execute(conn)
    .await?;

    Ok(())
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

pub async fn create_blackjack(
    conn: &mut SqliteConnection,
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
    .fetch_one(conn)
    .await?;

    Ok(created)
}
