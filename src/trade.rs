use std::collections::HashMap;

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

pub fn calc_buy_shares_price(
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
    let shares_price =
        calc_buy_shares_price(quantity, instrument.id, &outstanding_shares, MARKET_B);

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
