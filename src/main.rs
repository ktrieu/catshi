use std::env;

use serenity::{
    Client,
    all::{
        CommandInteraction, Context, CreateInteractionResponse, CreateInteractionResponseMessage,
        EventHandler, GatewayIntents, GuildId, Interaction, ModalInteraction, Ready,
    },
    async_trait,
};
use sqlx::SqlitePool;

use crate::store::User;

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

impl Handler {
    async fn handle_command(&self, ctx: &Context, command: CommandInteraction) {
        let result: anyhow::Result<()> = async {
            let user = store::get_user_by_discord_id(&self.pool, &command.user.id).await?;

            let mut tx = self.pool.begin().await?;
            let _user: User = match user {
                Some(user) => anyhow::Ok(user),
                None => {
                    // Automatically register if we haven't seen them before.
                    let server_nickname = command.user.nick_in(ctx, self.guild_id).await;
                    let name: &str = server_nickname.as_ref().unwrap_or(&command.user.name);
                    let user_id = &command.user.id.to_string();

                    let user = store::insert_user_if_not_exists(&mut tx, &user_id, name).await?;

                    // We're only in this branch if user query above didn't return a user. Possible race condition, will fix later.
                    Ok(user.expect("user should have been created"))
                }
            }?;
            tx.commit().await?;

            match command.data.name.as_str() {
                command::market::NAME => command::market::run(&ctx, self, &command).await?,
                _ => {}
            };

            Ok(())
        }
        .await;

        if let Err(e) = result {
            let err_send_result = command
                .create_response(
                    ctx,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content(format!("Internal command error: {}", e.to_string())),
                    ),
                )
                .await;

            if let Err(_) = err_send_result {
                // Dang, this really sucks. Don't do anything for now, maybe we'll add logging later.
            }
        };
    }

    async fn handle_modal(&self, ctx: &Context, modal: ModalInteraction) {
        let result: anyhow::Result<()> = async {
            let user = store::get_user_by_discord_id(&self.pool, &modal.user.id).await?;

            let mut tx = self.pool.begin().await?;
            let user: User = match user {
                Some(user) => anyhow::Ok(user),
                None => {
                    // Automatically register if we haven't seen them before.
                    let server_nickname = modal.user.nick_in(ctx, self.guild_id).await;
                    let name: &str = server_nickname.as_ref().unwrap_or(&modal.user.name);
                    let user_id = &modal.user.id.to_string();

                    let user = store::insert_user_if_not_exists(&mut tx, &user_id, name).await?;

                    // We're only in this branch if user query above didn't return a user. Possible race condition, will fix later.
                    Ok(user.expect("user should have been created"))
                }
            }?;
            tx.commit().await?;

            match modal.data.custom_id.as_str() {
                command::market::MODAL_ID => {
                    command::market::modal_submit(ctx, &self, &modal, &user).await?
                }
                _ => {}
            }

            Ok(())
        }
        .await;

        if let Err(e) = result {
            let err_send_result = modal
                .create_response(
                    ctx,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content(format!("Internal command error: {}", e.to_string())),
                    ),
                )
                .await;

            if let Err(_) = err_send_result {
                // Dang, this really sucks. Don't do anything for now, maybe we'll add logging later.
            }
        };
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, _ready: Ready) {
        self.guild_id
            .set_commands(ctx, vec![command::market::create()])
            .await
            .expect("command registration should succeed");
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(command) => self.handle_command(&ctx, command).await,
            Interaction::Modal(modal) => self.handle_modal(&ctx, modal).await,
            _ => {}
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
