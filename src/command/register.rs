use serenity::all::{
    CommandInteraction, Context, CreateCommand, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};

use crate::{Handler, store::insert_user_if_not_exists};

pub const NAME: &'static str = "register";

pub fn create() -> CreateCommand {
    CreateCommand::new(NAME).description("register your Discord user")
}

pub async fn run(
    ctx: &Context,
    handler: &Handler,
    interaction: &CommandInteraction,
) -> anyhow::Result<()> {
    let discord_id = interaction.user.id;

    let server_nickname = interaction.user.nick_in(ctx, handler.guild_id).await;
    let name: &str = server_nickname.as_ref().unwrap_or(&interaction.user.name);

    let mut tx = handler.pool.begin().await?;

    let created_user = insert_user_if_not_exists(&mut tx, &discord_id.to_string(), name).await?;

    tx.commit().await?;

    let response = match created_user {
        Some(_) => {
            let msg = CreateInteractionResponseMessage::new()
                .content(format!("Registered {}.", name))
                .ephemeral(true);
            CreateInteractionResponse::Message(msg)
        }
        None => {
            let msg = CreateInteractionResponseMessage::new()
                .content("You are already registered.")
                .ephemeral(true);
            CreateInteractionResponse::Message(msg)
        }
    };

    interaction.create_response(ctx, response).await?;

    Ok(())
}
