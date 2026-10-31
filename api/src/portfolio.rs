use std::collections::HashMap;

use axum::{Json, extract::State};
use common::{
    portfolio::PortfolioValue,
    store::{
        instrument::{InstrumentStore, InstrumentWithShares},
        position::{PositionStore, PositionWithMarketId},
        transfer::TransferStore,
    },
};
use serde::Serialize;

use crate::{
    auth::{AuthUser, CurrentUser},
    error::HandlerResult,
    state::AppState,
};

// All currency values are integer 1000ths of a YP, matching Currency's internal
// representation (same as AuthUser::cash_balance).
#[derive(Serialize)]
pub struct PortfolioResponse {
    user: AuthUser,
    net_deposits: i64,
    trades_profit: i64,
    fees_profit: i64,
    gambling_winnings: i64,
    tips_received: i64,
    tips_sent: i64,
    transfers_received: i64,
    transfers_sent: i64,
    positions_value: i64,
    // Derived from the values above and the user's cash balance.
    net_profit: i64,
    net_worth: i64,
}

impl From<PortfolioValue> for PortfolioResponse {
    fn from(pv: PortfolioValue) -> Self {
        let net_profit = pv.net_profit().into();
        let net_worth = pv.net_worth().into();

        Self {
            user: pv.user.into(),
            net_deposits: pv.net_deposits.into(),
            trades_profit: pv.trades_profit.into(),
            fees_profit: pv.fees_profit.into(),
            gambling_winnings: pv.gambling_winnings.into(),
            tips_received: pv.tips_received.into(),
            tips_sent: pv.tips_sent.into(),
            transfers_received: pv.transfers_received.into(),
            transfers_sent: pv.transfers_sent.into(),
            positions_value: pv.positions_value.into(),
            net_profit,
            net_worth,
        }
    }
}

pub async fn get_portfolio(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> HandlerResult<Json<PortfolioResponse>> {
    let mut tx = state.db.begin().await?;

    let transfers = state
        .transfer_store
        .get_user_transfer_by_source_and_direction(&mut tx)
        .await?
        .into_iter()
        .map(|t| ((t.user.id, t.source, t.direction), t.amount))
        .collect();

    let positions: Vec<PositionWithMarketId> = state
        .position_store
        .get_all_positions_with_market_id(&mut tx)
        .await?
        .into_iter()
        .filter(|p| p.position.owner_id == user.id)
        .collect();

    let mut instruments_by_market: HashMap<i32, Vec<InstrumentWithShares>> = HashMap::new();
    for i in state
        .instrument_store
        .get_all_open_instruments_with_share_counts(&mut tx)
        .await?
    {
        instruments_by_market
            .entry(i.0.market_id)
            .or_default()
            .push(i);
    }

    let pv = PortfolioValue::new(user, &transfers, &positions, &instruments_by_market)?;

    Ok(Json(pv.into()))
}
