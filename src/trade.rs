use std::collections::HashMap;

use anyhow::bail;
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::{
    currency::Currency,
    store::{self, CreateTransfer, DbUser, Instrument},
};

pub const MARKET_B: f32 = 10.0f32;

fn cost(share_counts: impl IntoIterator<Item = i64>, b: f32) -> f32 {
    let summed: f32 = share_counts.into_iter().map(|s| (s as f32 / b).exp()).sum();

    b * summed.ln()
}

pub fn calc_cost_delta(
    quantity: i64,
    instrument_id: i64,
    shares: &HashMap<i64, i64>,
    b: f32,
) -> Currency {
    // Pre cost is the current state of the market.
    let pre_cost = cost(shares.values().cloned(), b);

    // Post cost is the market + the potentially bought shares.
    let post_shares = shares.iter().map(|(id, count)| {
        if *id == instrument_id {
            *count + quantity
        } else {
            *count
        }
    });

    let post_cost = cost(post_shares, b);

    let total_price = post_cost - pre_cost;

    Currency::from_instrument_price(total_price)
}

fn calc_fees(shares_price: Currency) -> Currency {
    // Flat two percent.
    shares_price * 0.02f32
}

pub struct BuyResult {
    pub shares_price: Currency,
    pub fees: Currency,
}

impl BuyResult {
    pub fn total(&self) -> Currency {
        self.shares_price + self.fees
    }
}

async fn transfer_cash(
    tx: &mut Transaction<'_, Sqlite>,
    sender: &DbUser,
    receiver: &DbUser,
    amount: Currency,
    memo: &str,
) -> anyhow::Result<()> {
    // 1. Create the transfer.
    let create = CreateTransfer {
        amount,
        sender: sender.id,
        receiver: receiver.id,
        memo: memo.to_owned(),
    };
    store::insert_transfer(&mut **tx, create).await?;

    // 2. Credit the receiving account.
    store::increment_balance(&mut **tx, receiver, amount).await?;

    // 3. Debit the sending account.
    store::increment_balance(&mut **tx, sender, -amount).await?;

    Ok(())
}

async fn system_credit_user(
    tx: &mut Transaction<'_, Sqlite>,
    user: &DbUser,
    system_user: &DbUser,
    amount: Currency,
    memo: &str,
) -> anyhow::Result<()> {
    transfer_cash(tx, system_user, user, amount, memo).await?;

    Ok(())
}

async fn system_debit_user(
    tx: &mut Transaction<'_, Sqlite>,
    user: &DbUser,
    system_user: &DbUser,
    amount: Currency,
    memo: &str,
) -> anyhow::Result<()> {
    transfer_cash(tx, user, system_user, amount, memo).await?;

    Ok(())
}

pub async fn buy(
    pool: &SqlitePool,
    quantity: i64,
    instrument: &Instrument,
    user: &DbUser,
    system_user: &DbUser,
) -> anyhow::Result<BuyResult> {
    let mut tx = pool.begin().await?;

    let outstanding_shares =
        store::get_outstanding_shares_for_market(&mut *tx, instrument.market_id).await?;

    // Simple MVP behaviour here: buy 1 share.
    let shares_price = calc_cost_delta(quantity, instrument.id, &outstanding_shares, MARKET_B);

    let fees = calc_fees(shares_price);
    let total_price = shares_price + fees;

    let existing = store::get_user_position(&mut *tx, &instrument, &user).await?;
    if existing.is_none() {
        store::create_new_position(&mut *tx, quantity, total_price, &instrument, &user).await?;
    } else {
        store::increase_position(&mut *tx, quantity, total_price, &instrument, &user).await?;
    }

    store::create_order(
        &mut *tx,
        store::OrderDirection::Buy,
        quantity,
        shares_price,
        fees,        // no fees for now.
        total_price, // cost basis is the same as shares_price for buys.
        instrument,
        user,
    )
    .await?;

    // Make the user pay for their shares.
    system_debit_user(
        &mut tx,
        user,
        system_user,
        shares_price,
        format!("BUY {} shares {}", quantity, instrument.name).as_str(),
    )
    .await?;

    // And make the user pay their fees. TODO: direct this to the market owner instead.
    transfer_cash(
        &mut tx,
        user,
        system_user,
        fees,
        format!("BUY {} shares {} fees", quantity, instrument.name).as_str(),
    )
    .await?;

    tx.commit().await?;

    Ok(BuyResult { shares_price, fees })
}

pub struct SellResult {
    pub shares_price: Currency,
    pub order_cost_basis: Currency,
    pub fees: Currency,
}

impl SellResult {
    pub fn profit(&self) -> Currency {
        self.shares_price - self.fees - self.order_cost_basis
    }

    pub fn net(&self) -> Currency {
        self.shares_price - self.fees
    }
}

pub async fn sell(
    pool: &SqlitePool,
    quantity: i64,
    instrument: &Instrument,
    user: &DbUser,
    system_user: &DbUser,
) -> anyhow::Result<SellResult> {
    let mut tx = pool.begin().await?;

    let outstanding_shares =
        store::get_outstanding_shares_for_market(&mut *tx, instrument.market_id).await?;

    // A sell would be decreasing the amount of shares, so negate quantity. We receive the corresponding decrease in price
    // so also negate the result.
    let shares_price = -calc_cost_delta(-quantity, instrument.id, &outstanding_shares, MARKET_B);

    // No fees for now.
    let fees = calc_fees(shares_price);

    let position = match store::get_user_position(&mut *tx, &instrument, &user).await? {
        Some(position) => position,
        None => {
            // Can't sell if there's no position. Raise an error here. Caller should catch this
            // and display a more graceful message, however.
            bail!(
                "no position to sell for user {}, instrument {}",
                user.id,
                instrument.id
            );
        }
    };

    // Important! Check and make sure we have enough shares to sell!
    if position.quantity < quantity {
        bail!(
            "insufficient shares to sell for user {}, instrument {}. tried to sell {} but only had {}",
            user.id,
            instrument.id,
            quantity,
            position.quantity
        )
    }

    // Scale down the cost-basis uniformly based on the proportion of shares we are selling.
    let new_position_cost_basis =
        position.cost_basis * (1f32 - (quantity as f32 / position.quantity as f32));

    store::decrease_position(
        &mut *tx,
        quantity,
        new_position_cost_basis,
        &instrument,
        &user,
    )
    .await?;

    // The cost basis of the shares we sold is what's remaining from the original position's cost basis.
    let order_cost_basis = position.cost_basis - new_position_cost_basis;

    store::create_order(
        &mut *tx,
        store::OrderDirection::Sell,
        quantity,
        shares_price,
        fees,
        order_cost_basis,
        instrument,
        user,
    )
    .await?;

    // Give the user their proceeds.
    system_credit_user(
        &mut tx,
        user,
        system_user,
        shares_price,
        format!("SELL {} shares {}", quantity, instrument.name).as_str(),
    )
    .await?;

    // And make the user pay their fees. TODO: direct this to the market owner instead.
    transfer_cash(
        &mut tx,
        user,
        system_user,
        fees,
        format!("BUY {} shares {} fees", quantity, instrument.name).as_str(),
    )
    .await?;

    tx.commit().await?;

    Ok(SellResult {
        shares_price,
        fees,
        order_cost_basis,
    })
}
