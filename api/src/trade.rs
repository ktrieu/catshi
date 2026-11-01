use anyhow::anyhow;
use axum::{Json, extract::State, http::StatusCode};
use common::{
    place_trade::{PlaceTradeError, PlacedTrade, TradeAction, TradeRequest, place_trade},
    store::user::UserStore,
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{AuthUser, CurrentUser},
    error::{AppError, HandlerResult},
    state::AppState,
};

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Buy,
    Sell,
}

impl From<Direction> for TradeAction {
    fn from(value: Direction) -> Self {
        match value {
            Direction::Buy => TradeAction::Buy,
            Direction::Sell => TradeAction::Sell,
        }
    }
}

#[derive(Deserialize)]
pub struct TradeBody {
    quantity: i64,
    direction: Direction,
    market_id: i32,
    instrument_id: i32,
}

// All currency values are integer 1000ths of a YP, matching Currency's internal
// representation (same as AuthUser::cash_balance).
#[derive(Serialize)]
pub struct TradeResponse {
    user: AuthUser,
    market_id: i32,
    instrument_id: i32,
    quantity: i64,
    shares_price: i64,
    fees: i64,
    total: i64,
    position_quantity: i64,
}

fn trade_error_response(value: PlaceTradeError) -> AppError {
    let status = match value {
        PlaceTradeError::Internal(e) => return AppError::from(e),
        PlaceTradeError::NonPositiveQuantity
        | PlaceTradeError::WrongMarket { .. }
        | PlaceTradeError::InsufficientFunds { .. }
        | PlaceTradeError::InsufficientShares { .. } => StatusCode::BAD_REQUEST,
        PlaceTradeError::MarketClosed | PlaceTradeError::InstrumentClosed => StatusCode::CONFLICT,
    };
    let message = value.user_message().unwrap_or_default();
    AppError::with_status_code(anyhow!(message), status)
}

pub async fn post_trade(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<TradeBody>,
) -> HandlerResult<Json<TradeResponse>> {
    let mut tx = state.db.begin().await?;

    let request = TradeRequest {
        market_id: Some(body.market_id),
        instrument_id: body.instrument_id,
        action: body.direction.into(),
        quantity: body.quantity,
    };

    let PlacedTrade {
        market,
        instrument,
        result,
        ..
    } = place_trade(
        &mut tx,
        &state.market_store,
        &state.instrument_store,
        &state.user_store,
        &state.position_store,
        &state.order_store,
        &state.transfer_store,
        user.id,
        request,
    )
    .await
    .map_err(trade_error_response)?;

    tx.commit().await?;

    let mut conn = state.db.conn().await?;
    let user = state.user_store.get_by_id(&mut conn, user.id).await?;

    Ok(Json(TradeResponse {
        user: user.into(),
        market_id: market.row.id,
        instrument_id: instrument.id,
        quantity: result.quantity,
        shares_price: result.prices.shares_price.into(),
        fees: result.prices.fees.into(),
        total: result.prices.total().into(),
        position_quantity: result.position.quantity,
    }))
}
