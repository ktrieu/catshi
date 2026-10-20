use serenity::all::{
    CommandInteraction, Context, CreateInteractionResponse, CreateInteractionResponseFollowup,
    CreateInteractionResponseMessage,
};
use serenity::constants::MESSAGE_CODE_LIMIT;

pub fn text_interaction_response(text: &'_ str, ephemeral: bool) -> CreateInteractionResponse<'_> {
    CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .content(text)
            .ephemeral(ephemeral),
    )
}

/// Responds to `command` with `text`, splitting it on newlines across multiple
/// messages so each one fits under Discord's message limit. The first chunk is
/// sent as the interaction response and the rest as followups.
pub async fn send_long_message_safe(
    ctx: &Context,
    command: &CommandInteraction,
    text: &str,
    ephemeral: bool,
) -> anyhow::Result<()> {
    let chunks = split_message(text, MESSAGE_CODE_LIMIT);
    let mut chunks = chunks.iter();

    let first = chunks.next().map(String::as_str).unwrap_or_default();
    command
        .create_response(&ctx.http, text_interaction_response(first, ephemeral))
        .await?;

    for chunk in chunks {
        command
            .create_followup(
                &ctx.http,
                CreateInteractionResponseFollowup::new()
                    .content(chunk.as_str())
                    .ephemeral(ephemeral),
            )
            .await?;
    }

    Ok(())
}

/// Splits `text` into chunks of at most `limit` chars, breaking on newlines.
/// A single line longer than `limit` is hard-split.
fn split_message(text: &str, limit: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut current_len = 0;

    // Hard-split lines that are too long so every piece fits in a chunk.
    let pieces = text.split('\n').flat_map(|line| {
        let chars: Vec<char> = line.chars().collect();
        if chars.is_empty() {
            vec![String::new()]
        } else {
            chars.chunks(limit).map(|c| c.iter().collect()).collect()
        }
    });

    for piece in pieces {
        let piece_len = piece.chars().count();
        let sep_len = if current.is_empty() { 0 } else { 1 };

        // Start a new chunk if this piece would overflow the current one.
        if current_len + sep_len + piece_len > limit {
            chunks.push(std::mem::take(&mut current));
            current_len = 0;
        } else if sep_len == 1 {
            current.push('\n');
            current_len += 1;
        }

        current.push_str(&piece);
        current_len += piece_len;
    }

    if !current.is_empty() || chunks.is_empty() {
        chunks.push(current);
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_message_is_one_chunk() {
        assert_eq!(split_message("a\nb", 10), vec!["a\nb"]);
    }

    #[test]
    fn splits_on_newlines() {
        assert_eq!(split_message("aaaa\nbbbb\ncc", 9), vec!["aaaa\nbbbb", "cc"]);
    }

    #[test]
    fn hard_splits_long_lines() {
        assert_eq!(
            split_message("ab\ncdefg\nh", 3),
            vec!["ab", "cde", "fg", "h"]
        );
    }

    #[test]
    fn counts_chars_not_bytes() {
        assert_eq!(split_message("猫猫\n猫", 4), vec!["猫猫\n猫"]);
    }

    #[test]
    fn chunks_respect_limit() {
        let text = (0..500)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let chunks = split_message(&text, 2000);
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|c| c.chars().count() <= 2000));
        assert_eq!(chunks.join("\n"), text);
    }
}
