use std::{env, sync::LazyLock};

use include_dir::{Dir, include_dir};
use rusqlite_migration::Migrations;
use serenity::{
    Client,
    all::{Context, EventHandler, GatewayIntents, Ready},
    async_trait,
};
use tokio_rusqlite_new::Connection;

use crate::bot::Bot;

static MIGRATIONS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/migrations");

// Define migrations. These are applied atomically.
static MIGRATIONS: LazyLock<Migrations<'static>> =
    LazyLock::new(|| Migrations::from_directory(&MIGRATIONS_DIR).unwrap());

pub async fn init_db(path: &str) -> anyhow::Result<Connection> {
    let conn = Connection::open(path).await?;

    // Update the database schema, atomically
    conn.call_unwrap(|conn| MIGRATIONS.to_latest(conn)).await?;

    Ok(conn)
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

    let db_path = env::var("DB_PATH").expect("DB_PATH should be set");
    let conn = init_db(&db_path)
        .await
        .expect("db initialization should succeed");

    let discord_token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN should be set");
    let guild_id = env::var("GUILD_ID").expect("GUILD_ID should be set");

    let mut client = Client::builder(discord_token, GatewayIntents::empty())
        .event_handler(Handler {})
        .await
        .expect("client creation should succeed");

    let bot = Bot::new(&guild_id, conn).expect("bot initialization should succeed");
    {
        let mut data = client.data.write().await;
        data.insert::<Bot>(bot);
    }

    client.start().await.expect("client start should succeed");
}
