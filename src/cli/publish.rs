use clap::Args;

use crate::domain::NewMessage;
use crate::error::Result;
use crate::exchange::Exchange;

#[derive(Debug, Args)]
#[command(
    about = "Publish a plain-text message to a topic",
    long_about = "Publish a plain-text message to a topic.\n\nTopics are the addressing mechanism in Plugboard. A worker or other participant can later read or claim messages from that topic.\n\nThe BODY argument is stored as plain text exactly as provided."
)]
pub struct PublishArgs {
    #[arg(help = "Topic name to publish to, such as review.request or gemini.review.request")]
    pub topic: String,
    #[arg(help = "Plain-text message body to store on the topic")]
    pub body: String,
    #[arg(long, help = "Optional parent message id for follow-up threading")]
    pub parent_id: Option<String>,
    #[arg(long, help = "Optional conversation id to group related messages")]
    pub conversation_id: Option<String>,
    #[arg(long, help = "Optional producer label to record with the message")]
    pub producer: Option<String>,
    #[arg(long, help = "Optional JSON string for shallow message metadata")]
    pub metadata_json: Option<String>,
}

pub fn execute(exchange: &impl Exchange, args: PublishArgs) -> Result<()> {
    let message = exchange.publish(NewMessage {
        topic: args.topic,
        body: args.body,
        parent_id: args.parent_id,
        conversation_id: args.conversation_id,
        producer: args.producer,
        metadata_json: args.metadata_json,
    })?;

    println!("{}", message.id);
    Ok(())
}
