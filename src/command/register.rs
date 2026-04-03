use serenity::all::{CommandInteraction, Context, CreateCommand};

use crate::bot::Bot;

pub const NAME: &'static str = "register";

pub fn create() -> CreateCommand {
    CreateCommand::new(NAME).description("register your Discord user")
}

pub fn run(bot: &mut Bot, interaction: CommandInteraction) -> anyhow::Result<()> {
    Ok(())
}
