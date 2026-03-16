use clap::Args;

use crate::error::Result;
use crate::exchange::Exchange;

#[derive(Debug, Args)]
pub struct ReadArgs {
    #[arg(long, conflicts_with = "conversation")]
    pub topic: Option<String>,
    #[arg(long, conflicts_with = "topic")]
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
