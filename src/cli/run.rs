use clap::Args;

use crate::error::Result;
use crate::exchange::Exchange;
use crate::plugin::command::CommandPlugin;
use crate::worker::{WorkerConfig, WorkerHost};

#[derive(Debug, Args)]
#[command(about = "Run a long-lived worker host that claims messages and executes a plugin")]
pub struct RunArgs {
    #[arg(long, help = "Topic to poll for work")]
    pub topic: String,
    #[arg(long, help = "Topic to publish when plugin execution succeeds")]
    pub success_topic: String,
    #[arg(long, help = "Topic to publish when plugin execution fails")]
    pub failure_topic: String,
    #[arg(long, default_value_t = 60, help = "Per-message timeout in seconds")]
    pub timeout_seconds: u64,
    #[arg(long, help = "Optional worker host name used in claim records")]
    pub runner_name: Option<String>,
    #[arg(
        last = true,
        required = true,
        help = "Command plugin to execute after --"
    )]
    pub command: Vec<String>,
}

pub fn execute(exchange: &impl Exchange, args: RunArgs) -> Result<()> {
    let mut config = WorkerConfig::new(
        args.topic,
        args.success_topic,
        args.failure_topic,
        args.timeout_seconds,
    );
    if let Some(runner_name) = args.runner_name {
        config.worker_name = runner_name;
    }

    let plugin = CommandPlugin::new(args.command)?;
    WorkerHost::new(exchange, &plugin, config).run_forever()
}
