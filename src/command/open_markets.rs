use serenity::all::{CommandInteraction, Context, CreateCommand};

use crate::{
    Handler,
    store::{self, market::MarketState},
    ui, utils,
};

pub const NAME: &'static str = "open_markets";

pub fn create() -> CreateCommand<'static> {
    CreateCommand::new(NAME).description("list all open markets")
}

pub async fn run(
    ctx: &Context,
    handler: &Handler,
    command: &CommandInteraction,
) -> anyhow::Result<()> {
    let markets =
        store::market::get_markets_by_state(&mut *handler.pool.acquire().await?, MarketState::Open)
            .await?;

    let mut lines: Vec<String> = vec![format!("## OPEN MARKETS ({})", markets.len())];

    for m in markets {
        let message = ui::get_market_message(&m, &ctx).await?;

        let l = format!(
            "**{}**  {} - {}",
            ui::format_market_id(m.id),
            ui::truncate_text(&m.description, 17),
            message.link(),
        );
        lines.push(l);
    }

    let resp = lines.join("\n");

    command
        .create_response(&ctx.http, utils::text_interaction_response(&resp, false))
        .await?;

    Ok(())
}
