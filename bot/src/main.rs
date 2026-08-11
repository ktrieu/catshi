use std::env;

use common::store;
use simplelog::{ColorChoice, Config, LevelFilter, TermLogger, TerminalMode};

mod blackjack;
mod bot;
mod command;
mod portfolio;
mod trade;
mod ui;
mod utils;

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

    // No subcommand runs the bot; future subcommands can dispatch to other scripts here.
    match env::args().nth(1).as_deref() {
        None => bot::run().await,
        Some(other) => panic!("Unrecognized command: {other}"),
    }
}
