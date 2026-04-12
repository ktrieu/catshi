use serenity::all::{
    ComponentInteraction, Context, CreateInteractionResponse, CreateLabel, CreateModal,
    CreateModalComponent, CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption,
    ModalInteraction,
};

use crate::{
    Handler,
    store::{self, DbUser, Market, get_instruments_with_share_counts_for_market},
    ui::format_market_id,
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

    Ok(())
}
