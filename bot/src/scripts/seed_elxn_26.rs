use std::env;

use serenity::all::{CreateMessage, GenericChannelId, MessageFlags, Token};
use serenity::http::Http;

use crate::ui::{
    create_market_thread,
    market_message::{render_details_message, render_market_message},
};
use common::store::{
    CatshiDb,
    instrument::{DbInstrumentStore, InstrumentStore},
    market::{DbMarketStore, MarketStore},
    user::{DbUserStore, UserStore},
};

struct MarketSeed {
    tag: &'static str,
    description: &'static str,
    instruments: &'static [&'static str],
}

const MARKETS: &[MarketSeed] = &[
    MarketSeed {
        tag: "elxn26-senate-margin",
        description: "2026 Senate topline: seat differential",
        instruments: &["R+2+", "Even", "D+2", "D+4", "D+6", "D+8", "D+10", "D+12+"],
    },
    MarketSeed {
        tag: "elxn26-senate-nc",
        description: "2026 Senate: North Carolina winner",
        instruments: &["Roy Cooper (D)", "Michael Whatley (R)"],
    },
    MarketSeed {
        tag: "elxn26-senate-me",
        description: "2026 Senate: Maine winner",
        instruments: &["Troy Jackson (D)", "Susan Collins (R)"],
    },
    MarketSeed {
        tag: "elxn26-senate-mi",
        description: "2026 Senate: Michigan winner",
        instruments: &["Abdul El-Sayed (D)", "Mike Rogers (R)"],
    },
    MarketSeed {
        tag: "elxn26-senate-oh",
        description: "2026 Senate: Ohio winner",
        instruments: &["Sherrod Brown (D)", "Jon Husted (R)"],
    },
    MarketSeed {
        tag: "elxn26-senate-ia",
        description: "2026 Senate: Iowa winner",
        instruments: &["Josh Turek (D)", "Ashley Hinson (R)"],
    },
    MarketSeed {
        tag: "elxn26-senate-tx",
        description: "2026 Senate: Texas winner",
        instruments: &["James Talarico (D)", "Ken Paxton (R)"],
    },
    MarketSeed {
        tag: "elxn26-senate-ak",
        description: "2026 Senate: Alaska winner",
        instruments: &["Mary Peltola (D)", "Dan Sullivan (R)"],
    },
    MarketSeed {
        tag: "elxn26-senate-sc",
        description: "2026 Senate: South Carolina winner",
        instruments: &["Annie Andrews (D)", "Lindsey Graham (R)"],
    },
    MarketSeed {
        tag: "elxn26-senate-ne",
        description: "2026 Senate: Nebraska winner",
        instruments: &["Dan Osborn (I)", "Pete Ricketts (R)"],
    },
    MarketSeed {
        tag: "elxn26-senate-ks",
        description: "2026 Senate: Kansas winner",
        instruments: &["Adam Hamilton (D)", "Roger Marshall (R)"],
    },
    MarketSeed {
        tag: "elxn26-senate-ga",
        description: "2026 Senate: Georgia winner",
        instruments: &["Jon Ossoff (D)", "Mike Collins (R)"],
    },
    MarketSeed {
        tag: "elxn26-senate-nh",
        description: "2026 Senate: New Hampshire winner",
        instruments: &["Chris Pappas (D)", "John E. Sununu (R)"],
    },
    MarketSeed {
        tag: "elxn26-house-nj7",
        description: "2026 House: New Jersey's 7th District winner",
        instruments: &["Rebecca Bennett (D)", "Tom Kean Jr. (R)"],
    },
];

pub async fn run(owner_name: &str, channel_id: u64) -> anyhow::Result<()> {
    let pg_url = env::var("POSTGRES_URL")?;
    let http = Http::new(Token::from_env("DISCORD_TOKEN")?);
    let channel = GenericChannelId::new(channel_id);
    let db = CatshiDb::new(&pg_url).await?;

    let market_store = DbMarketStore {};
    let instrument_store = DbInstrumentStore {};
    let user_store = DbUserStore {};

    let mut owners = user_store
        .get_by_name(&mut db.conn().await?, owner_name)
        .await?;
    let owner = match owners.len() {
        1 => owners.remove(0),
        0 => panic!("no user named {owner_name:?}"),
        n => panic!("{n} users named {owner_name:?}; name must be unique"),
    };

    let mut created = 0;
    let mut skipped = 0;

    for seed in MARKETS {
        let mut tx = db.begin().await?;

        let market = market_store
            .create_market_if_tag_absent(&mut tx, seed.tag, seed.description, &owner)
            .await?;

        match market {
            Some(market) => {
                let instruments = instrument_store
                    .insert_market_instruments(&mut tx, &market, seed.instruments)
                    .await?;
                let instruments_with_shares: Vec<_> =
                    instruments.into_iter().map(|i| (i, 0)).collect();

                let components =
                    render_market_message(&market, &owner, instruments_with_shares.iter());
                let message = channel
                    .send_message(
                        &http,
                        CreateMessage::new()
                            .components(&components)
                            .flags(MessageFlags::IS_COMPONENTS_V2),
                    )
                    .await?;

                let thread =
                    create_market_thread(&market, channel.expect_channel(), message.id, &http)
                        .await?;
                let details_message = thread
                    .send_message(
                        &http,
                        CreateMessage::new().content(render_details_message(
                            &instruments_with_shares,
                            &Vec::new(),
                        )),
                    )
                    .await?;

                market_store
                    .set_market_message_id(
                        &mut tx,
                        market.id,
                        message.id,
                        channel,
                        thread.id,
                        details_message.id,
                    )
                    .await?;

                tx.commit().await?;
                log::info!("created market {} ({})", seed.tag, market.id);
                created += 1;
            }
            None => {
                log::info!("skipped market {}: already exists", seed.tag);
                skipped += 1;
            }
        }
    }

    log::info!("done: {created} created, {skipped} skipped");

    Ok(())
}
