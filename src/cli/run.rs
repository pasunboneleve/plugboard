use clap::Args;

use crate::error::Result;
use crate::exchange::Exchange;
use crate::runner::{CommandRunner, RunnerConfig};

#[derive(Debug, Args)]
pub struct RunArgs {
    #[arg(long)]
    pub topic: String,
    #[arg(long)]
    pub success_topic: String,
    #[arg(long)]
    pub failure_topic: String,
    #[arg(long, default_value_t = 60)]
    pub timeout_seconds: u64,
    #[arg(long)]
    pub runner_name: Option<String>,
    #[arg(last = true, required = true)]
    pub command: Vec<String>,
}

pub fn execute(exchange: &impl Exchange, args: RunArgs) -> Result<()> {
    let mut config = RunnerConfig::new(
        args.topic,
        args.success_topic,
        args.failure_topic,
        args.timeout_seconds,
        args.command,
    );
    if let Some(runner_name) = args.runner_name {
        config.runner_name = runner_name;
    }

    CommandRunner::new(exchange, config).run_forever()
}
