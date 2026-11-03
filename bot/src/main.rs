use clap::{Parser, Subcommand};
use common::store;
use simplelog::{ColorChoice, Config, LevelFilter, TermLogger, TerminalMode};

use crate::Commands::{ExportUsers, SeedElxn26};

mod blackjack;
mod bot;
mod command;
use common::portfolio;
mod scripts;
use common::trade;
mod ui;
mod utils;

#[derive(Subcommand)]
enum Commands {
    #[command(name = "export_users")]
    ExportUsers,
    #[command(name = "seed_elxn_26")]
    SeedElxn26 {
        #[arg(long)]
        owner: String,
        #[arg(long)]
        channel_id: u64,
    },
}

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    commands: Option<Commands>,
}

#[tokio::main]
async fn main() {
    TermLogger::init(
        LevelFilter::Info,
        Config::default(),
        TerminalMode::Stdout,
        ColorChoice::Auto,
    )
    .expect("logger initialization should succeed");

    // Optional: absent when env vars are supplied another way (e.g. docker
    // compose's `environment:` block instead of a mounted .env file).
    dotenvy::dotenv().ok();

    let args = Args::parse();

    match &args.commands {
        Some(ExportUsers) => scripts::export_users::run(),
        Some(SeedElxn26 { owner, channel_id }) => scripts::seed_elxn_26::run(owner, *channel_id)
            .await
            .expect("seed_elxn_26 should succeed"),
        None => bot::run().await,
    }
}
