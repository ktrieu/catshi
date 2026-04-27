use std::{cmp::Reverse, collections::HashMap};

use serenity::all::{CommandInteraction, Context, CreateCommand};

use crate::{
    Handler,
    currency::Currency,
    portfolio::PortfolioValue,
    store::{
        self, instrument::InstrumentWithShares, position::PositionWithMarketId,
        transfer::TransferSource, user::DbUser,
    },
    ui::tabulate,
    utils,
};

pub const NAME: &'static str = "leaderboard";

pub fn create() -> CreateCommand<'static> {
    CreateCommand::new(NAME).description("view net worth of all users")
}

pub async fn run(
    ctx: &Context,
    handler: &Handler,
    command: &CommandInteraction,
) -> anyhow::Result<()> {
    let mut tx = handler.pool.begin().await?;

    let transfers = store::transfer::get_net_user_transfers_by_source(&mut *tx).await?;

    // Get our list of users by going through all unique users from the transfers.
    let mut users: HashMap<i64, DbUser> = HashMap::new();
    for t in transfers.iter() {
        users.entry(t.user.id).or_insert(t.user.clone());
    }

    // Process transfers into a HashMap by user and transaction type
    let transfers: HashMap<(i64, TransferSource), Currency> = transfers
        .into_iter()
        .map(|t| ((t.user.id, t.source), t.net))
        .collect();

    let positions = store::position::get_all_positions_with_market_id(&mut *tx).await?;
    let mut positions_by_user: HashMap<i64, Vec<PositionWithMarketId>> = HashMap::new();
    // Process positions into a HashMap of lists per user.
    for p in positions {
        // TODO: remove this check once we clear out empty positions
        if p.position.quantity != 0 {
            positions_by_user
                .entry(p.position.owner_id)
                .or_insert_with(Vec::new)
                .push(p);
        }
    }

    let instruments =
        store::instrument::get_all_open_instruments_with_share_counts(&mut *tx).await?;
    let mut instruments_by_market: HashMap<i64, Vec<InstrumentWithShares>> = HashMap::new();
    for i in instruments {
        instruments_by_market
            .entry(i.0.market_id)
            .or_insert_with(Vec::new)
            .push(i);
    }

    let empty = Vec::new();
    let mut portfolio_values = Vec::new();

    for user in users.into_values() {
        let positions = positions_by_user.get(&user.id).unwrap_or(&empty);
        portfolio_values.push(PortfolioValue::new(
            user,
            &transfers,
            positions,
            &instruments_by_market,
        )?);
    }

    portfolio_values.sort_by_key(|p| Reverse(p.net_profit()));

    let mut rows: Vec<[String; 6]> = Vec::with_capacity(portfolio_values.len() + 1);
    rows.push(PortfolioValue::table_header());

    for p in portfolio_values {
        rows.push(p.to_table_row());
    }

    let msg = tabulate(rows);

    command
        .create_response(&ctx.http, utils::text_interaction_response(&msg, false))
        .await?;

    Ok(())
}
