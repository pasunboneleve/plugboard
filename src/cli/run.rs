use std::time::Duration;

use clap::Args;

use crate::error::Result;
use crate::exchange::Exchange;
use crate::plugin::command::CommandPlugin;
use crate::worker::{DEFAULT_IDLE_SLEEP, DEFAULT_WAIT_TIMEOUT, WorkerConfig, WorkerHost};

#[derive(Debug, Args)]
#[command(
    about = "Host a worker host that listens on a topic",
    long_about = "Host a worker host on a topic.\n\nIn persistent mode, the worker waits for matching messages repeatedly, claims one message at a time, writes the claimed message body to the configured backend on stdin, captures stdout as success output, captures failure output from stderr or a non-zero exit, and publishes the follow-up message to the configured success, failure, or timeout topic. Wakeups are advisory only: after any wake or handled message, the worker drains all currently claimable work before waiting again.\n\nWith --once, the worker blocks until one matching message exists, handles exactly one message, publishes the follow-up, and exits immediately. This is the reactive path for passive tools.\n\nThe backend command is executed once per message. It must read input from stdin, write output to stdout, and exit. Plugboard does not maintain persistent sessions.\n\nClaims record both a stable worker group and a per-process worker instance id. A claim is live only while status is active and lease_until is still in the future. Expired active claims are recovered in the claim path itself.\n\nNotifier wakeups are advisory only. Correctness currently relies on bounded notifier waits and, when no notifier is available, bounded periodic SQLite re-checks. Both default to 250 ms, so worst-case detection latency under notifier failure is about 250 ms plus normal process and SQLite overhead.\n\nEnable targeted wakeup-path logs with RUST_LOG=debug.\n\nEach claimed message is also subject to the per-message timeout configured by --timeout-seconds. The default is 60 seconds, which is often too short for real LLM calls. Raise it for slower backends such as Gemini.\n\nThis command is intended for passive backends that fit that contract. Interactive tools usually need a wrapper or dedicated plugin before they fit this worker model."
)]
pub struct RunArgs {
    #[arg(long, help = "Topic to poll for work")]
    pub topic: String,
    #[arg(long, help = "Topic to publish when plugin execution succeeds")]
    pub success_topic: String,
    #[arg(long, help = "Topic to publish when plugin execution fails")]
    pub failure_topic: String,
    #[arg(
        long,
        default_value_t = 60,
        help = "Per-message timeout in seconds; default 60, increase for slower backends such as Gemini"
    )]
    pub timeout_seconds: u64,
    #[arg(long, help = "Optional stable worker group name recorded in claims")]
    pub runner_name: Option<String>,
    #[arg(
        long,
        help = "Claim lease in seconds; default is timeout + 30 seconds, or 300 seconds when no worker timeout is configured"
    )]
    pub lease_seconds: Option<u64>,
    #[arg(
        long,
        default_value_t = DEFAULT_WAIT_TIMEOUT.as_millis() as u64,
        help = "Maximum advisory notifier wait in milliseconds before forcing a SQLite re-check; default 250 ms"
    )]
    pub wait_timeout_ms: u64,
    #[arg(
        long,
        default_value_t = DEFAULT_IDLE_SLEEP.as_millis() as u64,
        help = "Periodic fallback re-check interval in milliseconds when no notifier is available; default 250 ms"
    )]
    pub idle_sleep_ms: u64,
    #[arg(
        long,
        help = "Reactive mode: block until one matching message exists, handle it once, then exit"
    )]
    pub once: bool,
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
        config.worker_group = runner_name;
    }
    if let Some(lease_seconds) = args.lease_seconds {
        config.lease_seconds = lease_seconds;
    }
    config.wait_timeout = Duration::from_millis(args.wait_timeout_ms);
    config.idle_sleep = Duration::from_millis(args.idle_sleep_ms);

    let plugin = CommandPlugin::new(args.command)?;
    let host = WorkerHost::new(exchange, &plugin, config);
    if args.once {
        host.run_once_blocking().map(|_| ())
    } else {
        host.run_forever()
    }
}
