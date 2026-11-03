use std::sync::Arc;

use common::store::{
    CatshiDb, catfishing::DbCatfishingStore, instrument::DbInstrumentStore,
    login_token::DbLoginTokenStore, market::DbMarketStore, order::DbOrderStore,
    position::DbPositionStore, transfer::DbTransferStore, user::DbUserStore,
};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<CatshiDb>,
    pub cf_store: DbCatfishingStore,
    pub login_token_store: DbLoginTokenStore,
    pub user_store: DbUserStore,
    pub instrument_store: DbInstrumentStore,
    pub market_store: DbMarketStore,
    pub order_store: DbOrderStore,
    pub position_store: DbPositionStore,
    pub transfer_store: DbTransferStore,
    pub guild_id: String,
}

impl AppState {
    pub fn new(db: Arc<CatshiDb>, guild_id: String) -> Self {
        Self {
            db,
            guild_id,
            cf_store: DbCatfishingStore {},
            login_token_store: DbLoginTokenStore {},
            user_store: DbUserStore {},
            instrument_store: DbInstrumentStore {},
            market_store: DbMarketStore {},
            order_store: DbOrderStore {},
            position_store: DbPositionStore {},
            transfer_store: DbTransferStore {},
        }
    }
}
