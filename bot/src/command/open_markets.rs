use common::store::CatshiDb;
use serenity::all::{CommandInteraction, Context, CreateCommand};

use crate::{
    bot::Handler,
    store::market::{MarketState, MarketStore},
    ui, utils,
};

pub const NAME: &'static str = "open_markets";

pub fn create() -> CreateCommand<'static> {
    CreateCommand::new(NAME).description("list all open markets")
}

pub async fn render(db: &CatshiDb, market_store: &impl MarketStore) -> anyhow::Result<String> {
    let mut conn = db.conn().await?;
    let markets = market_store
        .get_markets_by_state(&mut conn, MarketState::Open)
        .await?;

    let mut lines: Vec<String> = vec![format!("## OPEN MARKETS ({})", markets.len())];

    for m in markets {
        let link = ui::get_market_message_link(&m).await?;

        let l = format!(
            "**{}**  {} - {}",
            ui::format_market_id(m.id),
            ui::truncate_text(&m.description, 17),
            link,
        );
        lines.push(l);
    }

    Ok(lines.join("\n"))
}

pub async fn run(
    ctx: &Context,
    handler: &Handler,
    command: &CommandInteraction,
) -> anyhow::Result<()> {
    let resp = render(&handler.db, &handler.market_store).await?;

    utils::send_long_message_safe(ctx, command, &resp, false).await
}
