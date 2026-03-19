use clap::Args;

use crate::error::Result;
use crate::exchange::Exchange;

#[derive(Debug, Args)]
#[command(
    about = "Read messages already published to the exchange",
    long_about = "Read messages that were already published to the exchange.\n\nUse `--topic` to read all messages for a topic or `--conversation-id` to read one correlated conversation. Output is tab-separated as: created_at, topic, body.\n\nMessages are listed in stored order so the output reflects the conversation or topic history."
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
}

pub fn execute(exchange: &impl Exchange, args: ReadArgs) -> Result<()> {
    let messages = if let Some(topic) = args.topic.as_deref() {
        exchange.read_by_topic(topic)?
    } else if let Some(conversation_id) = args.conversation.as_deref() {
        exchange.read_by_conversation(conversation_id)?
    } else {
        exchange.list_messages()?
    };

    for message in messages {
        println!(
            "{}\t{}\t{}",
            message.created_at, message.topic, message.body
        );
    }

    Ok(())
}
