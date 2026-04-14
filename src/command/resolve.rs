use std::{cmp::Reverse, collections::HashMap};

use serenity::all::{
    ComponentInteraction, Context, CreateInteractionResponse, CreateLabel, CreateModal,
    CreateModalComponent, CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption,
    EditMessage, GenericChannelId, MessageId, ModalInteraction,
};

use crate::{
    Handler,
    currency::Currency,
    store::{
        self, DbUser, InstrumentState, Market, MarketState,
        get_instruments_with_share_counts_for_market,
    },
    trade::{self, ResolveInput},
    ui::{extract_modal_values, format_market_id, market_message::render_market_message, tabulate},
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
    let market = store::get_market_by_id(&handler.pool, market_id)
        .await?
        .ok_or(anyhow!("market {market_id} not found"))?;

    if market.owner_id != user.id {
        component
            .create_response(
                &ctx.http,
                utils::text_interaction_response("You are not the owner of this market.", true),
            )
            .await?;
    }

    let instruments =
        get_instruments_with_share_counts_for_market(&handler.pool, market_id).await?;

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

    let label = vec![CreateModalComponent::Label(
        CreateLabel::select_menu("Option", menu)
            .description("Select the winning option from the list"),
    )];

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
    let market = store::get_market_by_id(&handler.pool, market_id)
        .await?
        .ok_or(anyhow!("market {market_id} not found"))?;

    // This shouldn't happen and should be caught by the modal initiation logic. Double-check here
    // but just raise a raw error.
    if market.owner_id != user.id {
        bail!("user {} was not the owner of market {}", user.id, market.id);
    }

    let values = extract_modal_values(modal);

    let instrument_id = values
        .get(RESOLVE_INSTRUMENT_ID)
        .ok_or(anyhow!("market resolve instrument field not present"))?
        .parse::<i64>()?;

    let mut tx = handler.pool.begin_with("BEGIN IMMEDIATE").await?;

    let input = ResolveInput::new(&mut tx, market_id, instrument_id).await?;
    let system_user = store::get_system_user(&handler.pool).await?;

    let results = trade::resolve(&input).await?;

    for r in &results {
        r.persist(&mut tx, &system_user).await?;
    }

    // Set the market/instrument states.
    store::set_market_state(&mut *tx, &market, MarketState::Closed).await?;
    for (i, _) in &input.market_instruments {
        let state = if i.id == instrument_id {
            InstrumentState::Winner
        } else {
            InstrumentState::Loser
        };

        store::set_instrument_state(&mut *tx, &i, state).await?;
    }

    tx.commit().await?;

    // Only show the profit result msg if there were actually any positions closed out.
    if results.len() != 0 {
        let users = store::get_users_with_positions_in_market(&handler.pool, market.id).await?;

        let mut profits: HashMap<i64, (&DbUser, Currency)> = users
            .iter()
            .map(|u| (u.id, (u, Currency::from(0))))
            .collect();

        for r in &results {
            let entry = profits.entry(r.user.id);
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
            format_market_id(market.id),
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

    // Refetch and re-render market message.
    let market = store::get_market_by_id(&handler.pool, market_id)
        .await?
        .ok_or(anyhow!("market {market_id} not found"))?;
    let instruments =
        store::get_instruments_with_share_counts_for_market(&handler.pool, input.market.id).await?;
    let market_message = render_market_message(&market, &input.market_owner, instruments.iter());

    let msg_id = input
        .market
        .message_id
        .as_ref()
        .ok_or(anyhow!(
            "message ID not found for market {}",
            input.market.id
        ))?
        .parse::<u64>()?;
    let channel_id = input
        .market
        .channel_id
        .as_ref()
        .ok_or(anyhow!(
            "channel ID not found for market {}",
            input.market.id
        ))?
        .parse::<u64>()?;

    let mut msg = ctx
        .http
        .get_message(GenericChannelId::new(channel_id), MessageId::new(msg_id))
        .await?;

    msg.edit(&ctx.http, EditMessage::new().components(market_message))
        .await?;

    Ok(())
}
