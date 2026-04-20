use crate::{
    currency::Currency,
    store::{
        instrument::{Instrument, InstrumentWithShares},
        market::{FullMarket, Market},
        order::{CreateOrder, OrderDirection},
        position::{CreatePosition, Position, PositionWithUser},
        transfer::CreateTransfer,
        user::DbUser,
    },
    ui::instrument_display_text,
};

fn create_transfer(
    sender: &DbUser,
    receiver: &DbUser,
    amount: Currency,
    memo: &str,
) -> CreateTransfer {
    CreateTransfer {
        amount,
        sender: sender.id,
        receiver: receiver.id,
        memo: memo.to_owned(),
    }
}

fn create_system_debit(
    user: &DbUser,
    system_user: &DbUser,
    amount: Currency,
    memo: &str,
) -> CreateTransfer {
    create_transfer(user, system_user, amount, memo)
}

pub fn create_system_credit(
    user: &DbUser,
    system_user: &DbUser,
    amount: Currency,
    memo: &str,
) -> CreateTransfer {
    create_transfer(system_user, user, amount, memo)
}

pub const MARKET_B: f32 = 70.0f32;

fn cost(share_counts: impl IntoIterator<Item = i64>, b: f32) -> f32 {
    let summed: f32 = share_counts.into_iter().map(|s| (s as f32 / b).exp()).sum();

    b * summed.ln()
}

fn calc_cost_delta<'s>(
    quantity: i64,
    instrument_id: i64,
    shares: impl Iterator<Item = &'s InstrumentWithShares> + Clone,
    b: f32,
) -> Currency {
    // Pre cost is the current state of the market.
    let pre_cost = cost(shares.clone().map(|(_, qty)| *qty), b);

    // Post cost is the market + the potentially bought shares.
    let post_shares = shares.clone().map(|(instrument, count)| {
        if instrument.id == instrument_id {
            count + quantity
        } else {
            *count
        }
    });

    let post_cost = cost(post_shares, b);

    let total_price = post_cost - pre_cost;

    Currency::from_instrument_price(total_price)
}

fn calc_price_raw<'s>(
    instrument_id: i64,
    shares: impl Iterator<Item = &'s InstrumentWithShares> + Clone,
    b: f32,
) -> f32 {
    let all_sum: f32 = shares
        .clone()
        .map(|(_, count)| (*count as f32 / b).exp())
        .sum();

    let selected = shares
        .clone()
        .find(|(instrument, _)| instrument.id == instrument_id)
        .expect("instrument should be in instruments list");

    let selected_price_exp = (selected.1 as f32 / b).exp();

    selected_price_exp / all_sum
}

pub fn calc_price<'s>(
    instrument_id: i64,
    shares: impl Iterator<Item = &'s InstrumentWithShares> + Clone,
    b: f32,
) -> Currency {
    Currency::from_instrument_price(calc_price_raw(instrument_id, shares, b))
}

pub fn get_max_buy_shares<'s>(
    budget: Currency,
    instrument_id: i64,
    shares: impl Iterator<Item = &'s InstrumentWithShares> + Clone,
    b: f32,
) -> (i64, OrderPrices) {
    let price = calc_price_raw(instrument_id, shares.clone(), b);
    let inv_price = 1f32 - price;

    let inner = ((budget.as_instrument_price() / b).exp() - inv_price) / price;

    let raw_max = b * inner.ln();

    // Round down to calculate the max buy.
    let max_shares = raw_max.floor() as i64;

    (
        max_shares,
        calc_buy_prices(max_shares, instrument_id, shares, b),
    )
}

pub fn calc_fees(shares_price: Currency) -> Currency {
    // Flat two percent.
    shares_price * 0.02f32
}

#[derive(Debug, PartialEq, Eq)]
pub struct OrderPrices {
    pub shares_price: Currency,
    pub fees: Currency,
    pub direction: OrderDirection,
}

impl OrderPrices {
    pub fn total(&self) -> Currency {
        match self.direction {
            OrderDirection::Buy => self.shares_price + self.fees,
            OrderDirection::Sell => self.shares_price - self.fees,
        }
    }
}

pub fn calc_buy_prices<'s>(
    quantity: i64,
    instrument_id: i64,
    shares: impl Iterator<Item = &'s InstrumentWithShares> + Clone,
    b: f32,
) -> OrderPrices {
    let shares_price = calc_cost_delta(quantity, instrument_id, shares, b);

    OrderPrices {
        shares_price,
        fees: calc_fees(shares_price),
        direction: OrderDirection::Buy,
    }
}

