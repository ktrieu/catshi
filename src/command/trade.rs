use std::str::FromStr;

use serenity::all::{ComponentInteraction, Context};

use crate::{
    Handler,
    store::{self, DbUser, Instrument},
    trade::{self, TradeInput},
    utils,
};

#[derive(Debug, PartialEq, Eq)]
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

pub async fn trade(
    ctx: &Context,
    handler: &Handler,
    user: &DbUser,
    component: &ComponentInteraction,
    action: TradeAction,
    instrument_id: i64,
) -> anyhow::Result<()> {
    let quantity = 1;
    let input = TradeInput::new(&handler.pool, instrument_id, 1, (*user).clone()).await?;

    let system_user = store::get_system_user(&handler.pool).await?;

    if action == TradeAction::Buy {
        let result = trade::buy(&handler.pool, &input, &system_user).await?;

        let msg = format!(
            "Bought {quantity} shares of instrument {instrument_id}. Total: {} ({} + {} fees)",
            result.total(),
            result.shares_price,
            result.fees
        );
        component
            .create_response(&ctx.http, utils::text_interaction_response(&msg, true))
            .await?;
    } else {
        let result = trade::sell(&handler.pool, &input, &system_user).await?;

        let msg = format!(
            "Sold {quantity} shares of instrument {instrument_id}. Total: {} ({} - {} fees). Profit {}",
            result.net(),
            result.shares_price,
            result.fees,
            result.profit()
        );
        component
            .create_response(&ctx.http, utils::text_interaction_response(&msg, true))
            .await?;
    }

    Ok(())
}
