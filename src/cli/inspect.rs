use clap::Args;

use crate::error::Result;
use crate::exchange::Exchange;

#[derive(Debug, Args)]
pub struct InspectArgs {
    #[arg(long, conflicts_with = "conversation")]
    pub message: Option<String>,
    #[arg(long, conflicts_with = "message")]
    pub conversation: Option<String>,
}

pub fn execute(exchange: &impl Exchange, args: InspectArgs) -> Result<()> {
    if let Some(message_id) = args.message.as_deref() {
        if let Some(message) = exchange.get_message(message_id)? {
            print_message(&message);
            let claims = exchange.claims_for_message(message_id)?;
            for claim in claims {
                println!(
                    "claim {} worker_group={} worker_instance_id={} status={} claimed_at={} lease_until={} completed_at={}",
                    claim.id,
                    claim.worker_group,
                    claim.worker_instance_id,
                    claim.status,
                    claim.claimed_at,
                    claim.lease_until,
                    claim.completed_at.unwrap_or_else(|| "-".into()),
                );
            }
        }
        return Ok(());
    }

    let messages = if let Some(conversation_id) = args.conversation.as_deref() {
        exchange.read_by_conversation(conversation_id)?
    } else {
        exchange.list_messages()?
    };

    for message in messages {
        print_message(&message);
    }

    Ok(())
}

fn print_message(message: &crate::domain::Message) {
    println!(
        "message {} topic={} conversation={} parent={} producer={}",
        message.id,
        message.topic,
        message.conversation_id,
        message.parent_id.as_deref().unwrap_or("-"),
        message.producer.as_deref().unwrap_or("-"),
    );
    println!("created_at={}", message.created_at);
    println!("body={}", message.body);
    println!(
        "metadata_json={}",
        message.metadata_json.as_deref().unwrap_or("-"),
    );
}
