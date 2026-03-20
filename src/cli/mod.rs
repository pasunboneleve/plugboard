pub mod conversation_status;
pub mod check;
pub mod human_output;
pub mod inspect;
pub mod message_identifiers;
pub mod message_metadata;
pub mod notify;
pub mod publish;
pub mod read;
pub mod request;
pub mod run;
pub mod tracking;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::error::Result;
use crate::exchange::Exchange;
use crate::exchange::sqlite::SqliteExchange;

#[derive(Debug, Parser)]
#[command(
    name = "plugboard",
    about = "A local textual exchange built around topic-addressed messages",
    long_about = "Plugboard is a local textual exchange built around topics.\n\nUse `publish` to send plain text to a topic, `read` to inspect messages already published to a topic or conversation, `check` to ask whether one conversation has a terminal reply yet, `run` to host a worker that listens on a topic and forwards each claimed message to a passive backend, and `request` for the common publish-and-wait request/reply flow."
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
    #[command(about = "Check one conversation for a terminal success or failure reply")]
    Check(check::CheckArgs),
    #[command(about = "Run a local completion notifier for tracked conversations")]
    Notify(notify::NotifyArgs),
    #[command(about = "Publish a request and wait for one correlated reply")]
    Request(request::RequestArgs),
    #[command(about = "Inspect raw message and claim history for debugging and forensics")]
    Inspect(inspect::InspectArgs),
    #[command(about = "Host a long-running worker that listens on a topic")]
    Run(run::RunArgs),
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let exchange = SqliteExchange::open(&cli.database)?;
    exchange.init()?;
    let tracking_state_path = tracking::tracking_state_path(&cli.database);

    match cli.command {
        Commands::Publish(args) => publish::execute(&exchange, args, &tracking_state_path),
        Commands::Read(args) => read::execute(&exchange, args),
        Commands::Check(args) => check::execute(&exchange, args),
        Commands::Notify(args) => notify::execute(&exchange, args, &tracking_state_path),
        Commands::Request(args) => request::execute(&exchange, args),
        Commands::Inspect(args) => inspect::execute(&exchange, args),
        Commands::Run(args) => run::execute(&exchange, args),
    }
}
