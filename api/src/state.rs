use sqlx::SqlitePool;

use common::store::catfishing::SqliteCatfishingStore;

#[derive(Debug, Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub cf_store: SqliteCatfishingStore,
}

impl AppState {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            cf_store: SqliteCatfishingStore {},
        }
    }
}
