use std::sync::Arc;

use common::store::{
    CatshiDb, catfishing::DbCatfishingStore, instrument::DbInstrumentStore,
    login_token::DbLoginTokenStore, position::DbPositionStore, transfer::DbTransferStore,
    user::DbUserStore,
};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<CatshiDb>,
    pub cf_store: DbCatfishingStore,
    pub login_token_store: DbLoginTokenStore,
    pub user_store: DbUserStore,
    pub instrument_store: DbInstrumentStore,
    pub position_store: DbPositionStore,
    pub transfer_store: DbTransferStore,
}

impl AppState {
    pub fn new(db: Arc<CatshiDb>) -> Self {
        Self {
            db,
            cf_store: DbCatfishingStore {},
            login_token_store: DbLoginTokenStore {},
            user_store: DbUserStore {},
            instrument_store: DbInstrumentStore {},
            position_store: DbPositionStore {},
            transfer_store: DbTransferStore {},
        }
    }
}
