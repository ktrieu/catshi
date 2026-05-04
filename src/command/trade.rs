use std::str::FromStr;

use serenity::all::{
    ComponentInteraction, Context, CreateInteractionResponse, EditMessage, ModalInteraction,
};

use crate::{
    Handler,
    currency::Currency,
    store::{self, instrument::Instrument, market::FullMarket, user::DbUser},
    trade::{self, MARKET_B, TradeError, TradeResult, calc_buy_prices, calc_sell_prices},
    ui::{
        self, instrument_display_text,
        market_message::render_market_message,
        trade_flow::{create_trade_modal, extract_quantity_from_trade_modal},
    },
    utils,
};

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

pub fn get_trade_button_id(instrument: &Instrument, action: TradeAction) -> String {
    format!("trade_button|{}|{}", action.to_string(), instrument.id)
}

pub fn parse_trade_button_id(id: &str) -> Option<(TradeAction, i64)> {
    let components: Vec<&str> = id.split('|').collect();

    if components.len() != 3 {
        return None;
    }

    if components[0] != "trade_button" {
        return None;
    }

    let action = TradeAction::from_str(components[1]).ok()?;
    let id = components[2].parse::<i64>().ok()?;

    Some((action, id))
}

async fn calc_max_buy_shares(
    handler: &Handler,
    balance: Currency,
    market_id: i64,
    instrument_id: i64,
) -> anyhow::Result<i64> {
    let shares =
        store::instrument::get_instruments_with_share_counts_for_market(&handler.pool, market_id)
            .await?;

    let (mut max_shares, prices) =
        trade::get_max_buy_shares(balance, instrument_id, shares.iter(), trade::MARKET_B);

    // If adding fees puts us over the top subtract our max_shares by 1.
    if prices.total() > balance && max_shares > 0 {
        max_shares -= 1;
    };

    Ok(max_shares)
}

async fn calc_max_sell_shares(
    handler: &Handler,
    instrument: &Instrument,
    user: &DbUser,
) -> anyhow::Result<i64> {
    let position = store::position::get_user_position(&handler.pool, instrument, user).await?;

    match position {
        Some(position) => Ok(position.quantity),
        // No position = no sell
        None => Ok(0),
    }
}

fn get_prefilled_quantity(
    quantity: i64,
    instrument_id: i64,
    instruments: &Vec<(Instrument, i64)>,
    action: TradeAction,
) -> (i64, Currency) {
    // TODO: we should really harmonize this.
    let total = match action {
        TradeAction::Buy => {
            calc_buy_prices(quantity, instrument_id, instruments.iter(), trade::MARKET_B).total()
        }
        TradeAction::Sell => {
            calc_sell_prices(quantity, instrument_id, instruments.iter(), trade::MARKET_B).total()
        }
    };

    (quantity, total)
}

const PREFILLED_QUANTITIES: [i64; 5] = [1, 2, 5, 10, 20];

pub async fn initiate_trade(
    ctx: &Context,
    handler: &Handler,
    user: &DbUser,
    component: &ComponentInteraction,
    action: TradeAction,
    instrument_id: i64,
) -> anyhow::Result<()> {
    let market = store::market::get_market_by_instrument_id(&handler.pool, instrument_id).await?;
    let instruments =
        store::instrument::get_instruments_with_share_counts_for_market(&handler.pool, market.id)
            .await?;

    let instrument = store::instrument::get_instrument_by_id(&handler.pool, instrument_id).await?;

    let max_shares = match action {
        TradeAction::Buy => {
            calc_max_buy_shares(handler, user.cash_balance, market.id, instrument_id).await?
        }
        TradeAction::Sell => calc_max_sell_shares(handler, &instrument, user).await?,
    };

    let mut prefilled: Vec<(i64, Currency)> = Vec::new();
    for q in PREFILLED_QUANTITIES {
        if q < max_shares {
            prefilled.push(get_prefilled_quantity(
                q,
                instrument_id,
                &instruments,
                action,
            ));
        }
    }

    prefilled.push(get_prefilled_quantity(
        max_shares,
        instrument_id,
        &instruments,
        action,
    ));

    let modal = create_trade_modal(
        action,
        &market,
        &instrument,
        max_shares,
        prefilled,
        user.cash_balance,
    );

    component
        .create_response(&ctx.http, CreateInteractionResponse::Modal(modal))
        .await?;

    Ok(())
}

