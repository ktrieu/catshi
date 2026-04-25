use std::collections::HashMap;
use std::fmt::Display;
use std::str::FromStr;

use anyhow::anyhow;
use rand::rngs::StdRng;
use rand::seq::IndexedRandom;
use serenity::all::{ChannelId, MessageId};

use crate::currency::Currency;
use crate::store::blackjack::{BlackjackState, CreateBlackjack, DbBlackjack, UpdateBlackjack};
use crate::store::transfer::{CreateTransfer, TransferSource};
use crate::store::user::DbUser;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Card {
    Ace,
    King,
    Queen,
    Jack,
    Numeric(u8),
}

impl Card {
    fn value(&self) -> u8 {
        match self {
            // This can also be 1 if it stops you from going over, but we have to handle that when
            // valuing an entire hand.
            Card::Ace => 11,
            Card::King => 10,
            Card::Queen => 10,
            Card::Jack => 10,
            Card::Numeric(n) => *n,
        }
    }
}

impl Display for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let c = match self {
            Card::Ace => "A".to_string(),
            Card::King => "K".to_string(),
            Card::Queen => "Q".to_string(),
            Card::Jack => "J".to_string(),
            Card::Numeric(n) => n.to_string(),
        };

        write!(f, "{}", c)
    }
}

impl FromStr for Card {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "A" => Ok(Card::Ace),
            "K" => Ok(Card::King),
            "Q" => Ok(Card::Queen),
            "J" => Ok(Card::Jack),
            _ => {
                let n = s
                    .parse::<u8>()
                    .map_err(|_| "card was not a face or number")?;

                if n < 2 || n > 10 {
                    return Err("card was not in 2-10");
                }

                Ok(Card::Numeric(n))
            }
        }
    }
}

fn serialize_cards(cards: &Vec<Card>) -> String {
    let strings: Vec<String> = cards.iter().map(|c| c.to_string()).collect();

    strings.join(" ")
}

fn deserialize_cards(cards: &str) -> anyhow::Result<Vec<Card>> {
    let components = cards.split(" ");

    let mut cards: Vec<Card> = Vec::new();

    for comp in components {
        cards.push(Card::from_str(comp).map_err(|e| anyhow!(e))?);
    }

    Ok(cards)
}

fn value_cards(cards: &Vec<Card>) -> u8 {
    // Almost a simple sum but aces can count for 1 if they stop you from busting.
    // Take the simple sum first but subtract 10 for each ace until you're under the limit.

    let mut sum = cards.iter().map(|c| c.value()).sum();

    if sum > 21 {
        let mut num_aces = cards.iter().filter(|c| **c == Card::Ace).count();

        while sum > 21 && num_aces != 0 {
            sum -= 10;
            num_aces -= 1;
        }
    }

    sum
}

const ALL_CARDS: [Card; 13] = [
    Card::Ace,
    Card::King,
    Card::Queen,
    Card::Jack,
    Card::Numeric(10),
    Card::Numeric(9),
    Card::Numeric(8),
    Card::Numeric(7),
    Card::Numeric(6),
    Card::Numeric(5),
    Card::Numeric(4),
    Card::Numeric(3),
    Card::Numeric(2),
];

pub trait Deck {
    fn draw(&mut self) -> Card;
}

#[derive(Debug)]
pub struct RngDeck {
    rng: StdRng,
}

impl Deck for RngDeck {
    fn draw(&mut self) -> Card {
        *ALL_CARDS
            .choose(&mut self.rng)
            .expect("ALL_CARDS must be defined")
    }
}

