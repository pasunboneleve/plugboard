use clap::Args;
use serde_json::json;

use crate::error::{PlugboardError, Result};
use crate::exchange::{Exchange, IncrementalExchange};

const DEFAULT_INCREMENTAL_READ_LIMIT: usize = 100;

#[derive(Debug, Args)]
#[command(
    about = "Read messages already published to the exchange",
    long_about = "Read messages that were already published to the exchange.\n\nUse `--topic` to read all messages for a topic or `--conversation-id` to read one correlated conversation. Output is tab-separated as: created_at, topic, body.\n\nFor bounded incremental consumption, combine a conversation with `--after-position` and `--limit`. The cursor is exclusive; zero starts at the beginning. Add `--json` for one JSON object per line.\n\nMessages are listed in durable insertion order so the output reflects the conversation or topic history."
)]
pub struct ReadArgs {
    #[arg(
        long,
        conflicts_with = "conversation",
        help = "Read all messages published to a topic"
    )]
    pub topic: Option<String>,
    #[arg(
        long = "conversation",
        alias = "conversation-id",
        conflicts_with = "topic",
        help = "Read all messages in one conversation thread by conversation id"
    )]
    pub conversation: Option<String>,
    #[arg(
        long,
        requires_all = ["conversation", "json"],
        help = "Read conversation messages after this exclusive database-local position"
    )]
    pub after_position: Option<i64>,
    #[arg(
        long,
        requires_all = ["conversation", "json"],
        help = "Bound an incremental conversation read to this many messages (maximum 1000)"
    )]
    pub limit: Option<usize>,
    #[arg(
        long,
        requires = "conversation",
        help = "Emit bounded conversation results as newline-delimited JSON"
    )]
    pub json: bool,
}

pub fn execute(exchange: &(impl Exchange + IncrementalExchange), args: ReadArgs) -> Result<()> {
    if args.json {
        let conversation_id = args
            .conversation
            .as_deref()
            .ok_or(PlugboardError::IncrementalReadRequiresConversation)?;
        let messages = exchange.read_conversation_after(
            conversation_id,
            args.after_position.unwrap_or(0),
            args.limit.unwrap_or(DEFAULT_INCREMENTAL_READ_LIMIT),
        )?;

        for positioned in messages {
            print_json(&positioned)?;
        }
        return Ok(());
    }

    let messages = if let Some(topic) = args.topic.as_deref() {
        exchange.read_by_topic(topic)?
    } else if let Some(conversation_id) = args.conversation.as_deref() {
        exchange.read_by_conversation(conversation_id)?
    } else {
        exchange.list_messages()?
    };

    for message in messages {
        print_human(&message);
    }

    Ok(())
}

fn print_human(message: &crate::domain::Message) {
    println!(
        "{}\t{}\t{}",
        message.created_at, message.topic, message.body
    );
}

fn print_json(positioned: &crate::domain::PositionedMessage) -> Result<()> {
    let message = &positioned.message;
    println!(
        "{}",
        serde_json::to_string(&json!({
            "position": positioned.position,
            "id": message.id,
            "topic": message.topic,
            "body": message.body,
            "created_at": message.created_at,
            "parent_id": message.parent_id,
            "conversation_id": message.conversation_id,
            "producer": message.producer,
            "metadata_json": message.metadata_json,
        }))?
    );
    Ok(())
}
