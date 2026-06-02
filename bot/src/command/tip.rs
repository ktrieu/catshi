use log::info;
use serenity::all::{Context, Message};

use common::currency::Currency;
use common::store::{
    self,
    transfer::{CreateTransfer, TransferSource},
    user::DbUser,
};

use crate::Handler;

// Tip amount is 1 yp = 1000 bips.
const TIP_AMOUNT: i64 = 1000;

pub async fn on_tip(
    ctx: &Context,
    handler: &Handler,
    user: &DbUser,
    message: &Message,
) -> anyhow::Result<()> {
    let amount = Currency::from(TIP_AMOUNT);

    if user.cash_balance < amount {
        // TODO: Make this user visible somehow. DM the user? Send an insulting public message?
        info!(
            "Rejecting tip for message {} and user {}, NSF",
            message.id, user.name
        );
        return Ok(());
    }

    let receiver = handler.authenticate(ctx, &message.author).await?;

    let mut conn = handler.pool.acquire().await?;
    let existing =
        store::tip::get_tip_by_message_and_user(&mut conn, user, message.channel_id, message.id)
            .await?;
    if existing.is_some() {
        // We've already tipped for this message.
        return Ok(());
    }

    let mut tx = handler.pool.begin_with("BEGIN IMMEDIATE").await?;

    let create_transfer = CreateTransfer {
        amount,
        sender: user.id,
        receiver: receiver.id,
        memo: "Tip for message".to_string(),
        source: TransferSource::MessageTip,
    };

    let transfer = store::transfer::persist_transfer(&mut tx, &create_transfer).await?;
    store::tip::create_tip(
        &mut *tx,
        amount,
        &transfer,
        &user,
        message.channel_id,
        message.id,
    )
    .await?;

    tx.commit().await?;

    Ok(())
}
