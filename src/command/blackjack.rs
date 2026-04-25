use anyhow::anyhow;
use serenity::all::{
    CommandInteraction, CommandOptionType, ComponentInteraction, Context, CreateCommand,
    CreateCommandOption, CreateInteractionResponse, CreateInteractionResponseMessage, EditMessage,
    MessageFlags,
};

use crate::{
    Handler,
    blackjack::{Blackjack, BlackjackAction, Card, RngDeck, tests::RiggedDeck},
    currency::Currency,
    store::{
        self,
        transfer::{CreateTransfer, TransferSource},
        user::DbUser,
    },
    ui, utils,
};

pub const NAME: &'static str = "blackjack";

const OPTION_BET: &'static str = "bet";

pub fn create() -> CreateCommand<'static> {
    let amount_opt = CreateCommandOption::new(
        CommandOptionType::Number,
        OPTION_BET,
        "how much you want to bet",
    )
    // very clever, but no negative transfers.
    .required(true)
    .min_number_value(0.0f64);

    CreateCommand::new(NAME)
        .description("play some blackjack")
        .set_options(vec![amount_opt])
}

pub async fn run(
    ctx: &Context,
    handler: &Handler,
    user: &DbUser,
    command: &CommandInteraction,
) -> anyhow::Result<()> {
    let bet_value = ui::get_command_option_number(command, OPTION_BET)
        .ok_or(anyhow!("amount option is invalid"))?;
    let bet = Currency::new_yp_fractional(bet_value);

    // Double check that you can't bet a negative amount.
    if bet < Currency::from(0) {
        command
            .create_response(
                &ctx.http,
                utils::text_interaction_response("Bet must be greater than 0", true),
            )
            .await?;
        return Ok(());
    }

    // Check if we can actually transfer this much.
    if user.cash_balance < bet {
        let msg = format!(
            "You do not have enough yp ({}) to cover your bet ({}). Please kindly leave the casino.",
            user.cash_balance, bet
        );
        command
            .create_response(&ctx.http, utils::text_interaction_response(&msg, true))
            .await?;
        return Ok(());
    }

    let rigged = RiggedDeck::new(vec![
        Card::Numeric(2),
        Card::Numeric(2),
        Card::Ace,
        Card::Numeric(10),
    ]);

    let (game, payout) = Blackjack::new(bet, rigged);

    let resp_components = ui::blackjack::render_blackjack_message(&game, &user);

    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .components(&resp_components)
                    .flags(MessageFlags::IS_COMPONENTS_V2),
            ),
        )
        .await?;

    let response = command.get_response(&ctx.http).await?;

    let mut tx = handler.pool.begin().await?;

    let system_user = store::user::get_system_user(&mut *tx).await?;

    // Transfer the initial bet now. Otherwise if a game wasn't going well you could just never complete the game.
    let initial_bet = CreateTransfer {
        amount: bet,
        sender: user.id,
        receiver: system_user.id,
        memo: "Blackjack: initial bet".to_string(),
        source: TransferSource::Gambling,
    };
    store::transfer::persist_transfer(&mut tx, &initial_bet).await?;

    // If you won on a natural, transfer that as well.
    if let Some(payout) = payout {
        let natural_payout = CreateTransfer {
            amount: payout,
            sender: system_user.id,
            receiver: user.id,
            memo: "Blackjack: winnings".to_string(),
            source: TransferSource::Gambling,
        };

        store::transfer::persist_transfer(&mut tx, &natural_payout).await?;
    }

    let create = game.to_db_create(&user, response.channel_id.expect_channel(), response.id);
    store::blackjack::create_blackjack(&mut *tx, &create).await?;

    tx.commit().await?;

    Ok(())
}

pub async fn interact(
    ctx: &Context,
    handler: &Handler,
    user: &DbUser,
    component: &ComponentInteraction,
    action: BlackjackAction,
) -> anyhow::Result<()> {
    let channel_id = component.channel_id;
    let message_id = component.message.id;

    let mut tx = handler.pool.begin_with("BEGIN IMMEDIATE").await?;

    let system_user = store::user::get_system_user(&mut *tx).await?;

    let db_blackjack = store::blackjack::get_blackjack_from_message(
        &mut *tx,
        channel_id.expect_channel(),
        message_id,
    )
    .await?;

    if db_blackjack.owner_id != user.id {
        component
            .create_response(
                &ctx.http,
                utils::text_interaction_response(
                    "This is not your table. Create your own with /blackjack.",
                    true,
                ),
            )
            .await?;
        return Ok(());
    }

    let mut game = Blackjack::from_db(&db_blackjack, RngDeck::new())?;

    let result = game.act(action)?;
    if let Some(amount) = result.bet_increase
        && user.cash_balance < amount
    {
        // Oops, we can't afford to bet more. Let them know and return.
        component
            .create_response(
                &ctx.http,
                utils::text_interaction_response(
                    &format!(
                        "You have {} and cannot afford to bet an additional {}",
                        user.cash_balance, amount
                    ),
                    true,
                ),
            )
            .await?;
        return Ok(());
    }

    if let Some(t) = result.transfer(&system_user, &user) {
        store::transfer::persist_transfer(&mut tx, &t).await?;
    }

    let update = game.to_db_update();
    store::blackjack::update_blackjack(&mut *tx, db_blackjack.id, &update).await?;

    let new_msg_content = ui::blackjack::render_blackjack_message(&game, &user);

    let mut msg = ui::get_blackjack_message(&db_blackjack, &ctx).await?;

    msg.edit(
        &ctx.http,
        EditMessage::new()
            .components(new_msg_content)
            .flags(MessageFlags::IS_COMPONENTS_V2),
    )
    .await?;

    tx.commit().await?;

    component.defer(&ctx.http).await?;

    Ok(())
}
