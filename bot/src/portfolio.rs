use std::collections::HashMap;

use crate::{
    trade::{self, calc_sell_prices},
    ui,
};
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
    pub net_user_transfers: Currency,
    pub tips: Currency,
    pub positions_value: Currency,
}

type TransferValueMap = HashMap<(i64, TransferSource, TransferDirection), Currency>;

fn get_net_value(
    net_transfers: &TransferValueMap,
    source: TransferSource,
    user: &DbUser,
) -> Currency {
    let credit = *net_transfers
        .get(&(user.id, source, TransferDirection::Credit))
        .unwrap_or(&Currency::from(0));

    let debit = *net_transfers
        .get(&(user.id, source, TransferDirection::Debit))
        .unwrap_or(&Currency::from(0));

    credit + debit
}

impl PortfolioValue {
    pub fn new(
        user: DbUser,
        net_transfers: &TransferValueMap,
        positions: &Vec<PositionWithMarketId>,
        market_shares: &HashMap<i64, Vec<InstrumentWithShares>>,
    ) -> anyhow::Result<Self> {
        let net_deposits = get_net_value(net_transfers, TransferSource::Deposit, &user);
        let trades_profit = get_net_value(net_transfers, TransferSource::Order, &user);
        let fees_profit = get_net_value(net_transfers, TransferSource::TradeFee, &user);
        let net_user_transfers = get_net_value(net_transfers, TransferSource::UserInitiated, &user);
        let gambling_winnings = get_net_value(net_transfers, TransferSource::Gambling, &user);
        let tips = get_net_value(net_transfers, TransferSource::MessageTip, &user);

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
            gambling_winnings,
            net_user_transfers,
            tips,
            positions_value: net_position_value?,
        })
    }

    pub fn net_profit(&self) -> Currency {
        self.trades_profit + self.fees_profit + self.positions_value
    }

    pub fn deposits(&self) -> Currency {
        self.net_deposits + self.net_user_transfers
    }

    pub fn table_header() -> [String; 7] {
        [
            "User".to_string(),
            "Balance".to_string(),
            "Positions".to_string(),
            "Deposits".to_string(),
            "Gambling".to_string(),
            "Profit".to_string(),
            "Tips".to_string(),
        ]
    }

    pub fn to_table_row(&self) -> [String; 7] {
        [
            ui::user_shortname(&self.user.name),
            self.user.cash_balance.to_string(),
            self.positions_value.to_string(),
            self.deposits().to_string(),
            self.gambling_winnings.to_string(),
            self.net_profit().to_string(),
            self.tips.to_string(),
        ]
    }
}
