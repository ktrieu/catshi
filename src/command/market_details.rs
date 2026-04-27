use std::{cmp::Reverse, collections::HashMap};

use serenity::all::{ComponentInteraction, Context};

use crate::{
    Handler,
    store::{self, position::PositionWithUser},
    ui::{self, code_block, tabulate},
    utils,
};

pub async fn view_market_details(
    ctx: &Context,
    handler: &Handler,
    market_id: i64,
    component: &ComponentInteraction,
) -> anyhow::Result<()> {
    let instruments =
        store::instrument::get_instruments_with_share_counts_for_market(&handler.pool, market_id)
            .await?;

    let mut all_positions =
        store::position::get_all_market_positions(&handler.pool, market_id).await?;
    all_positions.sort_by_key(|p| (p.position.instrument_id, p.position.quantity));

    let mut instrument_positions: HashMap<i64, Vec<&PositionWithUser>> = HashMap::new();

    for p in all_positions.iter() {
        if p.position.quantity == 0 {
            continue;
        }

        instrument_positions
            .entry(p.position.instrument_id)
            .or_default()
            .push(p);
    }

    let mut instrument_positions: Vec<(i64, Vec<&PositionWithUser>)> =
        instrument_positions.into_iter().collect();

    instrument_positions.sort_by_key(|(id, _)| *id);

    let mut lines: Vec<String> = Vec::new();

    for (instrument_id, positions) in instrument_positions.iter_mut() {
        let instrument_name = instruments
            .iter()
            .find(|(i, _)| i.id == *instrument_id)
            .expect(&format!("instrument {instrument_id} should exist",))
            .0
            .name
            .clone();

        let mut rows = vec![[
            "User".to_string(),
            "Quantity".to_string(),
            "Paid".to_string(),
        ]];

        positions.sort_by_key(|p| Reverse(p.position.quantity));

        for p in positions {
            rows.push([
                ui::user_shortname(&p.user.name),
                p.position.quantity.to_string(),
                p.position.cost_basis.to_string(),
            ]);
        }
        let result = tabulate(rows);

        lines.push(instrument_name);
        lines.push(result);
    }

    let details = code_block(&lines.join("\n"));

    component
        .create_response(&ctx.http, utils::text_interaction_response(&details, true))
        .await?;

    Ok(())
}
