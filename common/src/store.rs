use sqlx::{
    PgConnection, PgPool, Postgres, Sqlite, SqliteConnection, SqlitePool, Transaction,
    pool::PoolConnection,
};

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

trait DbExecutor {
    fn sqlite(&mut self) -> &mut SqliteConnection;
    fn psql(&mut self) -> &mut PgConnection;
}

pub struct CatshiDb {
    pub sqlite_pool: SqlitePool,
    pub pg_pool: PgPool,
}

pub struct CatshiConn {
    sqlite_conn: PoolConnection<Sqlite>,
    pg_conn: PoolConnection<Postgres>,
}

impl DbExecutor for CatshiConn {
    fn sqlite(&mut self) -> &mut SqliteConnection {
        &mut *self.sqlite_conn
    }

    fn psql(&mut self) -> &mut PgConnection {
        &mut *self.pg_conn
    }
}

pub struct CatshiTx {
    sqlite_tx: Transaction<'static, Sqlite>,
    pg_tx: Transaction<'static, Postgres>,
}

impl CatshiTx {
    pub async fn commit(self) -> anyhow::Result<()> {
        self.sqlite_tx.commit().await?;

        let psql_result = self.pg_tx.commit().await;
        if let Err(_) = psql_result {
            // todo: log an error here.
        }

        Ok(())
    }
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

    pub async fn conn(&self) -> anyhow::Result<CatshiConn> {
        Ok(CatshiConn {
            sqlite_conn: self.sqlite_pool.acquire().await?,
            pg_conn: self.pg_pool.acquire().await?,
        })
    }

    pub async fn begin(&self) -> anyhow::Result<CatshiTx> {
        Ok(CatshiTx {
            sqlite_tx: self.sqlite_pool.begin().await?,
            pg_tx: self.pg_pool.begin().await?,
        })
    }
}
