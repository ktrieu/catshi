use std::collections::HashMap;

use crate::trade::{self, calc_sell_prices};
use anyhow::anyhow;
use common::currency::Currency;
use common::store::{
    instrument::InstrumentWithShares,
    position::PositionWithMarketId,
    transfer::{TransferDirection, TransferSource},
    user::DbUser,
};

pub struct PortfolioValue {
    pub user: DbUser,
    pub net_deposits: Currency,
    pub trades_profit: Currency,
    pub fees_profit: Currency,
    pub gambling_winnings: Currency,
    pub tips_received: Currency,
    pub tips_sent: Currency,
    pub transfers_received: Currency,
    pub transfers_sent: Currency,
    pub positions_value: Currency,
}

type TransferValueMap = HashMap<(i32, TransferSource, TransferDirection), Currency>;

fn get_transfer_value(
    net_transfers: &TransferValueMap,
    source: TransferSource,
    user: &DbUser,
) -> (Currency, Currency, Currency) {
    let credit = *net_transfers
        .get(&(user.id, source, TransferDirection::Credit))
        .unwrap_or(&Currency::from(0));

    let debit = *net_transfers
        .get(&(user.id, source, TransferDirection::Debit))
        .unwrap_or(&Currency::from(0));

    (debit, credit, credit + debit)
}

impl PortfolioValue {
    pub fn new(
        user: DbUser,
        net_transfers: &TransferValueMap,
        positions: &Vec<PositionWithMarketId>,
        market_shares: &HashMap<i64, Vec<InstrumentWithShares>>,
    ) -> anyhow::Result<Self> {
        let (_, __, net_deposits) =
            get_transfer_value(net_transfers, TransferSource::Deposit, &user);
        let (_, _, trades_profit) = get_transfer_value(net_transfers, TransferSource::Order, &user);
        let (_, _, fees_profit) =
            get_transfer_value(net_transfers, TransferSource::TradeFee, &user);
        let (transfers_sent, transfers_received, _) =
            get_transfer_value(net_transfers, TransferSource::UserInitiated, &user);
        let (_, _, gambling_winnings) =
            get_transfer_value(net_transfers, TransferSource::Gambling, &user);
        let (tips_sent, tips_received, _) =
            get_transfer_value(net_transfers, TransferSource::MessageTip, &user);

        let net_position_value: anyhow::Result<Currency> = positions
            .iter()
            .map(|p| -> anyhow::Result<Currency> {
                let market_id = &p.market_id;
                let instruments = market_shares.get(&p.market_id).ok_or(anyhow!(
                    "could not find shares for market {market_id} when calculating portfolio value"
                ))?;

                let price = calc_sell_prices(
                    p.position.quantity,
                    p.position.instrument_id,
                    instruments.iter(),
                    trade::MARKET_B,
                );

                Ok(price.total())
            })
            .sum();

        Ok(Self {
            user,
            net_deposits,
            trades_profit,
            fees_profit,
            positions_value: net_position_value?,
            gambling_winnings,
            tips_received,
            tips_sent,
            transfers_received,
            transfers_sent,
        })
    }

    pub fn net_profit(&self) -> Currency {
        self.trades_profit + self.fees_profit + self.positions_value
    }

    pub fn net_worth(&self) -> Currency {
        self.user.cash_balance + self.positions_value
    }
}
