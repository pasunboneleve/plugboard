use clap::Args;

use crate::error::Result;
use crate::exchange::Exchange;
use crate::plugin::command::CommandPlugin;
use crate::worker::{WorkerConfig, WorkerHost};

#[derive(Debug, Args)]
#[command(
    about = "Host a long-running worker that listens on a topic",
    long_about = "Host a long-running worker on a topic.\n\nThe worker polls the configured topic, claims one message at a time, writes the claimed message body to the configured backend on stdin, captures stdout as success output, captures failure output from stderr or a non-zero exit, and publishes the follow-up message to the configured success or failure topic.\n\nThis command is intended for passive backends that read stdin, write stdout, and exit. Interactive tools usually need a wrapper or dedicated plugin before they fit this worker contract."
)]
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
        help = "Backend command to execute after --; it should read stdin, write stdout, and exit"
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
