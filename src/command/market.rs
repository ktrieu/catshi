use serenity::all::{
    ActionRowComponent, CommandInteraction, Context as SerenityContext, CreateActionRow,
    CreateCommand, CreateInputText, CreateInteractionResponse, CreateMessage, CreateModal,
    InputTextStyle, ModalInteraction,
};

use crate::{
    Handler,
    store::{self, Market, User},
};

pub const NAME: &'static str = "market";
pub const MODAL_ID: &'static str = "market-create-modal";

pub fn create() -> CreateCommand {
    CreateCommand::new(NAME).description("create a new prediction market")
}

const MODAL_DESC_ID: &'static str = "market-create-desc";

fn get_modal_opt_id(num: i64) -> String {
    format!("market-create-opt-{}", num)
}

pub async fn run(
    ctx: &SerenityContext,
    _handler: &Handler,
    command: &CommandInteraction,
) -> anyhow::Result<()> {
    let desc_entry = CreateInputText::new(InputTextStyle::Paragraph, "Question", MODAL_DESC_ID);

    let mut rows = vec![CreateActionRow::InputText(desc_entry)];

    // Add 4 potential options to the modal.
    for i in 0..4 {
        let num = i + 1;
        // We want at least two options here.
        let required = num <= 2;

        let text = CreateInputText::new(
            InputTextStyle::Short,
            format!("Option #{}", num),
            get_modal_opt_id(num),
        )
        .required(required);

        rows.push(CreateActionRow::InputText(text));
    }

    let modal = CreateModal::new(MODAL_ID, "New market").components(rows);

    command
        .create_response(ctx, CreateInteractionResponse::Modal(modal))
        .await?;

    Ok(())
}

struct CreateModalValues<'resp> {
    description: &'resp str,
    _options: Vec<&'resp str>,
}

fn extract_create_modal_values(
    modal: &'_ ModalInteraction,
) -> anyhow::Result<CreateModalValues<'_>> {
    let rows = &modal.data.components;
    let mut values = Vec::new();

    if rows.len() != 5 {
        anyhow::bail!(
            "invalid number of action rows: had {} expected 5",
            rows.len()
        );
    }

    for (i, r) in rows.iter().enumerate() {
        if r.components.len() != 1 {
            anyhow::bail!("action row {i} had more than one component")
        }

        let component = &r.components[0];

        if let ActionRowComponent::InputText(text) = component {
            let value = text
                .value
                .as_ref()
                .expect("text input value should always be set on receive")
                .as_str();
            values.push([text.custom_id.as_str(), value])
        } else {
            anyhow::bail!("action row {i} was not an input text")
        }
    }

    let [desc_id, description] = values[0];
    if desc_id != MODAL_DESC_ID {
        anyhow::bail!("description text field ID was invalid");
    }

    let mut options = Vec::new();

    for i in 0..4 {
        let [opt_id, opt_value] = values[i as usize + 1];

        let num = i + 1;
        if opt_id != get_modal_opt_id(num) {
            anyhow::bail!("option text field ID was invalid");
        }

        if opt_value != "" {
            options.push(opt_value)
        }
    }

    Ok(CreateModalValues {
        description,
        _options: options,
    })
}

fn make_market_message(market: &Market) -> CreateMessage {
    CreateMessage::new().content(format!("Prediction market: {}", market.description))
}

pub async fn modal_submit(
    ctx: &SerenityContext,
    handler: &Handler,
    modal: &ModalInteraction,
    user: &User,
) -> anyhow::Result<()> {
    let values = extract_create_modal_values(modal)
        .inspect_err(|e| println!("{}", e))
        .map_err(|_| anyhow::anyhow!("failed to parse modal response"))?;

    let mut tx = handler.pool.begin().await?;

    let new_market = store::create_new_market(&mut tx, values.description, user).await?;

    let resp_channel = modal.channel_id;
    let message = resp_channel
        .send_message(ctx, make_market_message(&new_market))
        .await?;

    store::set_market_message_id(&mut tx, new_market.id, message.id).await?;

    tx.commit().await?;

    Ok(())
}
