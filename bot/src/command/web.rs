use std::time::Duration;

use serenity::all::{CommandInteraction, Context, CreateCommand};

use common::store::{login_token::LoginTokenStore, user::DbUser};

use crate::{bot::Handler, utils};

pub const NAME: &'static str = "web";

// Magic links are single use, so keep the window short.
const TOKEN_EXPIRY: Duration = Duration::from_secs(15 * 60);

pub fn create() -> CreateCommand<'static> {
    CreateCommand::new(NAME).description("get a login link for the catshi website")
}

pub async fn run(
    ctx: &Context,
    handler: &Handler,
    user: &DbUser,
    command: &CommandInteraction,
) -> anyhow::Result<()> {
    let mut conn = handler.db.conn().await?;

    let token = handler
        .login_token_store
        .create(&mut conn, user, TOKEN_EXPIRY)
        .await?;

    let url = format!("{}/login/{}", handler.frontend_base_url, token.token);

    command
        .create_response(&ctx.http, utils::text_interaction_response(&url, true))
        .await?;

    Ok(())
}
