use serenity::all::{
    CreateInputText, CreateLabel, CreateModal, CreateModalComponent, InputTextStyle,
    ModalComponent, ModalInteraction,
};

pub const MODAL_ID: &'static str = "market-create-modal";
const MODAL_DESC_ID: &'static str = "market-create-desc";

fn get_modal_opt_id(num: i64) -> String {
    format!("market-create-opt-{}", num)
}

pub fn create_modal() -> CreateModal<'static> {
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

    CreateModal::new(MODAL_ID, "New market").components(rows)
}

pub struct CreateModalValues<'resp> {
    pub description: &'resp str,
    pub options: Vec<&'resp str>,
}

pub fn extract_create_modal_values(
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
