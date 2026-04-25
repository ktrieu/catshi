use std::{env, sync::Arc};

use log::{error, info, warn};
use serenity::{
    Client,
    all::{
        CommandInteraction, ComponentInteraction, Context, EventHandler, FullEvent, GatewayIntents,
        GuildId, Interaction, ModalInteraction, Token, User,
    },
    async_trait,
};
use simplelog::{ColorChoice, Config, LevelFilter, TermLogger, TerminalMode};
use sqlx::SqlitePool;

use crate::{
    command::{resolve::parse_market_resolve_modal_id, trade::parse_trade_button_id},
    currency::Currency,
    store::{transfer::TransferSource, user::DbUser},
    ui::{
        blackjack::parse_blackjack_action,
        market_message::{parse_market_details_button_id, parse_market_resolve_button_id},
        trade_flow::parse_trade_modal_id,
    },
};

mod blackjack;
mod command;
mod currency;
mod portfolio;
mod store;
mod trade;
mod ui;
mod utils;

pub async fn init_db(url: &str) -> anyhow::Result<SqlitePool> {
    let pool = SqlitePool::connect(url).await?;
    info!("Connected to database {url}");

    sqlx::migrate!("./migrations").run(&pool).await?;
    info!("Migrations run");

    Ok(pool)
}

struct Handler {
    pub guild_id: GuildId,
    pub pool: SqlitePool,
}

// Everyone starts with 20 YP.
const INITIAL_BALANCE: Currency = Currency::new_yp(20);

impl Handler {
    async fn authenticate(&self, ctx: &Context, discord_user: &User) -> anyhow::Result<DbUser> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;

        let user = store::user::get_user_by_discord_id(&self.pool, &discord_user.id).await?;
        let user = match user {
            Some(user) => anyhow::Ok(user),
            None => {
                let system_user = store::user::get_system_user(&mut *tx).await?;
                // Automatically register if we haven't seen them before.
                let server_nickname = discord_user.nick_in(ctx, self.guild_id).await;

                let name = server_nickname
                    .as_deref()
                    .unwrap_or(discord_user.name.as_str());
                let user_id = &discord_user.id.to_string();

                let user = store::user::insert_user_if_not_exists(
                    &mut *tx,
                    &user_id,
                    &name,
                    Currency::from(0),
                )
                .await?;

                // Credit the user their initial balance.
                let transfer = trade::create_system_credit(
                    &user,
                    &system_user,
                    INITIAL_BALANCE,
                    "Initial account funding. Have fun.",
                    TransferSource::Deposit,
                );

                store::transfer::persist_transfer(&mut tx, &transfer).await?;

                let user = store::user::get_user_by_id(&mut *tx, user.id).await?;

                // We're only in this branch if user query above didn't return a user.
                Ok(user)
            }
        }?;

        tx.commit().await?;

        Ok(user)
    }

    async fn ready(&self, ctx: &Context) {
        self.guild_id
            .set_commands(
                &ctx.http,
                &[
                    command::blackjack::create(),
                    command::market::create(),
                    command::leaderboard::create(),
                    command::transfer::create(),
                ],
            )
            .await
            .expect("command registration should succeed");
    }

    async fn interaction_create(&self, ctx: &Context, interaction: &Interaction) {
        if let Some(interaction_guild_id) = interaction.guild_id()
            && self.guild_id != interaction_guild_id
        {
            info!(
                "Skipping interaction not targeted for this instance's guild. Interaction guild {} did not match our guild {}",
                interaction_guild_id, self.guild_id
            );
            return;
        }

        let result = match &interaction {
            Interaction::Command(command) => self.handle_command(&ctx, command).await,
            Interaction::Modal(modal) => self.handle_modal(&ctx, modal).await,
            Interaction::Component(component) => self.handle_component(&ctx, component).await,
            _ => {
                warn!("Unrecognized interaction {:?}", interaction.kind());
                Ok(())
            }
        };

        if let Err(command_error) = result {
            let msg = format!("Internal error: {}", command_error.to_string());
            let response = utils::text_interaction_response(&msg, true);
            let send_result = match &interaction {
                Interaction::Command(command) => command.create_response(&ctx.http, response).await,
                Interaction::Modal(modal) => modal.create_response(&ctx.http, response).await,
                Interaction::Component(component) => {
                    component.create_response(&ctx.http, response).await
                }
                _ => Ok(()),
            };

            if let Err(send_error) = send_result {
                error!(
                    "Error sending internal error response: {send_error}. Original error: {command_error}"
                );
            }
        }
    }

    async fn handle_command(
        &self,
        ctx: &Context,
        command: &CommandInteraction,
    ) -> anyhow::Result<()> {
        let user = self.authenticate(ctx, &command.user).await?;

        match command.data.name.as_str() {
            command::blackjack::NAME => {
                command::blackjack::run(&ctx, self, &user, &command).await?
            }
            command::market::NAME => command::market::run(&ctx, self, &command).await?,
            command::leaderboard::NAME => command::leaderboard::run(&ctx, self, &command).await?,
            command::transfer::NAME => command::transfer::run(&ctx, self, &user, &command).await?,
            _ => {
                warn!("Unrecognized command {}", command.data.name);
            }
        };

        Ok(())
    }

    async fn handle_modal(&self, ctx: &Context, modal: &ModalInteraction) -> anyhow::Result<()> {
        let user = self.authenticate(ctx, &modal.user).await?;

        if let Some((trade_action, instrument_id)) = parse_trade_modal_id(&modal.data.custom_id) {
            command::trade::trade(ctx, &self, &user, modal, trade_action, instrument_id).await?;
        } else if let Some(market_id) = parse_market_resolve_modal_id(&modal.data.custom_id) {
            command::resolve::resolve(ctx, &self, market_id, modal, &user).await?;
        } else if modal.data.custom_id == ui::market_create_modal::MODAL_ID {
            command::market::modal_submit(ctx, &self, &modal, &user).await?
        } else {
            warn!("Unrecognized modal interaction {}", modal.data.custom_id);
        };

        Ok(())
    }

    async fn handle_component(
        &self,
        ctx: &Context,
        component: &ComponentInteraction,
    ) -> anyhow::Result<()> {
        let user = self.authenticate(ctx, &component.user).await?;

        if let Some((action, instrument_id)) = parse_trade_button_id(&component.data.custom_id) {
            command::trade::initiate_trade(ctx, &self, &user, component, action, instrument_id)
                .await?;
        } else if let Some(market_id) = parse_market_resolve_button_id(&component.data.custom_id) {
            command::resolve::initiate_resolve(ctx, &self, market_id, &component, &user).await?;
        } else if let Some(market_id) = parse_market_details_button_id(&component.data.custom_id) {
            command::market_details::view_market_details(ctx, &self, market_id, component).await?;
        } else if let Some(action) = parse_blackjack_action(&component.data.custom_id) {
            command::blackjack::interact(ctx, &self, &user, component, action).await?;
        } else {
            warn!(
                "Unrecognized component interaction {}",
                component.data.custom_id
            );
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
    TermLogger::init(
        LevelFilter::Info,
        Config::default(),
        TerminalMode::Stdout,
        ColorChoice::Auto,
    )
    .expect("logger initialization should succeed");

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

    info!("Starting client");
    client.start().await.expect("client start should succeed");
}
