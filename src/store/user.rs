use anyhow::anyhow;
use serenity::all::UserId;
use sqlx::{Executor, Sqlite, query, query_as};

use crate::currency::Currency;

#[derive(Debug, sqlx::FromRow, Clone)]
#[allow(dead_code)]
pub struct DbUser {
    pub id: i64,
    pub discord_id: String,
    pub name: String,
    pub cash_balance: Currency,
}

pub async fn insert_user_if_not_exists(
    exec: impl Executor<'_, Database = Sqlite>,
    discord_id: &str,
    name: &str,
    initial_balance: Currency,
) -> anyhow::Result<DbUser> {
    let user = query_as!(
        DbUser,
        "INSERT INTO users(discord_id, name, cash_balance) VALUES ($1, $2, $3) ON CONFLICT (discord_id) DO NOTHING RETURNING *",
        discord_id,
        name,
        initial_balance,
    )
    .fetch_one(exec)
    .await?;

    Ok(user)
}

pub async fn get_user_by_discord_id(
    exec: impl Executor<'_, Database = Sqlite>,
    discord_id: &UserId,
) -> anyhow::Result<Option<DbUser>> {
    let discord_id = discord_id.to_string();

    let user = query_as!(
        DbUser,
        r#"SELECT
            id,
            name,
            discord_id,
            cash_balance as "cash_balance: Currency"
        FROM users 
        WHERE 
            discord_id = $1"#,
        discord_id,
    )
    .fetch_optional(exec)
    .await?;

    Ok(user)
}

pub async fn get_user_by_id(
    exec: impl Executor<'_, Database = Sqlite>,
    id: i64,
) -> anyhow::Result<Option<DbUser>> {
    let user = query_as!(
        DbUser,
        r#"SELECT
            id,
            name,
            discord_id,
            cash_balance as "cash_balance: Currency"
        FROM users 
        WHERE 
            id = $1"#,
        id,
    )
    .fetch_optional(exec)
    .await?;

    Ok(user)
}

pub async fn get_system_user(exec: impl Executor<'_, Database = Sqlite>) -> anyhow::Result<DbUser> {
    // By convention the system user has a discord ID of 0, see the migrations.
    get_user_by_discord_id(exec, &UserId::new(0))
        .await?
        .ok_or(anyhow!("system user not found"))
}

pub async fn increment_balance(
    exec: impl Executor<'_, Database = Sqlite>,
    user: &DbUser,
    amount: Currency,
) -> anyhow::Result<()> {
    query!(
        r#"UPDATE users SET cash_balance = cash_balance + $1 WHERE id = $2"#,
        amount,
        user.id,
    )
    .execute(exec)
    .await?;

    Ok(())
}

pub async fn get_users_with_positions_in_market(
    exec: impl Executor<'_, Database = Sqlite>,
    market_id: i64,
) -> anyhow::Result<Vec<DbUser>> {
    let users = query_as!(
        DbUser,
        r#"SELECT
            DISTINCT users.id,
            users.name,
            users.discord_id,
            users.cash_balance as "cash_balance: Currency"
        FROM users 
        JOIN positions ON positions.owner_id = users.id
        JOIN instruments ON instruments.id = positions.instrument_id
        WHERE 
            instruments.market_id = $1"#,
        market_id,
    )
    .fetch_all(exec)
    .await?;

    Ok(users)
}