pub async fn trade(
    ctx: &Context,
    handler: &Handler,
    user: &DbUser,
    modal: &ModalInteraction,
    action: TradeAction,
    instrument_id: i64,
) -> anyhow::Result<()> {
    let quantity = match extract_quantity_from_trade_modal(modal) {
        Some(quantity) => quantity,
        None => {
            modal
                .create_response(
                    &ctx.http,
                    utils::text_interaction_response("Quantity was not a valid number.", true),
                )
                .await?;
            return Ok(());
        }
    };

    let mut tx = handler.pool.begin_with("BEGIN IMMEDIATE").await?;

    let market = FullMarket::new_from_instrument_id(&mut *tx, instrument_id).await?;
    let traded_instrument = &market.get_instrument(instrument_id)?.0;
    let position = store::position::get_user_position(&mut *tx, traded_instrument, user).await?;

    let system_user = store::user::get_system_user(&handler.pool).await?;

    let result = match action {
        TradeAction::Buy => trade::buy(
            quantity,
            traded_instrument,
            &market,
            position.as_ref(),
            user,
            &system_user,
            MARKET_B,
        ),
        TradeAction::Sell => trade::sell(
            quantity,
            traded_instrument,
            &market,
            position.as_ref(),
            user,
            &system_user,
            MARKET_B,
        ),
    };

    match result {
        Ok(result) => {
            store::order::create_order(&mut *tx, &result.order).await?;
            if result.position.quantity == 0 {
                store::position::delete_position(
                    &mut *tx,
                    result.position.instrument_id,
                    result.position.owner_id,
                )
                .await?;
            } else {
                store::position::upsert_position(&mut *tx, &result.position).await?;
            }
            for t in &result.transfers {
                store::transfer::persist_transfer(&mut tx, t).await?;
            }

            let verb = match action {
                TradeAction::Buy => "Bought",
                TradeAction::Sell => "Sold",
            };
            let fee_sign = match action {
                TradeAction::Buy => "+",
                TradeAction::Sell => "-",
            };

            let TradeResult {
                quantity, prices, ..
            } = result;
            let total = prices.total();
            let disp = instrument_display_text(traded_instrument, &market.row);
            let shares_price = prices.shares_price;
            let fees = prices.fees;

            let msg = format!(
                "{verb} {quantity} shares of {disp}. Total: {total} ({shares_price} {fee_sign} {fees} fees)",
            );
            modal
                .create_response(&ctx.http, utils::text_interaction_response(&msg, true))
                .await?;
        }
        Err(trade_error) => {
            let message = match trade_error {
                TradeError::InsufficientFunds(total) => format!(
                    "Insufficient funds: your order cost {} and you only have {} in cash.",
                    total, user.cash_balance
                ),
                TradeError::InsufficientShares => {
                    let held_shares = position.as_ref().map(|p| p.quantity).unwrap_or(0);
                    format!(
                        "Insufficient shares: you only have {} shares to sell.",
                        held_shares
                    )
                }
            };
            modal
                .create_response(
                    &ctx.http,
                    utils::text_interaction_response(message.as_str(), true),
                )
                .await?;
            return Ok(());
        }
    }

    tx.commit().await?;
    // Refetch the instruments after the trade is complete to update the market.
    let instruments = store::instrument::get_instruments_with_share_counts_for_market(
        &handler.pool,
        market.row.id,
    )
    .await?;
    let new_market_message = render_market_message(&market.row, &market.owner, instruments.iter());
    let mut market_message = ui::get_market_message(&market.row, ctx).await?;

    market_message
        .edit(&ctx.http, EditMessage::new().components(new_market_message))
        .await?;

    Ok(())
}
