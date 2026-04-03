use std::env;

use serenity::{
    Client,
    all::{Context, EventHandler, GatewayIntents, GuildId, Interaction, Ready},
    async_trait,
};
use sqlx::SqlitePool;

mod command;
mod store;

pub async fn init_db(url: &str) -> anyhow::Result<SqlitePool> {
    let pool = SqlitePool::connect(url).await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

struct Handler {
    pub guild_id: GuildId,
    pub pool: SqlitePool,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, _ready: Ready) {
        self.guild_id
            .create_command(ctx.http, command::register::create())
            .await
            .expect("command registration should succeed");
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            match command.data.name.as_str() {
                command::register::NAME => command::register::run(&ctx, &self, &command).await,
                _ => Ok(()),
            }
            .expect("command execution should succeed!");
        };
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().expect(".env loading should succeed");

    let url = env::var("DATABASE_URL").expect("DATABASE_URL should be set");
    let pool = init_db(&url)
        .await
        .expect("db initialization should succeed");

    let discord_token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN should be set");
    let guild_id = env::var("GUILD_ID")
        .expect("GUILD_ID should be set")
        .parse::<u64>()
        .expect("GUILD_ID should be a valid u64");
    let guild_id = GuildId::new(guild_id);

    let handler = Handler { guild_id, pool };

    let mut client = Client::builder(discord_token, GatewayIntents::empty())
        .event_handler(handler)
        .await
        .expect("client creation should succeed");

    client.start().await.expect("client start should succeed");
}
