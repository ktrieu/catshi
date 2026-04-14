use serenity::all::{ComponentInteraction, Context};

use crate::{
    Handler,
    store::{self},
    ui::tabulate,
    utils,
};

pub async fn view_market_details(
    ctx: &Context,
    handler: &Handler,
    market_id: i64,
    component: &ComponentInteraction,
) -> anyhow::Result<()> {
    let instruments =
        store::get_instruments_with_share_counts_for_market(&handler.pool, market_id).await?;

    let mut all_positions = store::get_all_market_positions(&handler.pool, market_id).await?;
    all_positions.sort_by_key(|p| (p.position.instrument_id, p.position.quantity));

    let mut rows = vec![[
        "User".to_string(),
        "Instrument".to_string(),
        "Quantity".to_string(),
        "Paid".to_string(),
    ]];

    for p in all_positions {
        if p.position.quantity == 0 {
            continue;
        }

        let instrument_name = instruments
            .iter()
            .find(|(i, _)| i.id == p.position.instrument_id)
            .expect(&format!(
                "instrument {} should exist",
                p.position.instrument_id
            ))
            .0
            .name
            .clone();

        rows.push([
            p.user.name.clone().to_string(),
            instrument_name.clone(),
            p.position.quantity.to_string(),
            p.position.cost_basis.to_string(),
        ]);
    }

    let result = tabulate(rows);

    component
        .create_response(&ctx.http, utils::text_interaction_response(&result, true))
        .await?;

    Ok(())
}
