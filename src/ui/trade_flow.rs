use std::str::FromStr;

use serenity::all::{
    CreateInputText, CreateLabel, CreateModal, CreateModalComponent, InputTextStyle,
    ModalInteraction,
};

use crate::{
    command::trade::TradeAction,
    store::{Instrument, Market},
    ui::{self, extract_modal_values},
};

pub fn generate_trade_modal_id(action: TradeAction, instrument_id: i64) -> String {
    format!("trade_modal|{}|{}", action.to_string(), instrument_id)
}

pub fn parse_trade_modal_id(id: &str) -> Option<(TradeAction, i64)> {
    let components: Vec<&str> = id.split('|').collect();

    if components.len() != 3 {
        return None;
    }

    if components[0] != "trade_modal" {
        return None;
    }

    let action = TradeAction::from_str(components[1]).ok()?;
    let id = components[2].parse::<i64>().ok()?;

    Some((action, id))
}

const TRADE_MODAL_QUANTITY_ID: &'static str = "trade_modal_quantity";

pub fn create_trade_modal(
    action: TradeAction,
    market: &Market,
    instrument: &Instrument,
    max_shares: i64,
) -> CreateModal<'static> {
    let instrument_text = ui::instrument_display_text(instrument, market);
    let verb = match action {
        TradeAction::Buy => "Buying",
        TradeAction::Sell => "Selling",
    };

    let description = match action {
        TradeAction::Buy => {
            format!("You can buy a maximum of {max_shares} shares with your current balance.")
        }
        TradeAction::Sell => {
            format!("You have a maximum of {max_shares} shares available to sell.")
        }
    };

    let input_label = CreateLabel::input_text(
        "Quantity",
        CreateInputText::new(InputTextStyle::Short, TRADE_MODAL_QUANTITY_ID)
            .required(true)
            .value("1"),
    )
    .description(description);

    let components = vec![CreateModalComponent::Label(input_label)];

    let header = format!("{verb} {instrument_text}").to_string();
    let modal_id = generate_trade_modal_id(action, instrument.id);
    CreateModal::new(modal_id, header).components(components)
}

pub fn extract_quantity_from_trade_modal(modal: &ModalInteraction) -> Option<i64> {
    let values = extract_modal_values(modal);

    values
        .get(TRADE_MODAL_QUANTITY_ID)
        .map(|val| val.parse::<i64>().ok())
        .flatten()
}
