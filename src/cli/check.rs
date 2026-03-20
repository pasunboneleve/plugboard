use clap::Args;
use serde_json::json;

use crate::cli::conversation_status::{ConversationState, find_terminal_reply, state_name};
use crate::cli::human_output::prefix_timestamp;
use crate::error::{PlugboardError, Result};
use crate::exchange::Exchange;

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
    let terminal = find_terminal_reply(&messages, &args.success_topic, &args.failure_topic);

    if let Some(terminal) = terminal {
        let message = terminal.message;
        let state = terminal.state;

        if args.json {
            println!(
                "{}",
                serde_json::to_string(&json!({
                    "state": state_name(state),
                    "conversation_id": args.conversation_id,
                    "message_id": message.id,
                    "topic": message.topic,
                    "body": message.body,
                }))?
            );
        } else {
            println!(
                "{}\n{}",
                prefix_timestamp(&format!(
                    "{} conversation_id={} message_id={} topic={}",
                    state_name(state),
                    args.conversation_id,
                    message.id,
                    message.topic,
                ))?,
                message.body
            );
        }

        if matches!(state, ConversationState::Failure) {
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
            println!(
                "{}",
                prefix_timestamp(&format!("pending conversation_id={}", args.conversation_id))?
            );
        }
        Ok(())
    }
}
