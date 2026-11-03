use anyhow::anyhow;
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use common::{
    discord_ui::render_trade_thread_message,
    place_trade::{
        PlaceTradeError, PlacedTrade, TradeAction, TradeRequest, place_trade, validate_trade,
    },
    store::{market::FullMarket, position::PositionStore, user::UserStore},
    trade::{MARKET_B, calc_buy_prices, calc_sell_prices, get_max_buy_shares},
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{AuthUser, CurrentUser},
    discord::sync_market_after_trade,
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
        user: placed_user,
        result,
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

    let thread_message = render_trade_thread_message(
        &placed_user.name,
        body.direction.into(),
        result.quantity,
        &instrument.name,
        &result.prices,
    );
    {
        let state = state.clone();
        let market = market.clone();
        let placed_user = placed_user.clone();
        tokio::spawn(async move {
            if let Err(e) = sync_market_after_trade(&state, &market, &placed_user, &thread_message).await
            {
                eprintln!("failed to update Discord after trade: {e:#}");
            }
        });
    }

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

#[derive(Deserialize)]
pub struct QuoteQuery {
    quantity: i64,
    direction: Direction,
    market_id: i32,
    instrument_id: i32,
}

#[derive(Serialize)]
pub struct QuoteResponse {
    market_id: i32,
    instrument_id: i32,
    quantity: i64,
    shares_price: i64,
    fees: i64,
    total: i64,
    max_quantity: i64,
}

pub async fn get_quote(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<QuoteQuery>,
) -> HandlerResult<Json<QuoteResponse>> {
    let mut conn = state.db.conn().await?;

    let market = FullMarket::new_from_instrument_id(
        &mut conn,
        &state.market_store,
        &state.instrument_store,
        &state.user_store,
        query.instrument_id,
    )
    .await?;
    let instrument = market.get_instrument(query.instrument_id)?.0.clone();

    let request = TradeRequest {
        market_id: Some(query.market_id),
        instrument_id: query.instrument_id,
        action: query.direction.into(),
        quantity: query.quantity,
    };
    validate_trade(&request, &market, &instrument).map_err(trade_error_response)?;

    let shares = market.instruments.iter();
    let max_quantity = match query.direction {
        Direction::Buy => {
            let (mut max, prices) =
                get_max_buy_shares(user.cash_balance, instrument.id, shares.clone(), MARKET_B);
            if prices.total() > user.cash_balance && max > 0 {
                max -= 1;
            }
            max.max(0)
        }
        Direction::Sell => {
            let position = state
                .position_store
                .get_user_position(&mut conn, &instrument, &user)
                .await?;
            position.map(|p| p.quantity).unwrap_or(0)
        }
    };
    let prices = match query.direction {
        Direction::Buy => calc_buy_prices(query.quantity, instrument.id, shares, MARKET_B),
        Direction::Sell => calc_sell_prices(query.quantity, instrument.id, shares, MARKET_B),
    };

    Ok(Json(QuoteResponse {
        market_id: market.row.id,
        instrument_id: instrument.id,
        quantity: query.quantity,
        shares_price: prices.shares_price.into(),
        fees: prices.fees.into(),
        total: prices.total().into(),
        max_quantity,
    }))
}
