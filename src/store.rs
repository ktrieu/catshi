use serenity::all::{MessageId, UserId};
use sqlx::{Executor, Sqlite, query, query_as};

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
pub struct User {
    id: i64,
    discord_id: String,
    name: String,
}

pub async fn insert_user_if_not_exists(
    exec: impl Executor<'_, Database = Sqlite>,
    discord_id: &str,
    name: &str,
) -> anyhow::Result<Option<User>> {
    let user = query_as!(
        User,
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
) -> anyhow::Result<Option<User>> {
    let discord_id = discord_id.to_string();

    let user = query_as!(
        User,
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
}

pub async fn create_new_market(
    exec: impl Executor<'_, Database = Sqlite>,
    description: &str,
    owner: &User,
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
                message_id
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
) -> anyhow::Result<()> {
    let message_id = message_id.to_string();

    query!(
        "UPDATE markets SET message_id = $1 WHERE id = $2",
        message_id,
        market_id,
    )
    .execute(exec)
    .await?;

    Ok(())
}
