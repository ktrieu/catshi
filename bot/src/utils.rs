use serenity::all::{CreateInteractionResponse, CreateInteractionResponseMessage};

pub fn text_interaction_response(text: &'_ str, ephemeral: bool) -> CreateInteractionResponse<'_> {
    CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .content(text)
            .ephemeral(ephemeral),
    )
}
