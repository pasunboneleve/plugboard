pub mod inspect;
pub mod publish;
pub mod read;
pub mod request;
pub mod run;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::error::Result;
use crate::exchange::Exchange;
use crate::exchange::sqlite::SqliteExchange;

#[derive(Debug, Parser)]
#[command(
    name = "plugboard",
    about = "A local textual exchange built around topic-addressed messages",
    long_about = "Plugboard is a local textual exchange built around topics.\n\nUse `publish` to send plain text to a topic, `read` to inspect messages already published to a topic or conversation, `run` to host a worker that listens on a topic and forwards each claimed message to a passive backend, and `request` for the common publish-and-wait request/reply flow."
)]
pub struct Cli {
    #[arg(
        long,
        global = true,
        default_value = ".plugboard/plugboard.db",
        help = "SQLite database path for the local message exchange"
    )]
    pub database: PathBuf,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(about = "Publish a plain-text message to a topic")]
    Publish(publish::PublishArgs),
    #[command(about = "Read messages that were already published to a topic or conversation")]
    Read(read::ReadArgs),
    #[command(about = "Publish a request and wait for one correlated reply")]
    Request(request::RequestArgs),
    Inspect(inspect::InspectArgs),
    #[command(about = "Host a long-running worker that listens on a topic")]
    Run(run::RunArgs),
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let exchange = SqliteExchange::open(&cli.database)?;
    exchange.init()?;

    match cli.command {
        Commands::Publish(args) => publish::execute(&exchange, args),
        Commands::Read(args) => read::execute(&exchange, args),
        Commands::Request(args) => request::execute(&exchange, args),
        Commands::Inspect(args) => inspect::execute(&exchange, args),
        Commands::Run(args) => run::execute(&exchange, args),
    }
}
