use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;

use clap::Args;

use crate::cli::conversation_status::{ConversationState, find_terminal_reply};
use crate::cli::tracking::{load_tracked_conversations, mark_notified};
use crate::error::Result;
use crate::exchange::Exchange;

#[derive(Debug, Args)]
#[command(
    about = "Run a local completion notifier for tracked conversations",
    long_about = "Run a local completion notifier for tracked conversations.\n\nThis command reads local tracked-conversation state, checks each non-notified conversation for a terminal success or failure reply, emits one advisory local notification, and marks that conversation as notified in local state.\n\nIt does not change exchange correctness or delivery guarantees."
)]
pub struct NotifyArgs {
    #[arg(long, help = "Run a single notification scan and exit")]
    pub once: bool,
    #[arg(long, default_value = "2", help = "Polling interval in seconds")]
    pub poll_seconds: u64,
}

pub fn execute(exchange: &impl Exchange, args: NotifyArgs, state_path: &Path) -> Result<()> {
    loop {
        scan_once(exchange, state_path)?;
        if args.once {
            return Ok(());
        }
        thread::sleep(Duration::from_secs(args.poll_seconds));
    }
}

fn scan_once(exchange: &impl Exchange, state_path: &Path) -> Result<()> {
    for tracked in load_tracked_conversations(state_path)?
        .into_iter()
        .filter(|tracked| !tracked.notified)
    {
        let messages = exchange.read_by_conversation(&tracked.conversation_id)?;
        if let Some(terminal) =
            find_terminal_reply(&messages, &tracked.success_topic, &tracked.failure_topic)
        {
            let label = label_for_topics(&tracked.success_topic);
            emit_notification(
                &label,
                &tracked.conversation_id,
                terminal.state,
                &terminal.message.body,
                &tracked.success_topic,
                &tracked.failure_topic,
            )?;
            mark_notified(state_path, &tracked.conversation_id)?;
        }
    }

    Ok(())
}

fn emit_notification(
    label: &str,
    conversation_id: &str,
    state: ConversationState,
    body: &str,
    success_topic: &str,
    failure_topic: &str,
) -> Result<()> {
    let title = match state {
        ConversationState::Success => label.to_string(),
        ConversationState::Failure => format!("{label} (failed)"),
    };
    let detail = format!(
        "conversation_id={conversation_id}\n{}\nRun: ./target/debug/plugboard check --conversation-id {conversation_id} --success-topic {success_topic} --failure-topic {failure_topic}",
        preview_body(body)
    );

    match std::env::var("PLUGBOARD_NOTIFY_BACKEND").ok().as_deref() {
        Some("stderr") => {
            eprintln!("{title}\n{detail}");
            return Ok(());
        }
        Some("bell") => {
            eprint!("\x07");
            eprintln!("{title}\n{detail}");
            return Ok(());
        }
        _ => {}
    }

    if notify_send(&title, &detail) {
        return Ok(());
    }

    eprint!("\x07");
    eprintln!("{title}\n{detail}");
    Ok(())
}

fn notify_send(title: &str, detail: &str) -> bool {
    Command::new("sh")
        .args(["-lc", "command -v notify-send >/dev/null 2>&1"])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
        && Command::new("notify-send")
            .arg(title)
            .arg(detail)
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
}

fn label_for_topics(success_topic: &str) -> String {
    if success_topic == "ollama.done" {
        "Ollama reply ready".into()
    } else {
        "Reply ready".into()
    }
}

fn preview_body(body: &str) -> String {
    let mut compact = body.lines().next().unwrap_or("").trim().to_string();
    if compact.len() > 80 {
        compact.truncate(77);
        compact.push_str("...");
    }
    if compact.is_empty() {
        "(empty reply preview)".into()
    } else {
        compact
    }
}
