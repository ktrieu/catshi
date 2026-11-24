use std::env;

use anyhow::Context;
use serenity::all::{GenericChannelId, Token};
use serenity::http::Http;

use crate::command::{
    leaderboard::{self, LeaderboardField},
    open_markets,
};
use crate::utils;
use common::store::{
    CatshiDb, instrument::DbInstrumentStore, market::DbMarketStore, position::DbPositionStore,
    transfer::DbTransferStore,
};

pub async fn run() -> anyhow::Result<()> {
    let pg_url = env::var("POSTGRES_URL").context("POSTGRES_URL should be set")?;
    let channel_id = env::var("DAILY_DIGEST_CHANNEL_ID")
        .context("DAILY_DIGEST_CHANNEL_ID should be set")?
        .parse::<u64>()
        .context("DAILY_DIGEST_CHANNEL_ID should be a valid u64")?;
    let channel = GenericChannelId::new(channel_id);

    let http = Http::new(Token::from_env("DISCORD_TOKEN")?);
    let db = CatshiDb::new(&pg_url).await?;

    let markets = open_markets::render(&db, &DbMarketStore {}).await?;
    let leaderboard = leaderboard::render(
        &db,
        &DbTransferStore {},
        &DbPositionStore {},
        &DbInstrumentStore {},
        LeaderboardField::NetWorth,
    )
    .await?;

    utils::send_long_channel_message(&http, channel, &markets).await?;
    utils::send_long_channel_message(
        &http,
        channel,
        &format!("## NET WORTH LEADERBOARD\n{leaderboard}"),
    )
    .await?;

    log::info!("posted daily digest to channel {channel_id}");

    Ok(())
}
