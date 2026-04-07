use std::{env, sync::Arc};

use anyhow::anyhow;
use serenity::{
    Client,
    all::{
        CommandInteraction, ComponentInteraction, Context, EventHandler, FullEvent, GatewayIntents,
        GuildId, Interaction, ModalInteraction, Token, User,
    },
    async_trait,
};
use sqlx::SqlitePool;

use crate::{
    store::DbUser,
    ui::market_message::{self, TradeAction},
};

mod command;
mod currency;
mod store;
mod trade;
mod ui;
mod utils;

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
    async fn authenticate(&self, ctx: &Context, discord_user: &User) -> anyhow::Result<DbUser> {
        let mut tx = self.pool.begin().await?;

        let user = store::get_user_by_discord_id(&self.pool, &discord_user.id).await?;
        let user = match user {
            Some(user) => anyhow::Ok(user),
            None => {
                // Automatically register if we haven't seen them before.
                let server_nickname = discord_user.nick_in(ctx, self.guild_id).await;

                let name = server_nickname
                    .as_deref()
                    .unwrap_or(discord_user.name.as_str());
                let user_id = &discord_user.id.to_string();

                let user = store::insert_user_if_not_exists(&mut *tx, &user_id, &name).await?;

                // We're only in this branch if user query above didn't return a user.
                Ok(user.expect("user should have been created"))
            }
        }?;

        tx.commit().await?;

        Ok(user)
    }

    async fn ready(&self, ctx: &Context) {
        self.guild_id
            .set_commands(&ctx.http, &[command::market::create()])
            .await
            .expect("command registration should succeed");
    }

    async fn interaction_create(&self, ctx: &Context, interaction: &Interaction) {
        let result = match &interaction {
            Interaction::Command(command) => self.handle_command(&ctx, command).await,
            Interaction::Modal(modal) => self.handle_modal(&ctx, modal).await,
            Interaction::Component(component) => self.handle_component(&ctx, component).await,
            _ => Ok(()),
        };

        if let Err(e) = result {
            let msg = format!("Internal error: {}", e.to_string());
            let response = utils::text_interaction_response(&msg, true);
            let send_result = match &interaction {
                Interaction::Command(command) => command.create_response(&ctx.http, response).await,
                Interaction::Modal(modal) => modal.create_response(&ctx.http, response).await,
                Interaction::Component(component) => {
                    component.create_response(&ctx.http, response).await
                }
                _ => Ok(()),
            };

            if let Err(_) = send_result {
                // Dang, that sucks. Just log this error we have logging.
            }
        }
    }

    async fn handle_command(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
    ) -> anyhow::Result<()> {
        let _user = self.authenticate(ctx, &command.user).await?;

        match command.data.name.as_str() {
            command::market::NAME => command::market::run(&ctx, self, &command).await?,
            _ => {}
        };

        Ok(())
    }

    async fn handle_modal(&self, ctx: &Context, modal: &ModalInteraction) -> anyhow::Result<()> {
        let user = self.authenticate(ctx, &modal.user).await?;

        match modal.data.custom_id.as_str() {
            ui::market_create_modal::MODAL_ID => {
                command::market::modal_submit(ctx, &self, &modal, &user).await?
            }
            _ => {}
        };

        Ok(())
    }

    async fn handle_component(
        &self,
        ctx: &Context,
        component: &ComponentInteraction,
    ) -> anyhow::Result<()> {
        let user = self.authenticate(ctx, &component.user).await?;

        if let Some((trade_action, instrument_id)) =
            market_message::parse_trade_button_id(&component.data.custom_id)
        {
            if trade_action == TradeAction::Buy {
                // We'll handle the UI later - for now assume we only buy one share.
                let quantity = 1;

                let instrument = store::get_instrument_by_id(&self.pool, instrument_id)
                    .await?
                    .ok_or(anyhow!("instrument not found"))?;

                let result = trade::buy(&self.pool, quantity, &instrument, &user).await?;

                let msg = format!(
                    "Bought {} shares of instrument {} for {}",
                    quantity, instrument_id, result.total_price
                );
                component
                    .create_response(&ctx.http, utils::text_interaction_response(&msg, true))
                    .await?;
            } else {
                let quantity = 1;

                let instrument = store::get_instrument_by_id(&self.pool, instrument_id)
                    .await?
                    .ok_or(anyhow!("instrument not found"))?;

                let result = trade::sell(&self.pool, quantity, &instrument, &user).await?;

                let msg = format!(
                    "Sold {} shares of instrument {} for {}. Profit {}",
                    quantity,
                    instrument_id,
                    result.shares_price,
                    result.profit()
                );
                component
                    .create_response(&ctx.http, utils::text_interaction_response(&msg, true))
                    .await?;
            }
        }

        Ok(())
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn dispatch(&self, ctx: &Context, event: &FullEvent) {
        match event {
            FullEvent::Ready { .. } => self.ready(&ctx).await,
            FullEvent::InteractionCreate { interaction, .. } => {
                self.interaction_create(ctx, interaction).await
            }
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().expect(".env loading should succeed");

    let url = env::var("DATABASE_URL").expect("DATABASE_URL should be set");
    let pool = init_db(&url)
        .await
        .expect("db initialization should succeed");

    let guild_id = env::var("GUILD_ID")
        .expect("GUILD_ID should be set")
        .parse::<u64>()
        .expect("GUILD_ID should be a valid u64");
    let guild_id = GuildId::new(guild_id);

    let handler = Arc::new(Handler { guild_id, pool });

    let discord_token =
        Token::from_env("DISCORD_TOKEN").expect("DISCORD_TOKEN should be present in env.");
    let mut client = Client::builder(discord_token, GatewayIntents::empty())
        .event_handler(handler)
        .await
        .expect("client creation should succeed");

    client.start().await.expect("client start should succeed");
}