impl RngDeck {
    pub fn new() -> Self {
        Self {
            rng: rand::make_rng(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlackjackAction {
    Hit,
    Stand,
    DoubleDown,
    Reveal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameWinner {
    Dealer,
    DealerNatural,
    Player,
    PlayerNatural,
    Push,
    PushNatural,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActResult {
    pub next_state: BlackjackState,
    pub bet_increase: Option<Currency>,
    pub payout: Option<Currency>,
}

impl ActResult {
    pub fn transfer(&self, system_user: &DbUser, player: &DbUser) -> Option<CreateTransfer> {
        let increase_transfer = self.bet_increase.map(|increase| CreateTransfer {
            amount: increase,
            sender: player.id,
            receiver: system_user.id,
            memo: "Blackjack: Double down".to_string(),
            source: TransferSource::Gambling,
        });

        let payout_transfer = self.payout.map(|payout| CreateTransfer {
            amount: payout,
            sender: system_user.id,
            receiver: player.id,
            memo: "Blackjack: winnings".to_string(),
            source: TransferSource::Gambling,
        });

        // We should never get two of these at the same time.
        assert!(increase_transfer.is_none() || payout_transfer.is_none());

        increase_transfer.or(payout_transfer)
    }
}

#[derive(Debug, Clone)]
pub struct Blackjack<D: Deck> {
    pub dealer: Vec<Card>,
    pub player: Vec<Card>,
    pub staked: Currency,
    pub state: BlackjackState,
    pub deck: D,
}

impl<D: Deck> Blackjack<D> {
    fn draw_card(&mut self) -> Card {
        // We don't want to draw more than four of each card, that would look dumb.
        let mut counts: HashMap<&Card, u8> = HashMap::new();

        for c in &self.dealer {
            *counts.entry(c).or_insert(0) += 1;
        }

        for c in &self.player {
            *counts.entry(c).or_insert(0) += 1;
        }

        let mut drawn = self.deck.draw();

        while *counts.get(&drawn).unwrap_or(&0) > 4 {
            drawn = self.deck.draw();
        }

        drawn
    }

    fn draw_dealer(&mut self) {
        let drawn = self.draw_card();

        self.dealer.push(drawn);
    }

    fn draw_player(&mut self) {
        let drawn = self.draw_card();

        self.player.push(drawn);
    }

    pub fn new(staked: Currency, deck: D) -> (Self, Option<Currency>) {
        let mut game = Self {
            dealer: Vec::new(),
            player: Vec::new(),
            staked,
            deck,
            state: BlackjackState::Betting,
        };

        game.draw_dealer();
        game.draw_dealer();

        game.draw_player();
        game.draw_player();

        let winner = game.winner();
        if matches!(
            winner,
            GameWinner::DealerNatural | GameWinner::PlayerNatural | GameWinner::PushNatural
        ) {
            game.state = BlackjackState::Closed;
        }

        match game.winner() {
            GameWinner::DealerNatural => {
                // You lose.
                (game, None)
            }
            GameWinner::PlayerNatural => {
                // Player gets a natural! we pay 3:2 for this.
                (game, Some(staked + staked * 1.5))
            }
            GameWinner::PushNatural => (game, Some(staked)),
            _ => (game, None),
        }
    }

    pub fn is_action_valid(&self, action: BlackjackAction) -> bool {
        match self.state {
            BlackjackState::Betting => {
                // Only double down if you have only two cards, i.e., you haven't hit yet.
                if action == BlackjackAction::DoubleDown && self.player.len() == 2 {
                    true
                } else {
                    action == BlackjackAction::Stand || action == BlackjackAction::Hit
                }
            }
            // If we stood or busted, you can only reveal the dealers.
            BlackjackState::Stand | BlackjackState::PlayerBust => action == BlackjackAction::Reveal,
            // And if we're closed you can do nothing.
            BlackjackState::Closed => false,
        }
    }

    pub fn act(&mut self, action: BlackjackAction) -> anyhow::Result<ActResult> {
        if !self.is_action_valid(action) {
            // This should be caught by the command but just double-check here.
            return Err(anyhow!("invalid action {action:?}"));
        }

        let next_state = match action {
            BlackjackAction::Hit => {
                self.draw_player();
                if value_cards(&self.player) > 21 {
                    BlackjackState::PlayerBust
                } else {
                    BlackjackState::Betting
                }
            }
            BlackjackAction::Stand => BlackjackState::Stand,
            BlackjackAction::DoubleDown => {
                self.draw_player();

                if value_cards(&self.player) > 21 {
                    BlackjackState::PlayerBust
                } else {
                    BlackjackState::Stand
                }
            }
            BlackjackAction::Reveal => {
                // If the player has busted, no need to draw any more cards.
                // The UI code will reveal the face down card.
                if self.state != BlackjackState::PlayerBust {
                    while value_cards(&self.dealer) < 17 {
                        self.draw_dealer()
                    }
                }
                BlackjackState::Closed
            }
        };

        self.state = next_state;

        let bet_increase = if action == BlackjackAction::DoubleDown {
            Some(self.staked)
        } else {
            None
        };

        let payout = if next_state == BlackjackState::Closed {
            match self.winner() {
                GameWinner::Dealer => None,
                GameWinner::Player => Some(self.staked * 2),
                GameWinner::Push => Some(self.staked),
                // Technically, these get immediately resolved on create but whatever.
                GameWinner::DealerNatural => None,
                GameWinner::PlayerNatural => Some(self.staked * 2.5),
                GameWinner::PushNatural => Some(self.staked),
            }
        } else {
            None
        };

        if let Some(increase) = bet_increase {
            self.staked = self.staked + increase;
        }

        let result = ActResult {
            next_state,
            bet_increase,
            payout,
        };

        Ok(result)
    }

    pub fn winner(&self) -> GameWinner {
        let player_value = value_cards(&self.player);
        let dealer_value = value_cards(&self.dealer);

        let dealer_natural = dealer_value == 21 && self.dealer.len() == 2;
        let player_natural = player_value == 21 && self.player.len() == 2;

        if dealer_natural && player_natural {
            GameWinner::PushNatural
        } else if dealer_natural {
            GameWinner::DealerNatural
        } else if player_natural {
            GameWinner::PlayerNatural
        } else if player_value > 21 {
            GameWinner::Dealer
        } else if dealer_value > 21 {
            GameWinner::Player
        } else {
            let dealer_diff = 21 - dealer_value;
            let player_diff = 21 - player_value;

            if dealer_diff < player_diff {
                GameWinner::Dealer
            } else if player_diff < dealer_diff {
                GameWinner::Player
            } else {
                GameWinner::Push
            }
        }
    }

    pub fn from_db(db: &DbBlackjack, deck: D) -> anyhow::Result<Self> {
        Ok(Self {
            dealer: deserialize_cards(&db.dealer)?,
            player: deserialize_cards(&db.player)?,
            staked: db.staked,
            state: db.state,
            deck,
        })
    }

    pub fn to_db_create(
        &self,
        owner: &DbUser,
        channel_id: ChannelId,
        message_id: MessageId,
    ) -> CreateBlackjack {
        CreateBlackjack {
            dealer: self.dealer_serialized(),
            player: self.player_serialized(),
            owner_id: owner.id,
            state: self.state,
            staked: self.staked,
            channel_id: channel_id.to_string(),
            message_id: message_id.to_string(),
        }
    }

    pub fn to_db_update(&self) -> UpdateBlackjack {
        UpdateBlackjack {
            dealer: self.dealer_serialized(),
            player: self.player_serialized(),
            staked: self.staked,
            state: self.state,
        }
    }

    pub fn dealer_serialized(&self) -> String {
        serialize_cards(&self.dealer)
    }

    pub fn player_serialized(&self) -> String {
        serialize_cards(&self.player)
    }

    pub fn dealer_value(&self) -> u8 {
        // Don't show the value of the face down card until the game is over.
        if self.state == BlackjackState::Closed {
            value_cards(&self.dealer)
        } else {
            self.dealer[0].value()
        }
    }

    pub fn player_value(&self) -> u8 {
        value_cards(&self.player)
    }

    pub fn dealer_display(&self) -> String {
        // If the game is closed, show all the cards. But if we haven't finished only show the first one.
        if self.state == BlackjackState::Closed {
            serialize_cards(&self.dealer)
        } else {
            format!("{} ?", self.dealer[0].to_string())
        }
    }

    pub fn player_display(&self) -> String {
        // Always display all the players cards.
        serialize_cards(&self.player)
    }
}

#[cfg(test)]
mod tests {
    struct RiggedDeck {
        seq: Vec<Card>,
        idx: usize,
    }

    impl RiggedDeck {
        pub fn new(seq: Vec<Card>) -> Self {
            RiggedDeck { seq, idx: 0 }
        }
    }

    impl Deck for RiggedDeck {
        fn draw(&mut self) -> Card {
            let card = self.seq[self.idx];
            let next_idx = (self.idx + 1) % self.seq.len();

            self.idx = next_idx;

            card
        }
    }

    use super::*;

    #[test]
    fn test_new_game_normal() {
        let rigged = RiggedDeck::new(vec![Card::Numeric(2)]);
        let (game, payout) = Blackjack::new(Currency::from(2), rigged);

        assert_eq!(game.dealer, vec![Card::Numeric(2), Card::Numeric(2)]);
        assert_eq!(game.player, vec![Card::Numeric(2), Card::Numeric(2)]);

        assert_eq!(game.state, BlackjackState::Betting);

        assert_eq!(game.staked, Currency::from(2));
        assert_eq!(payout, None)
    }

    #[test]
    fn test_new_game_natural_push() {
        let rigged = RiggedDeck::new(vec![Card::Numeric(10), Card::Ace]);
        let (game, payout) = Blackjack::new(Currency::from(100), rigged);

        assert_eq!(game.dealer, vec![Card::Numeric(10), Card::Ace]);
        assert_eq!(game.player, vec![Card::Numeric(10), Card::Ace]);

        assert_eq!(game.state, BlackjackState::Closed);

        // Dealer and player get a natural. That's a push.
        assert_eq!(game.staked, Currency::from(100));
        assert_eq!(payout, Some(Currency::from(100)))
    }

    #[test]
    fn test_new_game_natural_dealer() {
        let rigged = RiggedDeck::new(vec![
            Card::Numeric(10),
            Card::Ace,
            Card::Numeric(2),
            Card::Numeric(2),
        ]);
        let (game, payout) = Blackjack::new(Currency::from(100), rigged);

        assert_eq!(game.dealer, vec![Card::Numeric(10), Card::Ace]);
        assert_eq!(game.player, vec![Card::Numeric(2), Card::Numeric(2)]);

        assert_eq!(game.state, BlackjackState::Closed);

        // Dealer gets a natural. We lose immediately.
        assert_eq!(game.staked, Currency::from(100));
        assert_eq!(payout, None)
    }

    #[test]
    fn test_new_game_natural_player() {
        let rigged = RiggedDeck::new(vec![
            Card::Numeric(2),
            Card::Numeric(2),
            Card::Numeric(10),
            Card::Ace,
        ]);
        let (game, payout) = Blackjack::new(Currency::from(100), rigged);

        assert_eq!(game.dealer, vec![Card::Numeric(2), Card::Numeric(2)]);
        assert_eq!(game.player, vec![Card::Numeric(10), Card::Ace]);

        assert_eq!(game.state, BlackjackState::Closed);

        // Player gets a natural. This pays 3:2.
        assert_eq!(game.staked, Currency::from(100));
        assert_eq!(payout, Some(Currency::from(250)))
    }

    #[test]
    fn test_hit() {
        let rigged = RiggedDeck::new(vec![Card::King]);
        let mut game = Blackjack {
            dealer: vec![Card::King, Card::Queen],
            player: vec![Card::Numeric(2), Card::Numeric(2)],
            staked: Currency::from(2),
            state: BlackjackState::Betting,
            deck: rigged,
        };

        let result = game.act(BlackjackAction::Hit).expect("should succeed");
        assert_eq!(result.bet_increase, None);
        assert_eq!(result.next_state, BlackjackState::Betting);
        assert_eq!(result.payout, None);

        assert_eq!(
            game.player,
            vec![Card::Numeric(2), Card::Numeric(2), Card::King]
        );
        assert_eq!(game.state, BlackjackState::Betting);
    }

    #[test]
    fn test_hit_bust() {
        let rigged = RiggedDeck::new(vec![Card::King]);
        let mut game = Blackjack {
            dealer: vec![Card::King, Card::Queen],
            player: vec![Card::Numeric(10), Card::Numeric(10)],
            staked: Currency::from(2),
            state: BlackjackState::Betting,
            deck: rigged,
        };

        let result = game.act(BlackjackAction::Hit).expect("should succeed");
        assert_eq!(result.bet_increase, None);
        assert_eq!(result.next_state, BlackjackState::PlayerBust);
        assert_eq!(result.payout, None);

        assert_eq!(
            game.player,
            vec![Card::Numeric(10), Card::Numeric(10), Card::King]
        );
        assert_eq!(game.state, BlackjackState::PlayerBust);
    }

    #[test]
    fn test_stand() {
        let rigged = RiggedDeck::new(vec![Card::King]);
        let mut game = Blackjack {
            dealer: vec![Card::King, Card::Queen],
            player: vec![Card::Numeric(10), Card::Numeric(10)],
            staked: Currency::from(2),
            state: BlackjackState::Betting,
            deck: rigged,
        };

        let result = game.act(BlackjackAction::Stand).expect("should succeed");
        assert_eq!(result.bet_increase, None);
        assert_eq!(result.next_state, BlackjackState::Stand);
        assert_eq!(result.payout, None);

        assert_eq!(game.player, vec![Card::Numeric(10), Card::Numeric(10)]);
        assert_eq!(game.state, BlackjackState::Stand);
    }

    #[test]
    fn test_double_down() {
        let rigged = RiggedDeck::new(vec![Card::King]);
        let mut game = Blackjack {
            dealer: vec![Card::King, Card::Queen],
            player: vec![Card::Numeric(2), Card::Numeric(2)],
            staked: Currency::from(2),
            state: BlackjackState::Betting,
            deck: rigged,
        };

        let result = game
            .act(BlackjackAction::DoubleDown)
            .expect("should succeed");
        assert_eq!(result.bet_increase, Some(Currency::from(2)));
        assert_eq!(result.next_state, BlackjackState::Stand);
        assert_eq!(result.payout, None);

        assert_eq!(
            game.player,
            vec![Card::Numeric(2), Card::Numeric(2), Card::King]
        );
        assert_eq!(game.state, BlackjackState::Stand);
    }

    #[test]
    fn test_double_down_bust() {
        let rigged = RiggedDeck::new(vec![Card::King]);
        let mut game = Blackjack {
            dealer: vec![Card::King, Card::Queen],
            player: vec![Card::Numeric(10), Card::Numeric(10)],
            staked: Currency::from(2),
            state: BlackjackState::Betting,
            deck: rigged,
        };

        let result = game
            .act(BlackjackAction::DoubleDown)
            .expect("should succeed");
        assert_eq!(result.bet_increase, Some(Currency::from(2)));
        assert_eq!(result.next_state, BlackjackState::PlayerBust);
        assert_eq!(result.payout, None);

        assert_eq!(
            game.player,
            vec![Card::Numeric(10), Card::Numeric(10), Card::King]
        );
        assert_eq!(game.state, BlackjackState::PlayerBust);
    }

    #[test]
    fn test_resolve_dealer_bust() {
        let rigged = RiggedDeck::new(vec![Card::King]);
        let mut game = Blackjack {
            dealer: vec![Card::Numeric(10), Card::Numeric(6)],
            player: vec![Card::Numeric(10), Card::Numeric(10)],
            staked: Currency::from(2),
            state: BlackjackState::Stand,
            deck: rigged,
        };

        let result = game.act(BlackjackAction::Reveal).expect("should succeed");

        // Dealer draws a King next and loses. We get the payout.
        assert!(game.winner() == GameWinner::Player);
        assert_eq!(result.bet_increase, None);
        assert_eq!(result.next_state, BlackjackState::Closed);
        assert_eq!(result.payout, Some(game.staked * 2));

        assert_eq!(
            game.dealer,
            vec![Card::Numeric(10), Card::Numeric(6), Card::King]
        );
        assert_eq!(game.state, BlackjackState::Closed);
    }

    #[test]
    fn test_resolve_player_bust() {
        let rigged = RiggedDeck::new(vec![Card::King]);
        let mut game = Blackjack {
            dealer: vec![Card::Numeric(10), Card::Numeric(6)],
            player: vec![Card::Numeric(10), Card::Numeric(10), Card::Numeric(10)],
            staked: Currency::from(2),
            state: BlackjackState::PlayerBust,
            deck: rigged,
        };

        let result = game.act(BlackjackAction::Reveal).expect("should succeed");

        // Player busts. No payouts. Dealer doesn't draw.
        assert!(game.winner() == GameWinner::Dealer);
        assert_eq!(result.bet_increase, None);
        assert_eq!(result.next_state, BlackjackState::Closed);
        assert_eq!(result.payout, None);

        assert_eq!(game.dealer, vec![Card::Numeric(10), Card::Numeric(6)]);
        assert_eq!(game.state, BlackjackState::Closed);
    }

    #[test]
    fn test_resolve_neither_bust_player_wins() {
        let rigged = RiggedDeck::new(vec![Card::Numeric(5)]);
        let mut game = Blackjack {
            dealer: vec![Card::Numeric(10), Card::Numeric(2)],
            player: vec![Card::Numeric(10), Card::Numeric(10), Card::Ace],
            staked: Currency::from(2),
            state: BlackjackState::Stand,
            deck: rigged,
        };

        let result = game.act(BlackjackAction::Reveal).expect("should succeed");

        // Dealer draws a 5 next for a total of 17. Dealer stands while we have 21.
        assert!(game.winner() == GameWinner::Player);
        assert_eq!(result.bet_increase, None);
        assert_eq!(result.next_state, BlackjackState::Closed);
        assert_eq!(result.payout, Some(game.staked * 2));

        assert_eq!(
            game.dealer,
            vec![Card::Numeric(10), Card::Numeric(2), Card::Numeric(5)],
        );
        assert_eq!(game.state, BlackjackState::Closed);
    }

    #[test]
    fn test_resolve_neither_bust_dealer_wins() {
        let rigged = RiggedDeck::new(vec![Card::Numeric(5)]);
        let mut game = Blackjack {
            dealer: vec![Card::Numeric(10), Card::Numeric(2)],
            player: vec![Card::Numeric(2), Card::Numeric(2)],
            staked: Currency::from(2),
            state: BlackjackState::Stand,
            deck: rigged,
        };

        let result = game.act(BlackjackAction::Reveal).expect("should succeed");

        // We were dumb and stood on only 2. Dealer draws a 5 for 17 and wins.
        assert!(game.winner() == GameWinner::Dealer);
        assert_eq!(result.bet_increase, None);
        assert_eq!(result.next_state, BlackjackState::Closed);
        assert_eq!(result.payout, None);

        assert_eq!(
            game.dealer,
            vec![Card::Numeric(10), Card::Numeric(2), Card::Numeric(5)],
        );
        assert_eq!(game.state, BlackjackState::Closed);
    }

    #[test]
    fn test_resolve_push() {
        let rigged = RiggedDeck::new(vec![Card::Numeric(5)]);
        let mut game = Blackjack {
            dealer: vec![Card::Numeric(10), Card::Numeric(6)],
            player: vec![Card::Numeric(10), Card::Numeric(10), Card::Ace],
            staked: Currency::from(2),
            state: BlackjackState::Stand,
            deck: rigged,
        };

        let result = game.act(BlackjackAction::Reveal).expect("should succeed");

        // We both get 21. This is a push and the player gets their money back.
        assert!(game.winner() == GameWinner::Push);
        assert_eq!(result.bet_increase, None);
        assert_eq!(result.next_state, BlackjackState::Closed);
        assert_eq!(result.payout, Some(game.staked));

        assert_eq!(
            game.dealer,
            vec![Card::Numeric(10), Card::Numeric(6), Card::Numeric(5)],
        );
        assert_eq!(game.state, BlackjackState::Closed);
    }

    #[test]
    fn test_resolve_push_less_than_21() {
        let rigged = RiggedDeck::new(vec![Card::Numeric(5)]);
        let mut game = Blackjack {
            dealer: vec![Card::Numeric(10), Card::Numeric(8)],
            player: vec![Card::Numeric(10), Card::Numeric(8)],
            staked: Currency::from(2),
            state: BlackjackState::Stand,
            deck: rigged,
        };

        let result = game.act(BlackjackAction::Reveal).expect("should succeed");

        // We both stood at 18. This is a push as well.
        assert!(game.winner() == GameWinner::Push);
        assert_eq!(result.bet_increase, None);
        assert_eq!(result.next_state, BlackjackState::Closed);
        assert_eq!(result.payout, Some(game.staked));

        assert_eq!(game.dealer, vec![Card::Numeric(10), Card::Numeric(8)],);
        assert_eq!(game.state, BlackjackState::Closed);
    }

    #[test]
    fn test_no_double_down_after_draw() {
        let rigged = RiggedDeck::new(vec![Card::King]);
        let mut game = Blackjack {
            dealer: vec![Card::King, Card::Queen],
            player: vec![Card::Numeric(2), Card::Numeric(2), Card::Numeric(2)],
            staked: Currency::from(2),
            state: BlackjackState::Betting,
            deck: rigged,
        };

        let _ = game
            .act(BlackjackAction::DoubleDown)
            .expect_err("cannot double down after hitting");
        assert!(!game.is_action_valid(BlackjackAction::DoubleDown))
    }

    #[test]
    fn test_no_act_on_closed() {
        let rigged = RiggedDeck::new(vec![Card::King]);
        let mut game = Blackjack {
            dealer: vec![Card::King, Card::Queen],
            player: vec![Card::Numeric(2), Card::Numeric(2), Card::Numeric(2)],
            staked: Currency::from(2),
            state: BlackjackState::Closed,
            deck: rigged,
        };

        assert!(!game.is_action_valid(BlackjackAction::DoubleDown));
        let _ = game
            .act(BlackjackAction::DoubleDown)
            .expect_err("cannot double down on closed game");

        assert!(!game.is_action_valid(BlackjackAction::Hit));
        let _ = game
            .act(BlackjackAction::Hit)
            .expect_err("cannot double down on closed game");

        assert!(!game.is_action_valid(BlackjackAction::Stand));
        let _ = game
            .act(BlackjackAction::Stand)
            .expect_err("cannot double down on closed game");

        assert!(!game.is_action_valid(BlackjackAction::Reveal));
        let _ = game
            .act(BlackjackAction::Reveal)
            .expect_err("cannot double down on closed game");
    }

    #[test]
    fn test_no_bet_on_stand() {
        let rigged = RiggedDeck::new(vec![Card::King]);
        let mut game = Blackjack {
            dealer: vec![Card::King, Card::Queen],
            player: vec![Card::Numeric(2), Card::Numeric(2), Card::Numeric(2)],
            staked: Currency::from(2),
            state: BlackjackState::Stand,
            deck: rigged,
        };

        assert!(!game.is_action_valid(BlackjackAction::DoubleDown));
        let _ = game
            .act(BlackjackAction::DoubleDown)
            .expect_err("cannot double down on stood game");
        assert!(!game.is_action_valid(BlackjackAction::Hit));
        let _ = game
            .act(BlackjackAction::Hit)
            .expect_err("cannot double down on stood game");
        assert!(!game.is_action_valid(BlackjackAction::Stand));

        let _ = game
            .act(BlackjackAction::Stand)
            .expect_err("cannot double down on stood game");

        assert!(game.is_action_valid(BlackjackAction::Reveal));
        let _ = game
            .act(BlackjackAction::Reveal)
            .expect("should reveal on stood game");
    }

    #[test]
    fn test_no_bet_on_bust() {
        let rigged = RiggedDeck::new(vec![Card::King]);
        let mut game = Blackjack {
            dealer: vec![Card::King, Card::Queen],
            player: vec![Card::Numeric(2), Card::Numeric(2), Card::Numeric(2)],
            staked: Currency::from(2),
            state: BlackjackState::PlayerBust,
            deck: rigged,
        };

        assert!(!game.is_action_valid(BlackjackAction::DoubleDown));
        let _ = game
            .act(BlackjackAction::DoubleDown)
            .expect_err("cannot double down on busted game");

        assert!(!game.is_action_valid(BlackjackAction::Hit));
        let _ = game
            .act(BlackjackAction::Hit)
            .expect_err("cannot double down on busted game");

        assert!(!game.is_action_valid(BlackjackAction::Stand));
        let _ = game
            .act(BlackjackAction::Stand)
            .expect_err("cannot double down on busted game");

        assert!(game.is_action_valid(BlackjackAction::Reveal));
        let _ = game
            .act(BlackjackAction::Reveal)
            .expect("should reveal on busted game");
    }

    #[test]
    fn test_value_simple() {
        assert_eq!(value_cards(&vec![Card::King, Card::Numeric(2)]), 12);
    }

    #[test]
    fn test_value_ace_high() {
        assert_eq!(value_cards(&vec![Card::King, Card::Ace]), 21);
    }

    #[test]
    fn test_value_ace_low() {
        assert_eq!(value_cards(&vec![Card::King, Card::King, Card::Ace]), 21);
    }

    #[test]
    fn test_value_partial_aces() {
        assert_eq!(
            value_cards(&vec![Card::Numeric(9), Card::Ace, Card::Ace]),
            21
        );
    }

    #[test]
    fn test_value_many_aces() {
        assert_eq!(
            value_cards(&vec![
                Card::Ace,
                Card::Ace,
                Card::Ace,
                Card::Ace,
                Card::Ace,
                Card::Numeric(3)
            ]),
            18,
        );
    }

    #[test]
    fn test_value_ace_bust() {
        assert_eq!(
            value_cards(&vec![
                Card::King,
                Card::King,
                Card::King,
                Card::Ace,
                Card::Ace
            ]),
            32
        );
    }

    #[test]
    fn test_transfer_payout() {
        let result = ActResult {
            next_state: BlackjackState::Closed,
            bet_increase: None,
            payout: Some(Currency::from(3)),
        };

        let system_user = DbUser {
            id: 1,
            discord_id: "0".to_string(),
            name: "sys".to_string(),
            cash_balance: Currency::from(0),
        };

        let player = DbUser {
            id: 2,
            discord_id: "111".to_string(),
            name: "user".to_string(),
            cash_balance: Currency::from(0),
        };

        assert_eq!(
            result.transfer(&system_user, &player),
            Some(CreateTransfer {
                amount: Currency::from(3),
                sender: 1,
                receiver: 2,
                memo: "Blackjack: winnings".to_string(),
                source: TransferSource::Gambling,
            })
        )
    }

    #[test]
    fn test_transfer_bet_increase() {
        let result = ActResult {
            next_state: BlackjackState::Closed,
            bet_increase: Some(Currency::from(3)),
            payout: None,
        };

        let system_user = DbUser {
            id: 1,
            discord_id: "0".to_string(),
            name: "sys".to_string(),
            cash_balance: Currency::from(0),
        };

        let player = DbUser {
            id: 2,
            discord_id: "111".to_string(),
            name: "user".to_string(),
            cash_balance: Currency::from(0),
        };

        assert_eq!(
            result.transfer(&system_user, &player),
            Some(CreateTransfer {
                amount: Currency::from(3),
                sender: 2,
                receiver: 1,
                memo: "Blackjack: Double down".to_string(),
                source: TransferSource::Gambling,
            })
        )
    }

    #[test]
    fn test_to_create() {
        let rigged = RiggedDeck::new(vec![Card::King]);
        let game = Blackjack {
            dealer: vec![Card::King, Card::Queen],
            player: vec![Card::Numeric(2), Card::Numeric(2), Card::Numeric(2)],
            staked: Currency::from(2),
            state: BlackjackState::PlayerBust,
            deck: rigged,
        };

        let player = DbUser {
            id: 2,
            discord_id: "111".to_string(),
            name: "user".to_string(),
            cash_balance: Currency::from(0),
        };

        let channel_id = ChannelId::new(7);
        let message_id = MessageId::new(7);

        assert_eq!(
            game.to_db_create(&player, channel_id, message_id),
            CreateBlackjack {
                dealer: "K Q".to_string(),
                player: "2 2 2".to_string(),
                owner_id: player.id,
                state: BlackjackState::PlayerBust,
                staked: Currency::from(2),
                channel_id: "7".to_string(),
                message_id: "7".to_string(),
            },
        )
    }

    #[test]
    fn test_to_update() {
        let rigged = RiggedDeck::new(vec![Card::King]);
        let game = Blackjack {
            dealer: vec![Card::King, Card::Queen],
            player: vec![Card::Numeric(2), Card::Numeric(2), Card::Numeric(2)],
            staked: Currency::from(2),
            state: BlackjackState::PlayerBust,
            deck: rigged,
        };

        assert_eq!(
            game.to_db_update(),
            UpdateBlackjack {
                dealer: "K Q".to_string(),
                player: "2 2 2".to_string(),
                staked: Currency::from(2),
                state: BlackjackState::PlayerBust,
            },
        )
    }
}
