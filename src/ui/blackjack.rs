use serenity::all::{
    ButtonStyle, CreateActionRow, CreateButton, CreateComponent, CreateSeparator, CreateTextDisplay,
};

use crate::{
    blackjack::{Blackjack, BlackjackAction, Deck},
    store::user::DbUser,
};

pub const ACTION_HIT: &'static str = "blackjack|hit";
pub const ACTION_STAND: &'static str = "blackjack|stand";
pub const ACTION_DOUBLE: &'static str = "blackjack|double";
pub const ACTION_REVEAL: &'static str = "blackjack|reveal";

pub fn parse_blackjack_action(id: &str) -> Option<BlackjackAction> {
    match id {
        ACTION_HIT => Some(BlackjackAction::Hit),
        ACTION_STAND => Some(BlackjackAction::Stand),
        ACTION_DOUBLE => Some(BlackjackAction::DoubleDown),
        ACTION_REVEAL => Some(BlackjackAction::Reveal),
        _ => None,
    }
}

pub fn render_blackjack_message<'a, D: Deck>(
    blackjack: &'a Blackjack<D>,
    owner: &'a DbUser,
) -> Vec<CreateComponent<'a>> {
    let info = CreateTextDisplay::new(format!("{} ({} bet)", owner.name, blackjack.staked));
    let status_text = match blackjack.state {
        crate::store::blackjack::BlackjackState::Betting => "Betting in progress.".to_string(),
        crate::store::blackjack::BlackjackState::Stand => {
            "Player stood. Click to reveal dealer's cards.".to_string()
        }
        crate::store::blackjack::BlackjackState::PlayerBust => {
            "Player busted. Click to reveal dealer's cards.".to_string()
        }
        crate::store::blackjack::BlackjackState::Closed => match blackjack.winner() {
            crate::blackjack::GameWinner::Dealer => {
                format!("Dealer wins. You lost {}.", blackjack.staked)
            }
            crate::blackjack::GameWinner::Player => {
                format!("{} wins {}.", owner.name, blackjack.staked * 2)
            }
            crate::blackjack::GameWinner::Push | crate::blackjack::GameWinner::PushNatural => {
                format!("Push. {} receives {} back.", owner.name, blackjack.staked)
            }
            crate::blackjack::GameWinner::DealerNatural => {
                format!("Dealer gets natural 21. You lost {}", blackjack.staked)
            }
            crate::blackjack::GameWinner::PlayerNatural => {
                format!("You got a natural 21. You win {}", blackjack.staked * 2.5)
            }
        },
    };
    let status = CreateTextDisplay::new(status_text);

    let dealer = CreateTextDisplay::new(format!(
        "Dealer: {} ({})",
        blackjack.dealer_display(),
        blackjack.dealer_value()
    ));
    let player = CreateTextDisplay::new(format!(
        "Player: {} ({})",
        blackjack.player_display(),
        blackjack.player_value()
    ));

    let mut buttons = Vec::new();

    if blackjack.is_action_valid(BlackjackAction::Hit) {
        buttons.push(
            CreateButton::new(ACTION_HIT)
                .label("Hit")
                .style(ButtonStyle::Primary),
        );
    }

    if blackjack.is_action_valid(BlackjackAction::Stand) {
        buttons.push(
            CreateButton::new(ACTION_STAND)
                .label("Stand")
                .style(ButtonStyle::Secondary),
        );
    }

    if blackjack.is_action_valid(BlackjackAction::DoubleDown) {
        buttons.push(
            CreateButton::new(ACTION_DOUBLE)
                .label("Double down")
                .style(ButtonStyle::Success),
        );
    }

    if blackjack.is_action_valid(BlackjackAction::Reveal) {
        buttons.push(
            CreateButton::new(ACTION_REVEAL)
                .label("Reveal")
                .style(ButtonStyle::Success),
        );
    }

    let mut components = vec![
        CreateComponent::TextDisplay(info),
        CreateComponent::TextDisplay(status),
        CreateComponent::Separator(CreateSeparator::new()),
        CreateComponent::TextDisplay(dealer),
        CreateComponent::Separator(CreateSeparator::new()),
        CreateComponent::TextDisplay(player),
        CreateComponent::Separator(CreateSeparator::new()),
    ];

    if buttons.len() != 0 {
        let actions = CreateActionRow::buttons(buttons);
        components.push(CreateComponent::ActionRow(actions));
    }

    components
}
