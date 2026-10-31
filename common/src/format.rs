use crate::store::{instrument::Instrument, market::Market};

pub fn format_market_id(id: i32) -> String {
    format!("#{id:04}")
}

pub fn instrument_display_text(instrument: &Instrument, market: &Market) -> String {
    format!(
        "{} (market {})",
        instrument.name,
        format_market_id(market.id)
    )
}
