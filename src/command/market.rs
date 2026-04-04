use serenity::all::{
    CommandInteraction, Context, CreateCommand, CreateInputText, CreateInteractionResponse,
    CreateLabel, CreateMessage, CreateModal, CreateModalComponent, InputTextStyle, ModalComponent,
    ModalInteraction,
};

use crate::{
    Handler,
    store::{self, DbUser, Market},
};

pub const NAME: &'static str = "market";
pub const MODAL_ID: &'static str = "market-create-modal";

pub fn create() -> CreateCommand<'static> {
    CreateCommand::new(NAME).description("create a new prediction market")
}

const MODAL_DESC_ID: &'static str = "market-create-desc";

fn get_modal_opt_id(num: i64) -> String {
    format!("market-create-opt-{}", num)
}

pub async fn run(
    ctx: &Context,
    _handler: &Handler,
    command: &CommandInteraction,
) -> anyhow::Result<()> {
    let desc_label = CreateLabel::input_text(
        "Question",
        CreateInputText::new(InputTextStyle::Paragraph, MODAL_DESC_ID),
    );

    let mut rows = vec![CreateModalComponent::Label(desc_label)];

    // Add 4 potential options to the modal.
    for i in 0..4 {
        let num = i + 1;
        // We want at least two options here.
        let required = num <= 2;

        let option = CreateLabel::input_text(
            format!("Option #{}", num),
            CreateInputText::new(InputTextStyle::Short, get_modal_opt_id(num)).required(required),
        );

        rows.push(CreateModalComponent::Label(option));
    }

    let modal = CreateModal::new(MODAL_ID, "New market").components(rows);

    command
        .create_response(&ctx.http, CreateInteractionResponse::Modal(modal))
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

    for r in rows.iter() {
        if let ModalComponent::Label(label) = r {
            let (id, value) = match &label.component {
                serenity::all::LabelComponent::InputText(input_text) => (
                    input_text.custom_id.as_str(),
                    input_text
                        .value
                        .as_ref()
                        .expect("value should be set for received modal fields"),
                ),
                _ => {
                    anyhow::bail!("unsupported modal label component")
                }
            };
            values.push([id, value])
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

fn make_market_message(market: &'_ Market) -> CreateMessage<'_> {
    CreateMessage::new().content(format!("Prediction market: {}", market.description))
}

pub async fn modal_submit(
    ctx: &Context,
    handler: &Handler,
    modal: &ModalInteraction,
    user: &DbUser,
) -> anyhow::Result<()> {
    let values = extract_create_modal_values(modal)
        .inspect_err(|e| println!("{}", e))
        .map_err(|_| anyhow::anyhow!("failed to parse modal response"))?;

    let mut tx = handler.pool.begin().await?;

    let new_market = store::create_new_market(&mut *tx, values.description, user).await?;

    let resp_channel = modal.channel_id;
    let message = resp_channel
        .send_message(&ctx.http, make_market_message(&new_market))
        .await?;

    store::set_market_message_id(&mut *tx, new_market.id, message.id).await?;

    tx.commit().await?;

    Ok(())
}
