use std::env;

use clap::{Parser, Subcommand};
use common::store;
use simplelog::{ColorChoice, Config, LevelFilter, TermLogger, TerminalMode};

use crate::Commands::ExportUsers;

mod blackjack;
mod bot;
mod command;
mod portfolio;
mod scripts;
mod trade;
mod ui;
mod utils;

#[derive(Subcommand)]
enum Commands {
    #[command(name = "export_users")]
    ExportUsers,
}

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    commands: Option<Commands>,
}

async fn run_script(f: impl AsyncFnOnce() -> anyhow::Result<()>) {
    let result = f().await;

    if let Err(e) = result {
        println!("script execution failed: {e}");
    }
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
        Some(ExportUsers) => run_script(scripts::export_users::run).await,
        None => bot::run().await,
    }
}
