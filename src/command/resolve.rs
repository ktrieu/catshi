use std::{cmp::Reverse, collections::HashMap};

use log::warn;
use serenity::all::{
    ComponentInteraction, Context, CreateInteractionResponse, CreateLabel, CreateModal,
    CreateModalComponent, CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption,
    CreateTextDisplay, EditMessage, ModalInteraction,
};

use crate::{
    Handler,
    currency::Currency,
    store::{
        self,
        instrument::InstrumentState,
        market::{Market, MarketState},
        user::DbUser,
    },
    trade::{self},
    ui::{
        self, extract_modal_values, format_market_id, market_message::render_market_message,
        tabulate,
    },
    utils,
};
use anyhow::{anyhow, bail};

pub const RESOLVE_INSTRUMENT_ID: &'static str = "resolve_market_instrument";

pub fn generate_market_resolve_modal_id(market: &Market) -> String {
    format!("resolve_market|{}", market.id)
}

pub fn parse_market_resolve_modal_id(id: &str) -> Option<i64> {
    let components: Vec<&str> = id.split("|").collect();

    if components.len() != 2 {
        return None;
    }

    if components[0] != "resolve_market" {
        return None;
    }

    components[1].parse::<i64>().ok()
}

pub async fn initiate_resolve(
    ctx: &Context,
    handler: &Handler,
    market_id: i64,
    component: &ComponentInteraction,
    user: &DbUser,
) -> anyhow::Result<()> {
    let market = store::market::get_market_by_id(&handler.pool, market_id).await?;

    if market.owner_id != user.id {
        component
            .create_response(
                &ctx.http,
                utils::text_interaction_response("You are not the owner of this market.", true),
            )
            .await?;
    }

    let instruments =
        store::instrument::get_instruments_with_share_counts_for_market(&handler.pool, market_id)
            .await?;

    let question = CreateTextDisplay::new(&market.description);

    let options: Vec<CreateSelectMenuOption> = instruments
        .iter()
        .map(|(i, _)| CreateSelectMenuOption::new(i.name.clone(), i.id.to_string()))
        .collect();

    let menu = CreateSelectMenu::new(
        RESOLVE_INSTRUMENT_ID,
        CreateSelectMenuKind::String {
            options: options.into(),
        },
    );

    let label = vec![
        CreateModalComponent::TextDisplay(question),
        CreateModalComponent::Label(
            CreateLabel::select_menu("Option", menu)
                .description("Select the winning option from the list"),
        ),
    ];

    let title = format!("Resolving Market {}", format_market_id(market.id));
    let modal =
        CreateModal::new(generate_market_resolve_modal_id(&market), title).components(label);

    component
        .create_response(&ctx.http, CreateInteractionResponse::Modal(modal))
        .await?;

    Ok(())
}

pub async fn resolve(
    ctx: &Context,
    handler: &Handler,
    market_id: i64,
    modal: &ModalInteraction,
    user: &DbUser,
) -> anyhow::Result<()> {
    let values = extract_modal_values(modal);

    let instrument_id = values
        .get(RESOLVE_INSTRUMENT_ID)
        .ok_or(anyhow!("market resolve instrument field not present"))?
        .parse::<i64>()?;

    let mut tx = handler.pool.begin_with("BEGIN IMMEDIATE").await?;

    let market = store::market::FullMarket::new_from_instrument_id(&mut *tx, instrument_id).await?;

    // This shouldn't happen and should be caught by the modal initiation logic. Double-check here
    // but just raise a raw error.
    if market.owner.id != user.id {
        bail!(
            "user {} was not the owner of market {}",
            user.id,
            market.row.id
        );
    }

    let winner = &market.get_instrument(instrument_id)?.0;
    let positions = store::position::get_all_market_positions(&mut *tx, market.row.id).await?;
    let system_user = store::user::get_system_user(&handler.pool).await?;

    let results = trade::resolve(&market, &winner, &positions, &system_user)?;

    for r in &results {
        store::order::create_order(&mut *tx, &r.order).await?;

        for t in &r.transfers {
            store::transfer::persist_transfer(&mut tx, &t).await?;
        }
        if r.position.quantity == 0 {
            store::position::delete_position(
                &mut *tx,
                r.position.instrument_id,
                r.position.owner_id,
            )
            .await?;
        } else {
            warn!(
                "non-zero position quantity when resolving! {} {}",
                r.position.instrument_id, r.position.owner_id
            );
            store::position::upsert_position(&mut *tx, &r.position).await?;
        }
    }

    // Set the market/instrument states.
    store::market::set_market_state(&mut *tx, &market.row, MarketState::Closed).await?;
    for (i, _) in &market.instruments {
        let state = if i.id == instrument_id {
            InstrumentState::Winner
        } else {
            InstrumentState::Loser
        };

        store::instrument::set_instrument_state(&mut *tx, &i, state).await?;
    }

    tx.commit().await?;

    // Only show the profit result msg if there were actually any positions closed out.
    if results.len() != 0 {
        let users = positions.iter().map(|p| &p.user);

        let mut profits: HashMap<i64, (&DbUser, Currency)> =
            users.map(|u| (u.id, (u, Currency::from(0)))).collect();

        for r in &results {
            let entry = profits.entry(r.order.owner_id);
            entry.and_modify(|(_, profit)| *profit = *profit + r.profit());
        }

        let mut profits: Vec<(&DbUser, Currency)> = profits.into_values().collect();
        profits.sort_by_key(|(_, profit)| Reverse(*profit));

        let profits: Vec<[String; 2]> = profits
            .into_iter()
            .map(|(user, profit)| [user.name.clone(), profit.to_string()])
            .collect();

        let mut rows: Vec<[String; 2]> = vec![["Name".to_string(), "Profit".to_string()]];
        rows.extend_from_slice(&profits);

        let final_resp = format!(
            "Market {} resolved.\n{}",
            format_market_id(market_id),
            tabulate(rows)
        );

        modal
            .create_response(
                &ctx.http,
                utils::text_interaction_response(&final_resp, false),
            )
            .await?;
    } else {
        modal.defer(&ctx.http).await?;
    }

    let mut conn = handler.pool.acquire().await?;
    // Refetch and re-render market message.
    let market =
        store::market::FullMarket::new_from_instrument_id(&mut conn, instrument_id).await?;
    let new_market_message =
        render_market_message(&market.row, &market.owner, market.instruments.iter());

    let mut market_message = ui::get_market_message(&market.row, &ctx).await?;

    market_message
        .edit(&ctx.http, EditMessage::new().components(new_market_message))
        .await?;

    Ok(())
}
