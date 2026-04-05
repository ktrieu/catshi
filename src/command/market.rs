use serenity::all::{
    ButtonStyle, CommandInteraction, Context, CreateActionRow, CreateButton, CreateCommand,
    CreateComponent, CreateInteractionResponse, CreateMessage, CreateSeparator, CreateTextDisplay,
    MessageFlags, ModalInteraction,
};

use crate::{
    Handler,
    store::{self, DbUser, Instrument, Market},
    ui::market_create_modal,
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

fn get_trade_button_id(instrument: &Instrument, action: &str) -> String {
    format!("trade_button_{}_{}", action, instrument.id)
}

fn render_market_message<'a>(
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
            CreateButton::new(get_trade_button_id(i, "buy"))
                .label("Buy")
                .style(ButtonStyle::Success),
            CreateButton::new(get_trade_button_id(i, "sell"))
                .label("Sell")
                .style(ButtonStyle::Danger),
        ];

        let row = CreateActionRow::buttons(buttons);
        components.push(CreateComponent::ActionRow(row));
    }

    components
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
    let resp_components = render_market_message(&new_market, &instruments);
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
