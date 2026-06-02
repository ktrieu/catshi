use sqlx::SqlitePool;

pub mod blackjack;
pub mod instrument;
pub mod market;
pub mod order;
pub mod position;
pub mod tip;
pub mod transfer;
pub mod user;

pub async fn run_migrations(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;

    Ok(())
}
