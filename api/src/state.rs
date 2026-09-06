use std::sync::Arc;

use common::store::{CatshiDb, catfishing::DbCatfishingStore};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<CatshiDb>,
    pub cf_store: DbCatfishingStore,
}

impl AppState {
    pub fn new(db: Arc<CatshiDb>) -> Self {
        Self {
            db,
            cf_store: DbCatfishingStore {},
        }
    }
}
