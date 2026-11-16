use std::{env, sync::Arc};

use common::store::CatshiDb;
use log::info;
use serenity::{
    Client,
    all::{Context, EventHandler, FullEvent, GatewayIntents, GenericChannelId, Message, Token},
    async_trait,
};
use simplelog::{ColorChoice, Config, LevelFilter, TermLogger, TerminalMode};

struct Handler {
    channel_id: GenericChannelId,
    #[allow(dead_code)]
    db: CatshiDb,
}

impl Handler {
    async fn message(&self, message: &Message) {
        if message.channel_id != self.channel_id || message.author.bot() {
            return;
        }

        info!(
            "Message {} from {}: {}",
            message.id, message.author.name, message.content
        );
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn dispatch(&self, _ctx: &Context, event: &FullEvent) {
        match event {
            FullEvent::Ready { data_about_bot, .. } => {
                info!(
                    "Connected as {}, watching channel {}",
                    data_about_bot.user.name, self.channel_id
                );
            }
            FullEvent::Message { new_message, .. } => self.message(new_message).await,
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() {
    TermLogger::init(
        LevelFilter::Info,
        Config::default(),
        TerminalMode::Stdout,
        ColorChoice::Auto,
    )
    .expect("logger initialization should succeed");

    dotenvy::dotenv().ok();

    let pg_url = env::var("POSTGRES_URL").expect("POSTGRES_URL should be set");
    let db = CatshiDb::new(&pg_url)
        .await
        .expect("DB initialization should succeed");

    let channel_id = env::var("CF_ARCHIVE_CHANNEL_ID")
        .expect("CF_ARCHIVE_CHANNEL_ID should be set")
        .parse::<u64>()
        .expect("CF_ARCHIVE_CHANNEL_ID should be a valid u64");
    let channel_id = GenericChannelId::new(channel_id);

    let handler = Arc::new(Handler { channel_id, db });

    let discord_token =
        Token::from_env("DISCORD_TOKEN").expect("DISCORD_TOKEN should be present in env.");
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(discord_token, intents)
        .event_handler(handler)
        .await
        .expect("client creation should succeed");

    info!("Starting client");
    client.start().await.expect("client start should succeed");
}
