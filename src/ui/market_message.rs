use serenity::all::{
    ButtonStyle, CreateActionRow, CreateButton, CreateComponent, CreateSeparator, CreateTextDisplay,
};

use crate::store::{Instrument, Market};

#[derive(Debug)]
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

fn get_trade_button_id(instrument: &Instrument, action: TradeAction) -> String {
    format!("trade_button_{}_{}", action.to_string(), instrument.id)
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
