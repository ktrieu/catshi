use serenity::all::{
    CommandInteraction, Context, CreateCommand, CreateInteractionResponse, CreateMessage,
    MessageFlags, ModalInteraction,
};

use crate::{
    Handler,
    store::{self, DbUser},
    ui::{market_create_modal, market_message},
};

pub const NAME: &'static str = "market";

pub fn create() -> CreateCommand<'static> {
    CreateCommand::new(NAME).description("create a new prediction market")
}

pub async fn run(
    ctx: &Context,
    _handler: &Handler,
    command: &CommandInteraction,
) -> anyhow::Result<()> {
    let modal = market_create_modal::create_modal();

    command
        .create_response(&ctx.http, CreateInteractionResponse::Modal(modal))
        .await?;

    Ok(())
}

pub async fn modal_submit(
    ctx: &Context,
    handler: &Handler,
    modal: &ModalInteraction,
    user: &DbUser,
) -> anyhow::Result<()> {
    let values = market_create_modal::extract_create_modal_values(modal)
        .inspect_err(|e| println!("{}", e))
        .map_err(|_| anyhow::anyhow!("failed to parse modal response"))?;

    let mut tx = handler.pool.begin().await?;

    let new_market = store::create_new_market(&mut *tx, values.description, user).await?;
    let instruments =
        store::insert_market_instruments(&mut *tx, &new_market, &values.options).await?;

    let resp_channel = modal.channel_id;
    let resp_components = market_message::render_market_message(&new_market, &instruments);
    let message = resp_channel
        .send_message(
            &ctx.http,
            CreateMessage::new()
                .components(&resp_components)
                .flags(MessageFlags::IS_COMPONENTS_V2),
        )
        .await?;

    store::set_market_message_id(&mut *tx, new_market.id, message.id, resp_channel).await?;

    tx.commit().await?;

    // Be sure to acknowledge the interaction as well so the modal closes.
    modal
        .create_response(
            &ctx.http,
            crate::utils::text_interaction_response("Market created.", true),
        )
        .await?;

    Ok(())
}
