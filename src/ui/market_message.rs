use serenity::all::{
    ButtonStyle, CreateActionRow, CreateButton, CreateComponent, CreateSeparator, CreateTextDisplay,
};

use crate::{
    command::trade::{TradeAction, get_trade_button_id},
    store::{InstrumentWithShares, Market},
    trade,
};

pub fn get_market_resolve_id(market: &Market) -> String {
    format!("resolve_button|{}", market.id)
}

pub fn parse_market_resolve_id(id: &str) -> Option<i64> {
    let components: Vec<&str> = id.split('|').collect();

    if components.len() != 2 {
        return None;
    };

    if components[0] != "resolve_button" {
        return None;
    }

    components[1].parse::<i64>().ok()
}

pub fn render_market_message<'a>(
    market: &'a Market,
    instruments: impl Iterator<Item = &'a InstrumentWithShares> + Clone,
) -> Vec<CreateComponent<'a>> {
    let title = CreateTextDisplay::new(format!("## Market #{:04}", market.id));

    let desc = CreateTextDisplay::new(&market.description);

    let mut components = vec![
        CreateComponent::TextDisplay(title),
        CreateComponent::TextDisplay(desc),
        CreateComponent::Separator(CreateSeparator::new()),
    ];

    for (i, _) in instruments.clone() {
        let price = trade::calc_price(i.id, instruments.clone(), trade::MARKET_B);
        let desc_label = CreateTextDisplay::new(format!("{} ({})", i.name, price));
        components.push(CreateComponent::TextDisplay(desc_label));

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

    components.push(CreateComponent::Separator(CreateSeparator::new()));

    let resolve_button = vec![
        CreateButton::new(get_market_resolve_id(market))
            .label("Resolve")
            .style(ButtonStyle::Secondary),
    ];

    components.push(CreateComponent::ActionRow(CreateActionRow::Buttons(
        resolve_button.into(),
    )));

    components
}
