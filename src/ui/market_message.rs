use serenity::all::{
    ButtonStyle, CreateActionRow, CreateButton, CreateComponent, CreateSeparator, CreateTextDisplay,
};

use crate::{
    command::trade::{TradeAction, get_trade_button_id},
    store::{InstrumentState, InstrumentWithShares, Market, MarketState},
    trade,
};

pub fn get_market_resolve_id(market: &Market) -> String {
    format!("resolve_button|{}", market.id)
}

pub fn parse_market_resolve_button_id(id: &str) -> Option<i64> {
    let components: Vec<&str> = id.split('|').collect();

    if components.len() != 2 {
        return None;
    };

    if components[0] != "resolve_button" {
        return None;
    }

    components[1].parse::<i64>().ok()
}

pub fn get_market_details_button_id(market: &Market) -> String {
    format!("market_details_button|{}", market.id)
}

pub fn parse_market_details_button_id(id: &str) -> Option<i64> {
    let components: Vec<&str> = id.split('|').collect();

    if components.len() != 2 {
        return None;
    };

    if components[0] != "market_details_button" {
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
        let name = &i.name;
        let instrument_text = match i.state {
            InstrumentState::Open => {
                let price = trade::calc_price(i.id, instruments.clone(), trade::MARKET_B);
                format!("{name} ({price})")
            }
            InstrumentState::Winner => {
                format!("{name} ✅")
            }
            InstrumentState::Loser => {
                format!("{name} ❌")
            }
        };

        let desc_label = CreateTextDisplay::new(instrument_text);
        components.push(CreateComponent::TextDisplay(desc_label));

        if i.state == InstrumentState::Open {
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
    }

    components.push(CreateComponent::Separator(CreateSeparator::new()));

    if market.state == MarketState::Open {
        let secondary_actions_row = vec![
            CreateButton::new(get_market_resolve_id(market))
                .label("Resolve")
                .style(ButtonStyle::Secondary),
            CreateButton::new(get_market_details_button_id(market))
                .label("Details")
                .style(ButtonStyle::Secondary),
        ];
        components.push(CreateComponent::ActionRow(CreateActionRow::Buttons(
            secondary_actions_row.into(),
        )));
    }

    components
}
