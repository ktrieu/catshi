use std::collections::HashMap;

pub const MARKET_B: f32 = 10.0f32;

pub struct BuyResult {
    pub total_price: i64,
}

fn cost(share_counts: impl IntoIterator<Item = i64>, b: f32) -> f32 {
    let summed: f32 = share_counts.into_iter().map(|s| (s as f32 / b).exp()).sum();

    b * summed.ln()
}

pub fn buy(quantity: i64, instrument_id: i64, shares: &HashMap<i64, i64>, b: f32) -> BuyResult {
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

    let total_price = ((post_cost - pre_cost) * 100f32).round() as i64;

    BuyResult { total_price }
}
