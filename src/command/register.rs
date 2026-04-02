use serenity::all::CreateCommand;

pub const NAME: &'static str = "register";

pub fn create() -> CreateCommand {
    CreateCommand::new(NAME).description("register your Discord user")
}
