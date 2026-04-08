use std::collections::HashMap;

use serenity::all::{Label, ModalInteraction};

use crate::store::{Instrument, Market};

pub mod market_create_modal;
pub mod market_message;

fn extract_label_value(label: &Label) -> Option<(&str, &str)> {
    // For now, we only use input text values.
    // TODO: Update the return of extract_modal_values to handle text boxes other than strings, if needed,
    // and then return other types of input components.
    match &label.component {
        serenity::all::LabelComponent::InputText(input_text) => {
            let val = input_text.value.as_deref();
            val.map(|val| (input_text.custom_id.as_str(), val))
        }
        _ => None,
    }
}

// Extracts all the inputs from a modal to a map of ID -> value
pub fn extract_modal_values(modal: &'_ ModalInteraction) -> HashMap<&str, &str> {
    modal
        .data
        .components
        .iter()
        .filter_map(|c| match c {
            serenity::all::ModalComponent::TextDisplay(_) => None,
            serenity::all::ModalComponent::Label(label) => extract_label_value(label),
            _ => None,
        })
        .collect()
}

pub fn format_market_id(id: i64) -> String {
    format!("#{id:04}")
}

pub fn instrument_display_text(instrument: &Instrument, market: &Market) -> String {
    format!(
        "{} (market {})",
        instrument.name,
        format_market_id(market.id)
    )
}
