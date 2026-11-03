pub use common::discord_ui::{
    render_details_message, render_market_message,
};

pub fn parse_market_resolve_button_id(id: &str) -> Option<i32> {
    let components: Vec<&str> = id.split('|').collect();

    if components.len() != 2 {
        return None;
    };

    if components[0] != "resolve_button" {
        return None;
    }

    components[1].parse::<i32>().ok()
}
