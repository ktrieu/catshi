use std::collections::HashMap;

use anyhow::bail;
use sqlx::SqlitePool;

use crate::{
    currency::Currency,
    store::{self, DbUser, Instrument},
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

pub struct BuyResult {
    pub total_price: Currency,
}

pub async fn buy(
    pool: &SqlitePool,
    quantity: i64,
    instrument: &Instrument,
    user: &DbUser,
) -> anyhow::Result<BuyResult> {
    let mut tx = pool.begin().await?;

    let outstanding_shares =
        store::get_outstanding_shares_for_market(&mut *tx, instrument.market_id).await?;

    // Simple MVP behaviour here: buy 1 share.
    let shares_price = calc_cost_delta(quantity, instrument.id, &outstanding_shares, MARKET_B);

    // No fees for now.
    let fees = Currency::from(0i64);
    let total = shares_price + fees;

    let existing = store::get_user_position(&mut *tx, &instrument, &user).await?;
    if existing.is_none() {
        store::create_new_position(&mut *tx, quantity, total, &instrument, &user).await?;
    } else {
        store::increase_position(&mut *tx, quantity, total, &instrument, &user).await?;
    }

    store::create_order(
        &mut *tx,
        store::OrderDirection::Buy,
        quantity,
        shares_price,
        fees,  // no fees for now.
        total, // cost basis is the same as shares_price for buys.
        instrument,
        user,
    )
    .await?;

    tx.commit().await?;

    Ok(BuyResult {
        total_price: shares_price,
    })
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
}

pub async fn sell(
    pool: &SqlitePool,
    quantity: i64,
    instrument: &Instrument,
    user: &DbUser,
) -> anyhow::Result<SellResult> {
    let mut tx = pool.begin().await?;

    let outstanding_shares =
        store::get_outstanding_shares_for_market(&mut *tx, instrument.market_id).await?;

    // A sell would be decreasing the amount of shares, so negate quantity. We receive the corresponding decrease in price
    // so also negate the result.
    let shares_price = -calc_cost_delta(-quantity, instrument.id, &outstanding_shares, MARKET_B);

    // No fees for now.
    let fees = Currency::from(0i64);

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

    tx.commit().await?;

    Ok(SellResult {
        shares_price,
        fees,
        order_cost_basis,
    })
}
