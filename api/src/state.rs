use std::sync::Arc;

use common::store::{
    CatshiDb, catfishing::DbCatfishingStore, login_token::DbLoginTokenStore, user::DbUserStore,
};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<CatshiDb>,
    pub cf_store: DbCatfishingStore,
    pub login_token_store: DbLoginTokenStore,
    pub user_store: DbUserStore,
}

impl AppState {
    pub fn new(db: Arc<CatshiDb>) -> Self {
        Self {
            db,
            cf_store: DbCatfishingStore {},
            login_token_store: DbLoginTokenStore {},
            user_store: DbUserStore {},
        }
    }
}
