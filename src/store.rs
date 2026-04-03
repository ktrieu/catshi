use sqlx::{Sqlite, Transaction, query_as};

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
pub struct User {
    id: i64,
    discord_id: String,
    name: String,
}

pub async fn insert_user_if_not_exists(
    tx: &mut Transaction<'_, Sqlite>,
    discord_id: &str,
    name: &str,
) -> anyhow::Result<Option<User>> {
    let user = query_as!(
        User,
        "INSERT INTO users(discord_id, name) VALUES ($1, $2) ON CONFLICT (discord_id) DO NOTHING RETURNING *",
        discord_id,
        name
    )
    .fetch_optional(&mut **tx)
    .await?;

    Ok(user)
}
