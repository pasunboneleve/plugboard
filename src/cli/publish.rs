use clap::Args;

use crate::domain::NewMessage;
use crate::error::Result;
use crate::exchange::Exchange;

#[derive(Debug, Args)]
pub struct PublishArgs {
    pub topic: String,
    pub body: String,
    #[arg(long)]
    pub parent_id: Option<String>,
    #[arg(long)]
    pub conversation_id: Option<String>,
    #[arg(long)]
    pub producer: Option<String>,
    #[arg(long)]
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
