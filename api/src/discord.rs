use anyhow::anyhow;
use common::{
    discord_ui::{render_details_message, render_market_message},
    store::{
        instrument::InstrumentStore,
        market::FullMarket,
        position::PositionStore,
        user::DbUser,
    },
};
use serenity::all::{
    CreateMessage, EditMessage, GenericChannelId, MessageId, ThreadId, Token, UserId,
};
use serenity::http::Http;

use crate::state::AppState;

pub fn new_http() -> Http {
    Http::new(Token::from_env("DISCORD_TOKEN").expect("DISCORD_TOKEN should be set"))
}

fn parse_id(value: &Option<String>, what: &str, market_id: i32) -> anyhow::Result<u64> {
    Ok(value
        .as_ref()
        .ok_or_else(|| anyhow!("{what} not found for market {market_id}"))?
        .parse::<u64>()?)
}

pub async fn sync_market_after_trade(
    state: &AppState,
    market: &FullMarket,
    user: &DbUser,
    thread_message: &str,
) -> anyhow::Result<()> {
    let market_row = &market.row;
    let mut conn = state.db.conn().await?;

    let instruments = state
        .instrument_store
        .get_instruments_with_share_counts_for_market(&mut conn, market_row.id)
        .await?;
    let all_positions = state
        .position_store
        .get_all_market_positions(&mut conn, market_row.id)
        .await?;
    drop(conn);

    let http = &state.http;

    let channel_id = GenericChannelId::new(parse_id(
        &market_row.channel_id,
        "channel ID",
        market_row.id,
    )?);
    let message_id = MessageId::new(parse_id(
        &market_row.message_id,
        "message ID",
        market_row.id,
    )?);

    let components = render_market_message(market_row, &market.owner, instruments.iter());
    channel_id
        .edit_message(http, message_id, EditMessage::new().components(components))
        .await?;

    let Some(thread_id) = &market_row.thread_id else {
        return Ok(());
    };
    let thread_id = GenericChannelId::new(thread_id.parse::<u64>()?);

    if let Ok(details_msg_id) = parse_id(&market_row.details_msg_id, "details ID", market_row.id) {
        thread_id
            .edit_message(
                http,
                MessageId::new(details_msg_id),
                EditMessage::new().content(render_details_message(&instruments, &all_positions)),
            )
            .await?;
    }

    thread_id
        .send_message(http, CreateMessage::new().content(thread_message))
        .await?;

    let user_id = user.discord_id.parse::<u64>()?;
    http.add_thread_channel_member(ThreadId::from(thread_id.get()), UserId::new(user_id))
        .await?;

    Ok(())
}
