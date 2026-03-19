use clap::Args;
use serde_json::json;

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
    let terminal = messages
        .iter()
        .rev()
        .find(|message| message.topic == args.success_topic || message.topic == args.failure_topic);

    match terminal {
        Some(message) if message.topic == args.success_topic => {
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string(&json!({
                        "state": "success",
                        "conversation_id": args.conversation_id,
                        "message_id": message.id,
                        "topic": message.topic,
                        "body": message.body,
                    }))?
                );
            } else {
                println!(
                    "success conversation_id={} message_id={} topic={}\n{}",
                    args.conversation_id, message.id, message.topic, message.body
                );
            }
            Ok(())
        }
        Some(message) => {
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string(&json!({
                        "state": "failure",
                        "conversation_id": args.conversation_id,
                        "message_id": message.id,
                        "topic": message.topic,
                        "body": message.body,
                    }))?
                );
            } else {
                println!(
                    "failure conversation_id={} message_id={} topic={}\n{}",
                    args.conversation_id, message.id, message.topic, message.body
                );
            }
            Err(PlugboardError::SilentExit { code: 1 })
        }
        None => {
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
}
