use std::str::FromStr;

use serenity::all::{
    CreateInputText, CreateLabel, CreateModal, CreateModalComponent, CreateSelectMenu,
    CreateSelectMenuKind, CreateSelectMenuOption, InputTextStyle, ModalInteraction,
};

use crate::{
    command::trade::TradeAction,
    currency::Currency,
    store::{instrument::Instrument, market::Market},
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

const TRADE_MODAL_QUANTITY_PREFILL: &'static str = "trade_modal_quantity_prefill";
const TRADE_MODAL_QUANTITY_FREEFORM_ID: &'static str = "trade_modal_quantity_freeform";

pub fn create_trade_modal(
    action: TradeAction,
    market: &Market,
    instrument: &Instrument,
    max_shares: i64,
    prefilled: Vec<(i64, Currency)>,
    balance: Currency,
) -> CreateModal<'static> {
    let instrument_text = ui::instrument_display_text(instrument, market);
    let verb = match action {
        TradeAction::Buy => "Buying",
        TradeAction::Sell => "Selling",
    };

    let options: Vec<CreateSelectMenuOption> = prefilled
        .iter()
        .map(|(quantity, total)| {
            CreateSelectMenuOption::new(format!("{quantity} ({total})"), quantity.to_string())
        })
        .collect();

    let prefilled_label = CreateLabel::select_menu(
        "Quantity",
        CreateSelectMenu::new(
            TRADE_MODAL_QUANTITY_PREFILL,
            CreateSelectMenuKind::String {
                options: options.into(),
            },
        )
        .required(false),
    )
    .description("Select from this menu or enter a custom amount below.");

    let description = match action {
        TradeAction::Buy => {
            format!("You have {balance} in cash and can buy up to {max_shares} shares.")
        }
        TradeAction::Sell => {
            format!("You have a maximum of {max_shares} shares available to sell.")
        }
    };
    let freeform_label = CreateLabel::input_text(
        "Custom Quantity",
        CreateInputText::new(InputTextStyle::Short, TRADE_MODAL_QUANTITY_FREEFORM_ID)
            .required(false),
    )
    .description(description);

    let components = vec![
        CreateModalComponent::Label(prefilled_label),
        CreateModalComponent::Label(freeform_label),
    ];

    let header = format!("{verb} {instrument_text}").to_string();
    let modal_id = generate_trade_modal_id(action, instrument.id);
    CreateModal::new(modal_id, header).components(components)
}

pub fn extract_quantity_from_trade_modal(modal: &ModalInteraction) -> Option<i64> {
    let values = extract_modal_values(modal);

    let prefilled = values.get(TRADE_MODAL_QUANTITY_PREFILL);

    let custom = values.get(TRADE_MODAL_QUANTITY_FREEFORM_ID);

    prefilled
        .or(custom)
        .map(|val| val.parse::<i64>().ok())
        .flatten()
}
