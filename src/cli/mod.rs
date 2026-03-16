pub mod inspect;
pub mod publish;
pub mod read;
pub mod run;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::error::Result;
use crate::exchange::Exchange;
use crate::exchange::sqlite::SqliteExchange;

#[derive(Debug, Parser)]
#[command(name = "plugboard", about = "A local textual message exchange")]
pub struct Cli {
    #[arg(long, global = true, default_value = ".plugboard/plugboard.db")]
    pub database: PathBuf,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Publish(publish::PublishArgs),
    Read(read::ReadArgs),
    Inspect(inspect::InspectArgs),
    Run(run::RunArgs),
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let exchange = SqliteExchange::open(&cli.database)?;
    exchange.init()?;

    match cli.command {
        Commands::Publish(args) => publish::execute(&exchange, args),
        Commands::Read(args) => read::execute(&exchange, args),
        Commands::Inspect(args) => inspect::execute(&exchange, args),
        Commands::Run(args) => run::execute(&exchange, args),
    }
}