pub fn calc_sell_prices<'s>(
    quantity: i64,
    instrument_id: i64,
    shares: impl Iterator<Item = &'s InstrumentWithShares> + Clone,
    b: f32,
) -> OrderPrices {
    // A sell would be decreasing the amount of shares, so negate quantity. We receive the corresponding decrease in price
    // so also negate the result.
    let shares_price = -calc_cost_delta(-quantity, instrument_id, shares, b);

    OrderPrices {
        shares_price,
        fees: calc_fees(shares_price),
        direction: OrderDirection::Sell,
    }
}

enum TransferType {
    Buy,
    Sell,
    Resolve,
}

fn format_transfer_memo(
    ty: TransferType,
    is_fees: bool,
    qty: i64,
    instrument: &Instrument,
    market: &Market,
) -> String {
    let mut prefix = match ty {
        TransferType::Buy => "BUY",
        TransferType::Sell => "SELL",
        TransferType::Resolve => "RESOLVE",
    }
    .to_string();

    if is_fees {
        prefix += " FEES";
    };

    let display_text = instrument_display_text(instrument, market);

    format!("{prefix} {qty} shares {display_text}")
}

#[derive(Debug, PartialEq, Eq)]
pub struct TradeResult {
    pub order: CreateOrder,
    pub transfers: Vec<CreateTransfer>,
    pub position: CreatePosition,
    pub prices: OrderPrices,
    pub quantity: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TradeError {
    InsufficientFunds(Currency),
    InsufficientShares,
}

pub fn buy(
    quantity: i64,
    instrument: &Instrument,
    market: &FullMarket,
    existing_position: Option<&Position>,
    user: &DbUser,
    system_user: &DbUser,
    b: f32,
) -> Result<TradeResult, TradeError> {
    let prices = calc_buy_prices(quantity, instrument.id, market.instruments.iter(), b);
    let total = prices.total();

    // Check that we have enough money to actually purchase these shares.
    // To be generous, (and avoid annoying fractional YPs lying around) we'll let people go 1 YP into overdraft.
    let overdraft = user.cash_balance - (total);
    if overdraft < Currency::new_yp(-1) {
        return Err(TradeError::InsufficientFunds(total));
    }

    let order = CreateOrder {
        direction: OrderDirection::Buy,
        quantity,
        cost_basis: total,
        shares_price: prices.shares_price,
        fees: prices.fees,
        instrument_id: instrument.id,
        owner_id: user.id,
    };

    let shares_transfer = create_system_debit(
        user,
        system_user,
        prices.shares_price,
        &format_transfer_memo(TransferType::Buy, false, quantity, instrument, &market.row),
    );

    let fees_transfer = create_transfer(
        user,
        &market.owner,
        prices.fees,
        &format_transfer_memo(TransferType::Buy, true, quantity, instrument, &market.row),
    );

    let existing_cost_basis = existing_position
        .map(|p| p.cost_basis)
        .unwrap_or(Currency::from(0));
    let held_shares = existing_position.map(|p| p.quantity).unwrap_or(0);

    let position = CreatePosition {
        quantity: held_shares + quantity,
        cost_basis: existing_cost_basis + prices.total(),
        instrument_id: instrument.id,
        owner_id: user.id,
    };

    Ok(TradeResult {
        order,
        transfers: vec![shares_transfer, fees_transfer],
        position,
        prices,
        quantity,
    })
}

pub fn sell(
    quantity: i64,
    instrument: &Instrument,
    market: &FullMarket,
    existing_position: Option<&Position>,
    user: &DbUser,
    system_user: &DbUser,
    b: f32,
) -> Result<TradeResult, TradeError> {
    let prices = calc_sell_prices(quantity, instrument.id, market.instruments.iter(), b);

    let position = match existing_position {
        Some(position) => position,
        None => {
            return Err(TradeError::InsufficientShares);
        }
    };

    // Important! Check and make sure we have enough shares to sell!
    if position.quantity < quantity {
        return Err(TradeError::InsufficientShares);
    }

    let sold_ratio = quantity as f32 / position.quantity as f32;
    let new_cost_basis = position.cost_basis * (1.0f32 - sold_ratio);
    let order_cost_basis = position.cost_basis - new_cost_basis;

    let order = CreateOrder {
        direction: OrderDirection::Sell,
        quantity,
        cost_basis: order_cost_basis,
        shares_price: prices.shares_price,
        fees: prices.fees,
        instrument_id: instrument.id,
        owner_id: user.id,
    };

    let shares_transfer = create_system_credit(
        user,
        system_user,
        prices.shares_price,
        &format_transfer_memo(TransferType::Sell, false, quantity, instrument, &market.row),
    );

    let fees_transfer = create_transfer(
        user,
        &market.owner,
        prices.fees,
        &format_transfer_memo(TransferType::Sell, true, quantity, instrument, &market.row),
    );

    let position = CreatePosition {
        quantity: position.quantity - quantity,
        cost_basis: new_cost_basis,
        instrument_id: instrument.id,
        owner_id: user.id,
    };

    Ok(TradeResult {
        order,
        position,
        transfers: vec![shares_transfer, fees_transfer],
        prices,
        quantity,
    })
}

#[derive(Debug)]
pub struct ResolveResult {
    pub order: CreateOrder,
    pub transfers: Vec<CreateTransfer>,
    pub position: CreatePosition,
    pub shares_price: Currency,
    pub fees: Currency,
    pub cost_basis: Currency,
}

impl ResolveResult {
    pub fn profit(&self) -> Currency {
        self.shares_price - self.fees - self.cost_basis
    }
}

pub fn resolve(
    market: &FullMarket,
    winner: &Instrument,
    positions: &Vec<PositionWithUser>,
    system_user: &DbUser,
) -> anyhow::Result<Vec<ResolveResult>> {
    let mut results: Vec<ResolveResult> = Vec::new();

    for p in positions {
        let position = &p.position;
        let user = &p.user;
        let instrument = &market.get_instrument(position.instrument_id)?.0;

        // We're closing out the whole position.
        let quantity = position.quantity;
        let cost_basis = position.cost_basis;
        let is_winning_position = position.instrument_id == winner.id;

        let resolve_price = if is_winning_position {
            Currency::from_instrument_price(1.0)
        } else {
            Currency::from_instrument_price(0.0)
        };

        let shares_price = resolve_price * position.quantity;
        let prices = OrderPrices {
            shares_price,
            fees: calc_fees(shares_price),
            direction: OrderDirection::Sell,
        };

        let order = CreateOrder {
            direction: OrderDirection::Sell,
            quantity,
            shares_price,
            fees: prices.fees,
            cost_basis,
            instrument_id: instrument.id,
            owner_id: user.id,
        };

        let mut transfers = Vec::new();

        if is_winning_position {
            let shares_transfer = create_system_credit(
                user,
                system_user,
                shares_price,
                &format_transfer_memo(
                    TransferType::Resolve,
                    false,
                    quantity,
                    &instrument,
                    &market.row,
                ),
            );

            let fees_transfer = create_transfer(
                user,
                &market.owner,
                prices.fees,
                &format_transfer_memo(
                    TransferType::Resolve,
                    true,
                    quantity,
                    &instrument,
                    &market.row,
                ),
            );

            transfers.push(shares_transfer);
            transfers.push(fees_transfer);
        }

        // Create a new closed out position.
        let position = CreatePosition {
            quantity: 0,
            cost_basis: Currency::from(0),
            instrument_id: position.instrument_id,
            owner_id: user.id,
        };

        results.push(ResolveResult {
            order,
            transfers,
            position,
            shares_price,
            fees: prices.fees,
            cost_basis,
        });
    }

    Ok(results)
}

#[cfg(test)]
mod test {
    use crate::store::{
        instrument::InstrumentState,
        market::{Market, MarketState},
    };

