use sqlx::{PgConnection, PgPool, Postgres, Transaction, pool::PoolConnection};

pub mod blackjack;
pub mod catfishing;
pub mod instrument;
pub mod market;
pub mod order;
pub mod position;
pub mod tip;
pub mod transfer;
pub mod user;

pub trait DbExecutor: Send {
    fn psql(&mut self) -> &mut PgConnection;
}

pub struct CatshiDb {
    pub pg_pool: PgPool,
}

pub struct CatshiConn {
    pg_conn: PoolConnection<Postgres>,
}

impl DbExecutor for CatshiConn {
    fn psql(&mut self) -> &mut PgConnection {
        &mut *self.pg_conn
    }
}

pub struct CatshiTx {
    pg_tx: Transaction<'static, Postgres>,
}

impl CatshiTx {
    pub async fn commit(self) -> anyhow::Result<()> {
        self.pg_tx.commit().await?;

        Ok(())
    }
}

impl DbExecutor for CatshiTx {
    fn psql(&mut self) -> &mut PgConnection {
        &mut *self.pg_tx
    }
}

impl CatshiDb {
    pub async fn new(pg_url: &str) -> anyhow::Result<Self> {
        let pg_pool = PgPool::connect(pg_url).await?;

        Ok(Self { pg_pool })
    }

    pub async fn conn(&self) -> anyhow::Result<CatshiConn> {
        Ok(CatshiConn {
            pg_conn: self.pg_pool.acquire().await?,
        })
    }

    pub async fn begin(&self) -> anyhow::Result<CatshiTx> {
        Ok(CatshiTx {
            pg_tx: self.pg_pool.begin().await?,
        })
    }
}
