use anyhow::anyhow;
use serenity::all::{
    CreateInputText, CreateLabel, CreateModal, CreateModalComponent, InputTextStyle,
    ModalInteraction,
};

use crate::ui;

pub const MODAL_ID: &'static str = "market-create-modal";

const MODAL_DESC_ID: &'static str = "market-create-desc";
const MODAL_OPTS_ID: &'static str = "market-create-opts";

pub fn create_modal() -> CreateModal<'static> {
    let desc_label = CreateLabel::input_text(
        "Question".to_string(),
        CreateInputText::new(InputTextStyle::Paragraph, MODAL_DESC_ID),
    );

    let opts_label = CreateLabel::input_text(
        "Options".to_string(),
        CreateInputText::new(InputTextStyle::Paragraph, MODAL_OPTS_ID).value("Yes\nNo"),
    )
    .description(
        "Options for this market, one per line. Not too many or I will have to add validation."
            .to_string(),
    );

    let components = vec![
        CreateModalComponent::Label(desc_label),
        CreateModalComponent::Label(opts_label),
    ];

    CreateModal::new(MODAL_ID, "New market").components(components)
}

pub struct CreateModalValues<'resp> {
    pub description: &'resp str,
    pub options: Vec<&'resp str>,
}

pub fn extract_create_modal_values(
    modal: &'_ ModalInteraction,
) -> anyhow::Result<CreateModalValues<'_>> {
    let values = ui::extract_modal_values(&modal);

    let description = values
        .get(MODAL_DESC_ID)
        .ok_or(anyhow!("modal description field not found"))?;

    let options = values
        .get(MODAL_OPTS_ID)
        .ok_or(anyhow!("modal options field not present"))?
        .split("\n")
        .collect();

    Ok(CreateModalValues {
        description,
        options,
    })
}
