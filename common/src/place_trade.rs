use std::str::FromStr;

use crate::currency::Currency;
use crate::store::{
    CatshiTx,
    instrument::{Instrument, InstrumentState, InstrumentStore},
    market::{FullMarket, MarketState, MarketStore},
    order::OrderStore,
    position::{Position, PositionStore},
    transfer::TransferStore,
    user::{DbUser, UserStore},
};
use crate::trade::{self, MARKET_B, TradeError, TradeResult};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum TradeAction {
    Buy,
    Sell,
}

impl ToString for TradeAction {
    fn to_string(&self) -> String {
        match &self {
            TradeAction::Buy => "buy".to_string(),
            TradeAction::Sell => "sell".to_string(),
        }
    }
}

impl FromStr for TradeAction {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "buy" => Ok(TradeAction::Buy),
            "sell" => Ok(TradeAction::Sell),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TradeRequest {
    pub market_id: Option<i32>,
    pub instrument_id: i32,
    pub action: TradeAction,
    pub quantity: i64,
}

#[derive(Debug)]
pub enum PlaceTradeError {
    NonPositiveQuantity,
    WrongMarket { expected: i32, actual: i32 },
    MarketClosed,
    InstrumentClosed,
    InsufficientFunds { total: Currency, balance: Currency },
    InsufficientShares { held: i64 },
    Internal(anyhow::Error),
}

impl<T> From<T> for PlaceTradeError
where
    T: Into<anyhow::Error>,
{
    fn from(value: T) -> Self {
        PlaceTradeError::Internal(value.into())
    }
}

impl PlaceTradeError {
    pub fn user_message(&self) -> Option<String> {
        let msg = match self {
            PlaceTradeError::NonPositiveQuantity => {
                "Negative quantity trades are not permitted.".to_string()
            }
            PlaceTradeError::WrongMarket { expected, actual } => {
                format!(
                    "Instrument does not belong to market {expected} (it belongs to market {actual})."
                )
            }
            PlaceTradeError::MarketClosed => "This market is closed.".to_string(),
            PlaceTradeError::InstrumentClosed => "This instrument has been resolved.".to_string(),
            PlaceTradeError::InsufficientFunds { total, balance } => format!(
                "Insufficient funds: your order cost {total} and you only have {balance} in cash."
            ),
            PlaceTradeError::InsufficientShares { held } => {
                format!("Insufficient shares: you only have {held} shares to sell.")
            }
            PlaceTradeError::Internal(_) => return None,
        };
        Some(msg)
    }
}

pub struct PlacedTrade {
    pub market: FullMarket,
    pub instrument: Instrument,
    pub user: DbUser,
    pub result: TradeResult,
}

pub fn validate_trade(
    request: &TradeRequest,
    market: &FullMarket,
    instrument: &Instrument,
) -> Result<(), PlaceTradeError> {
    if request.quantity <= 0 {
        return Err(PlaceTradeError::NonPositiveQuantity);
    }

    if let Some(expected) = request.market_id {
        if expected != market.row.id {
            return Err(PlaceTradeError::WrongMarket {
                expected,
                actual: market.row.id,
            });
        }
    }

    if market.row.state != MarketState::Open {
        return Err(PlaceTradeError::MarketClosed);
    }

    if instrument.state != InstrumentState::Open {
        return Err(PlaceTradeError::InstrumentClosed);
    }

    Ok(())
}

fn map_trade_error(err: TradeError, user: &DbUser, position: Option<&Position>) -> PlaceTradeError {
    match err {
        TradeError::InsufficientFunds(total) => PlaceTradeError::InsufficientFunds {
            total,
            balance: user.cash_balance,
        },
        TradeError::InsufficientShares => PlaceTradeError::InsufficientShares {
            held: position.map(|p| p.quantity).unwrap_or(0),
        },
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn place_trade(
    tx: &mut CatshiTx,
    market_store: &impl MarketStore,
    instrument_store: &impl InstrumentStore,
    user_store: &(impl UserStore + Sync),
    position_store: &impl PositionStore,
    order_store: &impl OrderStore,
    transfer_store: &impl TransferStore,
    user_id: i32,
    request: TradeRequest,
) -> Result<PlacedTrade, PlaceTradeError> {
    let market = FullMarket::new_from_instrument_id(
        tx,
        market_store,
        instrument_store,
        user_store,
        request.instrument_id,
    )
    .await?;
    let instrument = market.get_instrument(request.instrument_id)?.0.clone();

    validate_trade(&request, &market, &instrument)?;

    let user = user_store.get_by_id(tx, user_id).await?;
    let position = position_store
        .get_user_position(tx, &instrument, &user)
        .await?;
    let system_user = user_store.get_system_user(tx).await?;

    let result = match request.action {
        TradeAction::Buy => trade::buy(
            request.quantity,
            &instrument,
            &market,
            position.as_ref(),
            &user,
            &system_user,
            MARKET_B,
        ),
        TradeAction::Sell => trade::sell(
            request.quantity,
            &instrument,
            &market,
            position.as_ref(),
            &user,
            &system_user,
            MARKET_B,
        ),
    }
    .map_err(|e| map_trade_error(e, &user, position.as_ref()))?;

    order_store.create_order(tx, &result.order).await?;
    if result.position.quantity == 0 {
        position_store
            .delete_position(tx, result.position.instrument_id, result.position.owner_id)
            .await?;
    } else {
        position_store.upsert_position(tx, &result.position).await?;
    }
    for t in &result.transfers {
        transfer_store.persist(tx, user_store, t).await?;
    }

    Ok(PlacedTrade {
        market,
        instrument,
        user,
        result,
    })
}
