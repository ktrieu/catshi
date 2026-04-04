use serenity::all::{
    ButtonStyle, CommandInteraction, Context, CreateActionRow, CreateButton, CreateCommand,
    CreateComponent, CreateInputText, CreateInteractionResponse, CreateLabel, CreateMessage,
    CreateModal, CreateModalComponent, CreateSeparator, CreateTextDisplay, InputTextStyle,
    MessageFlags, ModalComponent, ModalInteraction,
};

use crate::{
    Handler,
    store::{self, DbUser, Instrument, Market},
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
    options: Vec<&'resp str>,
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
        options,
    })
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
    let values = extract_create_modal_values(modal)
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