    use super::*;

    fn test_trade_data() -> (FullMarket, DbUser, DbUser) {
        let market_owner = DbUser {
            id: 0,
            discord_id: "000".to_string(),
            name: "market_owner".to_string(),
            cash_balance: Currency::from(0),
        };

        let market = FullMarket {
            row: Market {
                id: 0,
                description: "test".to_string(),
                state: MarketState::Open,
                owner_id: market_owner.id,
                message_id: Some("0".to_string()),
                channel_id: Some("0".to_string()),
            },
            instruments: vec![
                (
                    Instrument {
                        id: 0,
                        name: "yes".to_string(),
                        state: InstrumentState::Open,
                        market_id: 0,
                    },
                    0,
                ),
                (
                    Instrument {
                        id: 1,
                        name: "no".to_string(),
                        state: InstrumentState::Open,
                        market_id: 0,
                    },
                    0,
                ),
            ],
            owner: market_owner,
        };

        let system_user = DbUser {
            id: 1,
            discord_id: "0".to_string(),
            name: "system_user".to_string(),
            cash_balance: Currency::from(0),
        };

        let user = DbUser {
            id: 2,
            discord_id: "222".to_string(),
            name: "user".to_string(),
            cash_balance: Currency::new_yp(20),
        };

        (market, system_user, user)
    }

