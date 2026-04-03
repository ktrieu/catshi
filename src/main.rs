use std::env;

use serenity::{
    Client,
    all::{Context, EventHandler, GatewayIntents, Ready},
    async_trait,
};
use sqlx::SqlitePool;

use crate::bot::Bot;

pub async fn init_db(url: &str) -> anyhow::Result<SqlitePool> {
    let pool = SqlitePool::connect(url).await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, _ready: Ready) {
        println!("connected");
    }
}

mod bot;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().expect(".env loading should succeed");

    let url = env::var("DATABASE_URL").expect("DATABASE_URL should be set");
    let pool = init_db(&url)
        .await
        .expect("db initialization should succeed");

    let discord_token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN should be set");
    let guild_id = env::var("GUILD_ID").expect("GUILD_ID should be set");

    let mut client = Client::builder(discord_token, GatewayIntents::empty())
        .event_handler(Handler {})
        .await
        .expect("client creation should succeed");

    let bot = Bot::new(&guild_id, pool).expect("bot initialization should succeed");
    {
        let mut data = client.data.write().await;
        data.insert::<Bot>(bot);
    }

    client.start().await.expect("client start should succeed");
}
