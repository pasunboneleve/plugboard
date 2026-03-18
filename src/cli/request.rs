use std::io::{self, Read};
use std::time::Duration;

use clap::Args;

use crate::domain::{Message, NewMessage};
use crate::error::{PlugboardError, Result};
use crate::exchange::Exchange;

const BLOCKING_RECHECK_INTERVAL: Duration = Duration::from_millis(250);

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

    let reply = await_reply(
        exchange,
        &request,
        &args.success_topic,
        &args.failure_topic,
        BLOCKING_RECHECK_INTERVAL,
    )?;
    println!("{}", reply.body);
    if reply.topic == args.success_topic {
        Ok(())
    } else {
        Err(PlugboardError::SilentExit { code: 1 })
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

fn await_reply(
    exchange: &impl Exchange,
    request: &Message,
    success_topic: &str,
    failure_topic: &str,
    wait_interval: Duration,
) -> Result<Message> {
    loop {
        let ticket = exchange.prepare_wait_for_change()?;
        if let Some(reply) = find_reply(exchange, request, success_topic, failure_topic)? {
            return Ok(reply);
        }

        match ticket {
            Some(ticket) => {
                ticket.wait(Some(wait_interval))?;
            }
            None => {
                std::thread::sleep(wait_interval);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::await_reply;
    use crate::domain::{Claim, Message, NewMessage};
    use crate::error::Result;
    use crate::exchange::Exchange;
    use crate::notifier::WaitTicket;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    #[derive(Clone)]
    struct FakeExchange {
        messages: Arc<Mutex<Vec<Message>>>,
    }

    impl FakeExchange {
        fn new(messages: Vec<Message>) -> Self {
            Self {
                messages: Arc::new(Mutex::new(messages)),
            }
        }

        fn push_message(&self, message: Message) {
            self.messages.lock().unwrap().push(message);
        }
    }

    struct NeverWakeTicket;

    impl WaitTicket for NeverWakeTicket {
        fn wait(self: Box<Self>, timeout: Option<Duration>) -> Result<bool> {
            if let Some(timeout) = timeout {
                thread::sleep(timeout);
            }
            Ok(false)
        }
    }

    impl Exchange for FakeExchange {
        fn init(&self) -> Result<()> {
            Ok(())
        }

        fn publish(&self, _message: NewMessage) -> Result<Message> {
            unreachable!("publish is not used in this test")
        }

        fn read_by_topic(&self, _topic: &str) -> Result<Vec<Message>> {
            unreachable!("read_by_topic is not used in this test")
        }

        fn read_by_conversation(&self, conversation_id: &str) -> Result<Vec<Message>> {
            Ok(self
                .messages
                .lock()
                .unwrap()
                .iter()
                .filter(|message| message.conversation_id == conversation_id)
                .cloned()
                .collect())
        }

        fn list_messages(&self) -> Result<Vec<Message>> {
            unreachable!("list_messages is not used in this test")
        }

        fn get_message(&self, _message_id: &str) -> Result<Option<Message>> {
            unreachable!("get_message is not used in this test")
        }

        fn claims_for_message(&self, _message_id: &str) -> Result<Vec<Claim>> {
            unreachable!("claims_for_message is not used in this test")
        }

        fn claim_next(
            &self,
            _topic: &str,
            _worker_group: &str,
            _worker_instance_id: &str,
            _lease_seconds: i64,
        ) -> Result<Option<(Message, Claim)>> {
            unreachable!("claim_next is not used in this test")
        }

        fn claim_next_blocking(
            &self,
            _topic: &str,
            _worker_group: &str,
            _worker_instance_id: &str,
            _lease_seconds: i64,
            _idle_sleep: Duration,
        ) -> Result<(Message, Claim)> {
            unreachable!("claim_next_blocking is not used in this test")
        }

        fn prepare_wait_for_change(&self) -> Result<Option<Box<dyn WaitTicket>>> {
            Ok(Some(Box::new(NeverWakeTicket)))
        }

        fn wait_for_change(&self, _timeout: Option<Duration>) -> Result<bool> {
            unreachable!("wait_for_change is not used in this test")
        }

        fn complete_claim(&self, _claim_id: &str) -> Result<Claim> {
            unreachable!("complete_claim is not used in this test")
        }

        fn fail_claim(&self, _claim_id: &str) -> Result<Claim> {
            unreachable!("fail_claim is not used in this test")
        }

        fn timeout_claim(&self, _claim_id: &str) -> Result<Claim> {
            unreachable!("timeout_claim is not used in this test")
        }
    }

    fn request_message() -> Message {
        Message {
            id: "message-1".into(),
            topic: "review.request".into(),
            body: "Review this".into(),
            created_at: "2026-03-18T00:00:00Z".into(),
            parent_id: None,
            conversation_id: "conversation-1".into(),
            producer: Some("requestor".into()),
            metadata_json: None,
        }
    }

    #[test]
    fn await_reply_recovers_when_notifier_never_fires() {
        let request = request_message();
        let exchange = FakeExchange::new(vec![request.clone()]);
        let writer = exchange.clone();

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            writer.push_message(Message {
                id: "message-2".into(),
                topic: "review.done".into(),
                body: "Looks good".into(),
                created_at: "2026-03-18T00:00:01Z".into(),
                parent_id: Some("message-1".into()),
                conversation_id: "conversation-1".into(),
                producer: Some("worker".into()),
                metadata_json: None,
            });
        });

        let reply = await_reply(
            &exchange,
            &request,
            "review.done",
            "review.failed",
            Duration::from_millis(10),
        )
        .unwrap();

        assert_eq!(reply.topic, "review.done");
        assert_eq!(reply.body, "Looks good");
    }
}