    #[test]
    fn test_buy_no_position() {
        let (market, system_user, user) = test_trade_data();
        let instrument = &market.instruments[0].0;

        let qty = 5;

        let result = buy(
            qty,
            instrument,
            &market,
            None,
            &user,
            &system_user,
            MARKET_B,
        )
        .expect("buy should succeed");

        let prices = calc_buy_prices(qty, instrument.id, market.instruments.iter(), MARKET_B);
        // Don't directly assert the prices - just assert that it matches our internal calculation.
        // We'll test the math separately.
        assert_eq!(result.prices, prices);

        let expected_order = CreateOrder {
            direction: OrderDirection::Buy,
            quantity: qty,
            shares_price: prices.shares_price,
            fees: prices.fees,
            cost_basis: prices.shares_price + prices.fees,
            instrument_id: instrument.id,
            owner_id: user.id,
        };
        assert_eq!(result.order, expected_order);

        let expected_position = CreatePosition {
            quantity: qty,
            cost_basis: prices.shares_price + prices.fees,
            instrument_id: instrument.id,
            owner_id: user.id,
        };
        assert_eq!(result.position, expected_position);

        assert_eq!(result.quantity, qty);
    }

    #[test]
    fn test_buy_existing_position() {
        let (market, system_user, user) = test_trade_data();
        let instrument = &market.instruments[0].0;

        let existing_position = Position {
            id: 0,
            quantity: 10,
            cost_basis: Currency::new_yp(2),
            instrument_id: instrument.id,
            owner_id: user.id,
        };

        let qty = 5;

        let result = buy(
            qty,
            instrument,
            &market,
            Some(&existing_position),
            &user,
            &system_user,
            MARKET_B,
        )
        .expect("buy should succeed");

        let prices = calc_buy_prices(qty, instrument.id, market.instruments.iter(), MARKET_B);
        // Don't directly assert the prices - just assert that it matches our internal calculation.
        // We'll test the math separately.
        assert_eq!(result.prices, prices);

        let expected_order = CreateOrder {
            direction: OrderDirection::Buy,
            quantity: qty,
            shares_price: prices.shares_price,
            fees: prices.fees,
            cost_basis: prices.shares_price + prices.fees,
            instrument_id: instrument.id,
            owner_id: user.id,
        };
        assert_eq!(result.order, expected_order);

        let expected_position = CreatePosition {
            quantity: qty + existing_position.quantity,
            cost_basis: existing_position.cost_basis + prices.shares_price + prices.fees,
            instrument_id: instrument.id,
            owner_id: user.id,
        };
        assert_eq!(result.position, expected_position);

        assert_eq!(result.quantity, qty);
    }

    #[test]
    fn test_buy_transfers() {
        let (market, system_user, user) = test_trade_data();
        let instrument = &market.instruments[0].0;

        let qty = 5;

        let result = buy(
            qty,
            instrument,
            &market,
            None,
            &user,
            &system_user,
            MARKET_B,
        )
        .expect("buy should succeed");

        let prices = calc_buy_prices(qty, instrument.id, market.instruments.iter(), MARKET_B);

        let shares_transfer = CreateTransfer {
            amount: prices.shares_price,
            sender: user.id,
            receiver: system_user.id,
            memo: format_transfer_memo(TransferType::Buy, false, qty, instrument, &market.row),
        };
        assert!(result.transfers.contains(&shares_transfer));

        let fees_transfer = CreateTransfer {
            amount: prices.fees,
            sender: user.id,
            receiver: market.owner.id,
            memo: format_transfer_memo(TransferType::Buy, true, qty, instrument, &market.row),
        };
        assert!(result.transfers.contains(&fees_transfer));
    }

