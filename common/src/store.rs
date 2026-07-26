use sqlx::{PgPool, SqlitePool};

pub mod blackjack;
pub mod catfishing;
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

pub struct CatshiDb {
    pub sqlite_pool: SqlitePool,
    pub pg_pool: PgPool,
}

impl CatshiDb {
    pub async fn new(sqlite_url: &str, pg_url: &str) -> anyhow::Result<Self> {
        let sqlite_pool = SqlitePool::connect(sqlite_url).await?;
        let pg_pool = PgPool::connect(pg_url).await?;

        Ok(Self {
            sqlite_pool,
            pg_pool,
        })
    }
}
