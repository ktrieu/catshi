use std::str::FromStr;

use anyhow::anyhow;
use serenity::all::{
    ComponentInteraction, Context, CreateInteractionResponse, EditMessage, GenericChannelId,
    MessageId, ModalInteraction,
};

use crate::{
    Handler,
    currency::Currency,
    store::{self, DbUser, Instrument, OrderDirection},
    trade::{self, BuyError, SellError, TradeInput, calc_buy_prices},
    ui::{
        instrument_display_text,
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
        store::get_instruments_with_share_counts_for_market(&handler.pool, market_id).await?;

    let (max_shares, prices) =
        trade::get_max_buy_shares(balance, instrument_id, shares.iter(), trade::MARKET_B);

    // If adding fees puts us over the top subtract our max_shares by 1.
    if prices.total(OrderDirection::Buy) > balance {
        Ok(max_shares - 1)
    } else {
        Ok(max_shares)
    }
}

async fn calc_max_sell_shares(
    handler: &Handler,
    instrument: &Instrument,
    user: &DbUser,
) -> anyhow::Result<i64> {
    let position = store::get_user_position(&handler.pool, instrument, user).await?;

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
            calc_buy_prices(quantity, instrument_id, instruments.iter(), trade::MARKET_B)
                .total(OrderDirection::Buy)
        }
        TradeAction::Sell => {
            calc_buy_prices(quantity, instrument_id, instruments.iter(), trade::MARKET_B)
                .total(OrderDirection::Sell)
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
    let market = store::get_market_by_instrument_id(&handler.pool, instrument_id)
        .await?
        .ok_or(anyhow!("market not found for instrument {}", instrument_id))?;

    let instruments =
        store::get_instruments_with_share_counts_for_market(&handler.pool, market.id).await?;

    let instrument = store::get_instrument_by_id(&handler.pool, instrument_id)
        .await?
        .ok_or(anyhow!("instrument {} not found", instrument_id))?;

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

    let input = TradeInput::new(&mut tx, instrument_id, quantity, (*user).clone()).await?;

    let system_user = store::get_system_user(&handler.pool).await?;

    if action == TradeAction::Buy {
        let result = trade::buy(&input).await;
        match result {
            Ok(result) => {
                result.persist(&mut tx, &system_user).await?;

                let msg = format!(
                    "Bought {} shares of {}. Total: {} ({} + {} fees)",
                    quantity,
                    instrument_display_text(&input.traded_instrument, &input.market),
                    result.total(),
                    result.shares_price,
                    result.fees
                );
                modal
                    .create_response(&ctx.http, utils::text_interaction_response(&msg, true))
                    .await?;
            }
            Err(BuyError::InsufficientFunds(total)) => {
                let message = format!(
                    "Insufficient funds: your order cost {total} and you only have {} in cash.",
                    input.user.cash_balance
                );
                modal
                    .create_response(
                        &ctx.http,
                        utils::text_interaction_response(message.as_str(), true),
                    )
                    .await?;
                return Ok(());
            }
        }
    } else {
        let result = trade::sell(&input).await;
        match result {
            Ok(result) => {
                result.persist(&mut tx, &system_user).await?;

                let msg = format!(
                    "Sold {} shares of {}. Total: {} ({} - {} fees). Profit {}",
                    quantity,
                    instrument_display_text(&input.traded_instrument, &input.market),
                    result.net(),
                    result.shares_price,
                    result.fees,
                    result.profit()
                );
                modal
                    .create_response(&ctx.http, utils::text_interaction_response(&msg, true))
                    .await?;
            }
            Err(SellError::InsufficientShares) => {
                let held_shares = input.position.map(|p| p.quantity).unwrap_or(0);
                let message = format!("You have only {held_shares} shares to sell.");
                modal
                    .create_response(
                        &ctx.http,
                        utils::text_interaction_response(message.as_str(), true),
                    )
                    .await?;
                return Ok(());
            }
        }
    }

    tx.commit().await?;

    let msg_id = input
        .market
        .message_id
        .as_ref()
        .ok_or(anyhow!(
            "message ID not found for market {}",
            input.market.id
        ))?
        .parse::<u64>()?;
    let channel_id = input
        .market
        .channel_id
        .as_ref()
        .ok_or(anyhow!(
            "channel ID not found for market {}",
            input.market.id
        ))?
        .parse::<u64>()?;

    let mut msg = ctx
        .http
        .get_message(GenericChannelId::new(channel_id), MessageId::new(msg_id))
        .await?;

    // Refetch the instruments after the trade is complete to update the market.
    let instruments =
        store::get_instruments_with_share_counts_for_market(&handler.pool, input.market.id).await?;
    let market_message =
        render_market_message(&input.market, &input.market_owner, instruments.iter());

    msg.edit(&ctx.http, EditMessage::new().components(market_message))
        .await?;

    let market_message =
        render_market_message(&input.market, &input.market_owner, instruments.iter());

    msg.edit(&ctx.http, EditMessage::new().components(market_message))
        .await?;

    Ok(())
}