    #[test]
    fn test_buy_insufficient_funds() {
        let (market, system_user, mut user) = test_trade_data();
        let instrument = &market.instruments[0].0;

        let qty = 5;
        let prices = calc_buy_prices(qty, instrument.id, market.instruments.iter(), MARKET_B);

        user.cash_balance = Currency::from(0);

        let result = buy(
            qty,
            instrument,
            &market,
            None,
            &user,
            &system_user,
            MARKET_B,
        );

        assert_eq!(
            result,
            Err(TradeError::InsufficientFunds(
                prices.shares_price + prices.fees
            ))
        );
    }

    #[test]
    fn test_buy_insufficient_funds_overdraft() {
        let (market, system_user, mut user) = test_trade_data();
        let instrument = &market.instruments[0].0;

        let qty = 5;
        let prices = calc_buy_prices(qty, instrument.id, market.instruments.iter(), MARKET_B);

        // User has 0.5yp less than they need to complete the purchase. this should be allowed since we allow up to 1.0yp overdraft.
        user.cash_balance = prices.total() - Currency::from(500);

        let result = buy(
            qty,
            instrument,
            &market,
            None,
            &user,
            &system_user,
            MARKET_B,
        );

        result.expect("buy with overdraft should succeed");
    }

    #[test]
    fn test_sell_existing_position() {
        let (market, system_user, user) = test_trade_data();
        let instrument = &market.instruments[0].0;

        let existing_position = Position {
            id: 0,
            quantity: 10,
            cost_basis: Currency::new_yp(2),
            instrument_id: instrument.id,
            owner_id: user.id,
        };

        let qty = 5;

        let result = sell(
            qty,
            instrument,
            &market,
            Some(&existing_position),
            &user,
            &system_user,
            MARKET_B,
        )
        .expect("sell should succeed");

        let prices = calc_sell_prices(qty, instrument.id, market.instruments.iter(), MARKET_B);
        // Don't directly assert the prices - just assert that it matches our internal calculation.
        // We'll test the math separately.
        assert_eq!(result.prices, prices);

        // We're selling 5 shares out of a position of 10. half the cost basis should go to the order, and half
        // should remain in the original position.

        let expected_order = CreateOrder {
            direction: OrderDirection::Sell,
            quantity: qty,
            shares_price: prices.shares_price,
            fees: prices.fees,
            cost_basis: Currency::new_yp(1),
            instrument_id: instrument.id,
            owner_id: user.id,
        };
        assert_eq!(result.order, expected_order);

        let expected_position = CreatePosition {
            quantity: 5,
            cost_basis: Currency::new_yp(1),
            instrument_id: instrument.id,
            owner_id: user.id,
        };
        assert_eq!(result.position, expected_position);

        assert_eq!(result.quantity, qty);
    }

    #[test]
    fn test_sellout_position() {
        let (market, system_user, user) = test_trade_data();
        let instrument = &market.instruments[0].0;

        let existing_position = Position {
            id: 0,
            quantity: 10,
            cost_basis: Currency::new_yp(2),
            instrument_id: instrument.id,
            owner_id: user.id,
        };

        let qty = 10;

        let result = sell(
            qty,
            instrument,
            &market,
            Some(&existing_position),
            &user,
            &system_user,
            MARKET_B,
        )
        .expect("sell should succeed");

        let prices = calc_sell_prices(qty, instrument.id, market.instruments.iter(), MARKET_B);
        // Don't directly assert the prices - just assert that it matches our internal calculation.
        // We'll test the math separately.
        assert_eq!(result.prices, prices);

        let expected_order = CreateOrder {
            direction: OrderDirection::Sell,
            quantity: qty,
            shares_price: prices.shares_price,
            fees: prices.fees,
            cost_basis: Currency::new_yp(2),
            instrument_id: instrument.id,
            owner_id: user.id,
        };
        assert_eq!(result.order, expected_order);

        let expected_position = CreatePosition {
            quantity: 0,
            cost_basis: Currency::new_yp(0),
            instrument_id: instrument.id,
            owner_id: user.id,
        };
        assert_eq!(result.position, expected_position);

        assert_eq!(result.quantity, qty);
    }

