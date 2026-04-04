use serenity::all::{CreateInteractionResponse, CreateInteractionResponseMessage};

pub fn text_interaction_response(text: &str, ephemeral: bool) -> CreateInteractionResponse {
    CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .content(text)
            .ephemeral(ephemeral),
    )
}
