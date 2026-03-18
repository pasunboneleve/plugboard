use std::io::{self, Read};

use clap::Args;

use crate::domain::{Message, NewMessage};
use crate::error::{PlugboardError, Result};
use crate::exchange::Exchange;

#[derive(Debug, Args)]
#[command(
    about = "Publish a request and wait for one correlated reply",
    long_about = "Publish a plain-text request to a topic, then wait for the first correlated follow-up message in the same conversation on either the configured success or failure topic.\n\nThis is a thin request/reply helper at the edge. It uses the normal Plugboard message log, conversation_id propagation, and advisory wakeups. It does not add a new request entity or subscription model."
)]
pub struct RequestArgs {
    #[arg(help = "Topic name to publish the request to")]
    pub topic: String,
    #[arg(long, help = "Topic to treat as a successful reply")]
    pub success_topic: String,
    #[arg(long, help = "Topic to treat as a failure reply")]
    pub failure_topic: String,
    #[arg(long, help = "Plain-text request body; if omitted, read from stdin")]
    pub body: Option<String>,
    #[arg(long, help = "Optional producer label to record with the request message")]
    pub producer: Option<String>,
}

pub fn execute(exchange: &impl Exchange, args: RequestArgs) -> Result<()> {
    let body = match args.body {
        Some(body) => body,
        None => read_body_from_stdin()?,
    };

    let request = exchange.publish(NewMessage {
        topic: args.topic,
        body,
        parent_id: None,
        conversation_id: None,
        producer: args.producer,
        metadata_json: None,
    })?;

    loop {
        let ticket = exchange.prepare_wait_for_change()?;
        if let Some(reply) = find_reply(
            exchange,
            &request,
            &args.success_topic,
            &args.failure_topic,
        )? {
            println!("{}", reply.body);
            if reply.topic == args.success_topic {
                return Ok(());
            }
            return Err(PlugboardError::SilentExit { code: 1 });
        }

        match ticket {
            Some(ticket) => {
                ticket.wait(None)?;
            }
            None => {
                return Err(PlugboardError::Io(io::Error::other(
                    "blocking request waits require a file-backed exchange with notifier support",
                )));
            }
        }
    }
}

fn read_body_from_stdin() -> Result<String> {
    let mut body = String::new();
    io::stdin().read_to_string(&mut body)?;
    Ok(body)
}

fn find_reply(
    exchange: &impl Exchange,
    request: &Message,
    success_topic: &str,
    failure_topic: &str,
) -> Result<Option<Message>> {
    let conversation = exchange.read_by_conversation(&request.conversation_id)?;
    Ok(conversation.into_iter().find(|message| {
        message.id != request.id && (message.topic == success_topic || message.topic == failure_topic)
    }))
}
