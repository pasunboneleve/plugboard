use clap::Args;
use serde_json::json;

use crate::error::{PlugboardError, Result};
use crate::exchange::Exchange;

enum CheckState {
    Success,
    Failure,
}

#[derive(Debug, Args)]
#[command(
    about = "Check one conversation for a terminal success or failure reply",
    long_about = "Check one conversation for a terminal reply on either the configured success or failure topic.\n\nThis is a thin convenience helper over conversation-based reads. It does not add unread state, subscriptions, or schema. It exists to make async agent follow-up less repetitive: capture conversation_id at request time, then later ask whether that conversation has a terminal reply yet."
)]
pub struct CheckArgs {
    #[arg(long, help = "Conversation id to inspect")]
    pub conversation_id: String,
    #[arg(long, help = "Topic to treat as a successful terminal reply")]
    pub success_topic: String,
    #[arg(long, help = "Topic to treat as a failed terminal reply")]
    pub failure_topic: String,
    #[arg(long, help = "Emit machine-readable JSON")]
    pub json: bool,
}

pub fn execute(exchange: &impl Exchange, args: CheckArgs) -> Result<()> {
    let messages = exchange.read_by_conversation(&args.conversation_id)?;
    let terminal = messages
        .iter()
        .rev()
        .find(|message| message.topic == args.success_topic || message.topic == args.failure_topic);

    if let Some(message) = terminal {
        let state = if message.topic == args.success_topic {
            CheckState::Success
        } else {
            CheckState::Failure
        };

        if args.json {
            println!(
                "{}",
                serde_json::to_string(&json!({
                    "state": state_name(&state),
                    "conversation_id": args.conversation_id,
                    "message_id": message.id,
                    "topic": message.topic,
                    "body": message.body,
                }))?
            );
        } else {
            println!(
                "{} conversation_id={} message_id={} topic={}\n{}",
                state_name(&state),
                args.conversation_id,
                message.id,
                message.topic,
                message.body
            );
        }

        if matches!(state, CheckState::Failure) {
            Err(PlugboardError::SilentExit { code: 1 })
        } else {
            Ok(())
        }
    } else {
        if args.json {
            println!(
                "{}",
                serde_json::to_string(&json!({
                    "state": "pending",
                    "conversation_id": args.conversation_id,
                }))?
            );
        } else {
            println!("pending conversation_id={}", args.conversation_id);
        }
        Ok(())
    }
}

fn state_name(state: &CheckState) -> &'static str {
    match state {
        CheckState::Success => "success",
        CheckState::Failure => "failure",
    }
}