    #[test]
    fn test_sell_transfers() {
        let (market, system_user, user) = test_trade_data();
        let instrument = &market.instruments[0].0;

        let existing_position = Position {
            id: 0,
            quantity: 10,
            cost_basis: Currency::new_yp(2),
            instrument_id: instrument.id,
            owner_id: user.id,
        };

        let qty = 5;

        let result = sell(
            qty,
            instrument,
            &market,
            Some(&existing_position),
            &user,
            &system_user,
            MARKET_B,
        )
        .expect("buy should succeed");

        let prices = calc_sell_prices(qty, instrument.id, market.instruments.iter(), MARKET_B);

        let shares_transfer = CreateTransfer {
            amount: prices.shares_price,
            sender: system_user.id,
            receiver: user.id,
            memo: format_transfer_memo(TransferType::Sell, false, qty, instrument, &market.row),
        };
        assert!(result.transfers.contains(&shares_transfer));

        let fees_transfer = CreateTransfer {
            amount: prices.fees,
            sender: user.id,
            receiver: market.owner.id,
            memo: format_transfer_memo(TransferType::Sell, true, qty, instrument, &market.row),
        };
        assert!(result.transfers.contains(&fees_transfer));
    }

    #[test]
    fn test_insufficient_shares_existing_position() {
        let (market, system_user, user) = test_trade_data();
        let instrument = &market.instruments[0].0;

        let existing_position = Position {
            id: 0,
            quantity: 10,
            cost_basis: Currency::new_yp(2),
            instrument_id: instrument.id,
            owner_id: user.id,
        };

        let qty = 15;

        let result = sell(
            qty,
            instrument,
            &market,
            Some(&existing_position),
            &user,
            &system_user,
            MARKET_B,
        );

        assert_eq!(result, Err(TradeError::InsufficientShares));
    }

    #[test]
    fn test_insufficient_shares_no_position() {
        let (market, system_user, user) = test_trade_data();
        let instrument = &market.instruments[0].0;

        let qty = 15;

        let result = sell(
            qty,
            instrument,
            &market,
            None,
            &user,
            &system_user,
            MARKET_B,
        );

        assert_eq!(result, Err(TradeError::InsufficientShares));
    }

    #[test]
    fn test_resolve_no_positions() {
        let (market, system_user, _user) = test_trade_data();
        let instrument = &market.instruments[0].0;

        let result = resolve(&market, instrument, &Vec::new(), &system_user)
            .expect("resolve should succeed");

        assert!(result.len() == 0);
    }

    #[test]
    fn test_resolve_winning_position() {
        let (market, system_user, user) = test_trade_data();
        let instrument = &market.instruments[0].0;

        let positions = vec![PositionWithUser {
            position: Position {
                id: 0,
                quantity: 10,
                cost_basis: Currency::new_yp(2),
                instrument_id: instrument.id,
                owner_id: user.id,
            },
            user: user.clone(),
        }];

        let result =
            resolve(&market, instrument, &positions, &system_user).expect("resolve should succeed");

        assert!(result.len() == 1);
        let result = &result[0];

        let expected_order = CreateOrder {
            direction: OrderDirection::Sell,
            quantity: 10,
            shares_price: Currency::new_yp(10),
            fees: Currency::new_yp(10) * 0.02,
            cost_basis: Currency::new_yp(2),
            instrument_id: instrument.id,
            owner_id: user.id,
        };
        assert_eq!(result.order, expected_order);

        let shares_transfer = CreateTransfer {
            amount: Currency::new_yp(10),
            sender: system_user.id,
            receiver: user.id,
            memo: format_transfer_memo(TransferType::Resolve, false, 10, instrument, &market.row),
        };
        assert!(result.transfers.contains(&shares_transfer));

        let fees_transfer = CreateTransfer {
            amount: Currency::new_yp(10) * 0.02,
            sender: user.id,
            receiver: market.owner.id,
            memo: format_transfer_memo(TransferType::Resolve, true, 10, instrument, &market.row),
        };
        assert!(result.transfers.contains(&fees_transfer));
    }

