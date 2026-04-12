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
    ui::{extract_modal_values, format_market_id, market_message::render_market_message},
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

    let mut tx = handler.pool.begin().await?;

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

    let mut profits: HashMap<i64, Currency> = HashMap::new();

    for r in &results {
        let entry = profits.entry(r.user.id);

        let user_profit = entry.or_insert(Currency::from(0));
        *user_profit = *user_profit + r.profit();
    }

    let mut profits: Vec<(i64, Currency)> = profits.into_iter().collect();
    profits.sort_by_key(|(_, profit)| Reverse(*profit));

    // TODO: replace this with proper table production code.
    let rows: Vec<String> = profits
        .iter()
        .map(|(user_id, profit)| format!("{user_id} {profit}"))
        .collect();

    modal
        .create_response(
            &ctx.http,
            utils::text_interaction_response(&rows.join("\n"), false),
        )
        .await?;

    // Refetch and re-render market message.
    let market = store::get_market_by_id(&handler.pool, market_id)
        .await?
        .ok_or(anyhow!("market {market_id} not found"))?;
    let instruments =
        store::get_instruments_with_share_counts_for_market(&handler.pool, input.market.id).await?;
    let market_message = render_market_message(&market, instruments.iter());

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
