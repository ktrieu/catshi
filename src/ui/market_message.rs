use std::str::FromStr;

use serenity::all::{
    ButtonStyle, CreateActionRow, CreateButton, CreateComponent, CreateSeparator, CreateTextDisplay,
};

use crate::store::{Instrument, Market};

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

fn get_trade_button_id(instrument: &Instrument, action: TradeAction) -> String {
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

pub fn render_market_message<'a>(
    market: &'a Market,
    instruments: &'a [Instrument],
) -> Vec<CreateComponent<'a>> {
    let title = CreateTextDisplay::new(format!("## Market #{:04}", market.id));

    let desc = CreateTextDisplay::new(&market.description);

    let mut components = vec![
        CreateComponent::TextDisplay(title),
        CreateComponent::TextDisplay(desc),
        CreateComponent::Separator(CreateSeparator::new()),
    ];

    for i in instruments {
        let name = CreateTextDisplay::new(&i.name);
        components.push(CreateComponent::TextDisplay(name));

        let buttons = vec![
            CreateButton::new(get_trade_button_id(i, TradeAction::Buy))
                .label("Buy")
                .style(ButtonStyle::Success),
            CreateButton::new(get_trade_button_id(i, TradeAction::Sell))
                .label("Sell")
                .style(ButtonStyle::Danger),
        ];

        let row = CreateActionRow::buttons(buttons);
        components.push(CreateComponent::ActionRow(row));
    }

    components
}
