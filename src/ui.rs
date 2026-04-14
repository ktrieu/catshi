use std::collections::HashMap;

use serenity::all::{Label, ModalInteraction};

use crate::store::{Instrument, Market};

pub mod market_create_modal;
pub mod market_message;
pub mod trade_flow;

fn extract_label_value(label: &Label) -> Option<(&str, &str)> {
    // For now, we only use input text values.
    // TODO: Update the return of extract_modal_values to handle text boxes other than strings, if needed,
    // and then return other types of input components.
    match &label.component {
        serenity::all::LabelComponent::InputText(input_text) => {
            let val = input_text.value.as_deref();
            val.map(|val| (input_text.custom_id.as_str(), val))
        }
        serenity::all::LabelComponent::SelectMenu(menu) => {
            let val = menu.values.get(0);
            val.map(|val| (menu.custom_id.as_str(), val.as_str()))
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

const TABULATE_ROW_SEPARATOR: char = '|';

pub fn tabulate<const N: usize>(rows: Vec<[String; N]>) -> String {
    let num_cols = N;
    let num_rows = rows.len();

    let mut sizes: [usize; N] = [0; N];

    for r in &rows {
        for (i, s) in r.iter().enumerate() {
            sizes[i] = sizes[i].max(s.len() + 2);
        }
    }

    // Sum of all column sizes + all our separators (col count + 1)
    let row_length = sizes.iter().sum::<usize>() + (num_cols + 1);

    // Add num rows for the newlines and 6 more for the Discord monospace backticks.
    let table_length = (row_length * num_rows) + num_rows + 6;

    let mut tabulated = String::with_capacity(table_length);
    tabulated.push_str("```");

    for r in &rows {
        for (i, cell) in r.iter().enumerate() {
            // Minus 1 because we added one space on the left
            let cell_length = sizes[i] - 1;
            let formatted = format!(
                "{TABULATE_ROW_SEPARATOR} {cell:<width$}",
                width = cell_length
            );
            tabulated += &formatted
        }
        // Add the last row separator and the new line
        tabulated.push(TABULATE_ROW_SEPARATOR);
        tabulated.push('\n');
    }

    tabulated.push_str("```");
    tabulated
}
