use std::{cmp::Reverse, collections::HashMap};

use serenity::all::{
    ButtonStyle, CreateActionRow, CreateButton, CreateComponent, CreateSeparator, CreateTextDisplay,
};

use crate::{
    command::trade::{TradeAction, get_trade_button_id},
    store::{
        instrument::{InstrumentState, InstrumentWithShares},
        market::{Market, MarketState},
        position::PositionWithUser,
        user::DbUser,
    },
    trade,
    ui::{code_block, tabulate, user_shortname},
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

pub fn render_market_message<'a>(
    market: &'a Market,
    owner: &'a DbUser,
    instruments: impl Iterator<Item = &'a InstrumentWithShares> + Clone,
) -> Vec<CreateComponent<'a>> {
    let status = match market.state {
        MarketState::Open => "OPEN",
        MarketState::Closed => "CLOSED",
    };
    let title = CreateTextDisplay::new(format!("## {} MARKET #{:04}", status, market.id));
    let owner_name = CreateTextDisplay::new(owner.name.clone());

    let desc = CreateTextDisplay::new(&market.description);

    let mut components = vec![
        CreateComponent::TextDisplay(title),
        CreateComponent::TextDisplay(owner_name),
        CreateComponent::Separator(CreateSeparator::new()),
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
        ];
        components.push(CreateComponent::ActionRow(CreateActionRow::Buttons(
            secondary_actions_row.into(),
        )));
    }

    components
}

pub fn render_details_message(
    instruments: &Vec<InstrumentWithShares>,
    all_positions: &Vec<PositionWithUser>,
) -> String {
    let mut instrument_positions: HashMap<i64, Vec<&PositionWithUser>> = HashMap::new();

    for p in all_positions.iter() {
        if p.position.quantity == 0 {
            continue;
        }

        instrument_positions
            .entry(p.position.instrument_id)
            .or_default()
            .push(p);
    }

    let mut instrument_positions: Vec<(i64, Vec<&PositionWithUser>)> =
        instrument_positions.into_iter().collect();

    instrument_positions.sort_by_key(|(id, _)| *id);

    let mut lines: Vec<String> = Vec::new();

    for (instrument_id, positions) in instrument_positions.iter_mut() {
        let instrument_name = instruments
            .iter()
            .find(|(i, _)| i.id == *instrument_id)
            .expect(&format!("instrument {instrument_id} should exist",))
            .0
            .name
            .clone();

        let mut rows = vec![[
            "User".to_string(),
            "Quantity".to_string(),
            "Paid".to_string(),
        ]];

        positions.sort_by_key(|p| Reverse(p.position.quantity));

        for p in positions {
            rows.push([
                user_shortname(&p.user.name),
                p.position.quantity.to_string(),
                p.position.cost_basis.to_string(),
            ]);
        }
        let result = tabulate(rows);

        lines.push(instrument_name);
        lines.push(result);
    }

    if lines.len() > 0 {
        code_block(&lines.join("\n"))
    } else {
        "No open positions for this market.".to_string()
    }
}
