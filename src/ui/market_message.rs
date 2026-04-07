use serenity::all::{
    ButtonStyle, CreateActionRow, CreateButton, CreateComponent, CreateSeparator, CreateTextDisplay,
};

use crate::{
    command::trade::{TradeAction, get_trade_button_id},
    store::{Instrument, Market},
};

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