    #[test]
    fn test_resolve_losing_position() {
        let (market, system_user, user) = test_trade_data();
        let winner = &market.instruments[0].0;
        let loser = &market.instruments[1].0;

        let positions = vec![PositionWithUser {
            position: Position {
                id: 0,
                quantity: 10,
                cost_basis: Currency::new_yp(2),
                instrument_id: loser.id,
                owner_id: user.id,
            },
            user: user.clone(),
        }];

        let result =
            resolve(&market, winner, &positions, &system_user).expect("resolve should succeed");

        assert!(result.len() == 1);
        let result = &result[0];

        let expected_order = CreateOrder {
            direction: OrderDirection::Sell,
            quantity: 10,
            shares_price: Currency::new_yp(0),
            fees: Currency::new_yp(0),
            cost_basis: Currency::new_yp(2),
            instrument_id: loser.id,
            owner_id: user.id,
        };
        assert_eq!(result.order, expected_order);
        assert_eq!(result.transfers.len(), 0);
    }

    #[test]
    fn test_resolve_multi_positions() {
        let (market, system_user, user) = test_trade_data();
        let winner = &market.instruments[0].0;
        let loser = &market.instruments[1].0;

        let other_user = DbUser {
            id: 3,
            discord_id: "333".to_string(),
            name: "other_user".to_string(),
            cash_balance: Currency::new_yp(2),
        };

        let positions = vec![
            PositionWithUser {
                position: Position {
                    id: 0,
                    quantity: 10,
                    cost_basis: Currency::new_yp(2),
                    instrument_id: winner.id,
                    owner_id: user.id,
                },
                user: user.clone(),
            },
            PositionWithUser {
                position: Position {
                    id: 0,
                    quantity: 10,
                    cost_basis: Currency::new_yp(2),
                    instrument_id: loser.id,
                    owner_id: user.id,
                },
                user: user.clone(),
            },
            PositionWithUser {
                position: Position {
                    id: 0,
                    quantity: 10,
                    cost_basis: Currency::new_yp(2),
                    instrument_id: winner.id,
                    owner_id: other_user.id,
                },
                user: other_user.clone(),
            },
        ];

        let results =
            resolve(&market, winner, &positions, &system_user).expect("resolve should succeed");

        assert!(results.len() == 3);

        // First position
        let result = &results[0];
        let expected_order = CreateOrder {
            direction: OrderDirection::Sell,
            quantity: 10,
            shares_price: Currency::new_yp(10),
            fees: Currency::new_yp(10) * 0.02,
            cost_basis: Currency::new_yp(2),
            instrument_id: winner.id,
            owner_id: user.id,
        };
        assert_eq!(result.order, expected_order);

        let shares_transfer = CreateTransfer {
            amount: Currency::new_yp(10),
            sender: system_user.id,
            receiver: user.id,
            memo: format_transfer_memo(TransferType::Resolve, false, 10, winner, &market.row),
        };
        assert!(result.transfers.contains(&shares_transfer));

        let fees_transfer = CreateTransfer {
            amount: Currency::new_yp(10) * 0.02,
            sender: user.id,
            receiver: market.owner.id,
            memo: format_transfer_memo(TransferType::Resolve, true, 10, winner, &market.row),
        };
        assert!(result.transfers.contains(&fees_transfer));

        // Second position
        let result = &results[1];
        let expected_order = CreateOrder {
            direction: OrderDirection::Sell,
            quantity: 10,
            shares_price: Currency::new_yp(0),
            fees: Currency::new_yp(0),
            cost_basis: Currency::new_yp(2),
            instrument_id: loser.id,
            owner_id: user.id,
        };
        assert_eq!(result.order, expected_order);
        assert_eq!(result.transfers.len(), 0);

        // Third position
        let result = &results[2];
        let expected_order = CreateOrder {
            direction: OrderDirection::Sell,
            quantity: 10,
            shares_price: Currency::new_yp(10),
            fees: Currency::new_yp(10) * 0.02,
            cost_basis: Currency::new_yp(2),
            instrument_id: winner.id,
            owner_id: other_user.id,
        };
        assert_eq!(result.order, expected_order);

        let shares_transfer = CreateTransfer {
            amount: Currency::new_yp(10),
            sender: system_user.id,
            receiver: other_user.id,
            memo: format_transfer_memo(TransferType::Resolve, false, 10, winner, &market.row),
        };
        assert!(result.transfers.contains(&shares_transfer));

        let fees_transfer = CreateTransfer {
            amount: Currency::new_yp(10) * 0.02,
            sender: other_user.id,
            receiver: market.owner.id,
            memo: format_transfer_memo(TransferType::Resolve, true, 10, winner, &market.row),
        };
        assert!(result.transfers.contains(&fees_transfer));
    }
}
