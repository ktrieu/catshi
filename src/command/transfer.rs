use anyhow::anyhow;
use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption,
};

use crate::{
    Handler,
    currency::Currency,
    store::{
        self,
        transfer::{CreateTransfer, TransferSource},
        user::DbUser,
    },
    ui, utils,
};

pub const NAME: &'static str = "ctransfer";

const OPTION_RECIPIENT: &'static str = "recipient";
const OPTION_AMOUNT: &'static str = "amount";
const OPTION_MEMO: &'static str = "memo";

pub fn create() -> CreateCommand<'static> {
    let user_opt = CreateCommandOption::new(
        CommandOptionType::User,
        OPTION_RECIPIENT,
        "the user you want to transfer to",
    )
    .required(true);

    let amount_opt = CreateCommandOption::new(
        CommandOptionType::Number,
        OPTION_AMOUNT,
        "the amount of yp you want to transfer",
    )
    // very clever, but no negative transfers.
    .required(true)
    .min_number_value(0.0f64);

    let memo_opt = CreateCommandOption::new(
        CommandOptionType::String,
        OPTION_MEMO,
        "a description for the transfer",
    );

    CreateCommand::new(NAME)
        .description("transfer yp to another user")
        .set_options(vec![user_opt, amount_opt, memo_opt])
}

pub async fn run(
    ctx: &Context,
    handler: &Handler,
    user: &DbUser,
    command: &CommandInteraction,
) -> anyhow::Result<()> {
    let recipient_value = ui::get_command_option_user(command, OPTION_RECIPIENT)
        .ok_or(anyhow!("recipient option is invalid"))?;

    let amount_value = ui::get_command_option_number(command, OPTION_AMOUNT)
        .ok_or(anyhow!("amount option is invalid"))?;
    let amount = Currency::new_yp_fractional(amount_value);

    // Double check that you can't input a negative transfer.
    if amount < Currency::from(0) {
        command
            .create_response(
                &ctx.http,
                utils::text_interaction_response("Transfer amount must be greater than 0", true),
            )
            .await?;
        return Ok(());
    }

    let memo_value =
        ui::get_command_option_str(command, OPTION_MEMO).unwrap_or("User initiated transfer");

    let mut tx = handler.pool.begin().await?;

    let recipient = store::user::get_user_by_discord_id(&mut *tx, &recipient_value).await?;
    let recipient = match recipient {
        Some(recipient) => recipient,
        None => {
            command
                .create_response(
                    &ctx.http,
                    utils::text_interaction_response(
                        "Recipient was not registered. Make a trade to register.",
                        true,
                    ),
                )
                .await?;
            return Ok(());
        }
    };

    // Check if we can actually transfer this much.
    if user.cash_balance < amount {
        let msg = format!(
            "You do not have enough yp to transfer {}. Your balance is {}.",
            amount, user.cash_balance
        );
        command
            .create_response(&ctx.http, utils::text_interaction_response(&msg, true))
            .await?;
        return Ok(());
    }

    let create = CreateTransfer {
        amount,
        sender: user.id,
        receiver: recipient.id,
        memo: memo_value.to_string(),
        source: TransferSource::UserInitiated,
    };

    store::transfer::persist_transfer(&mut tx, &create).await?;

    tx.commit().await?;

    let confirm_msg = format!("Transferred {} to {}", amount, recipient.name);
    command
        .create_response(
            &ctx.http,
            utils::text_interaction_response(&confirm_msg, true),
        )
        .await?;

    Ok(())
}
