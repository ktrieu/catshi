use sqlx::query_as;

use crate::scripts::script_db;

struct UserRow {
    id: i64,
    name: String,
    discord_id: String,
    cash_balance: i64,
}

pub async fn run() -> anyhow::Result<()> {
    let db = script_db().await?;
    let mut tx = db.begin().await?;

    let all_users = query_as!(
        UserRow,
        r#"
        SELECT
            id,
            name,
            discord_id,
            cash_balance
        FROM
            users
        "#
    )
    .fetch_all(&mut **tx.sqlite_tx())
    .await?;

    Ok(())
}
