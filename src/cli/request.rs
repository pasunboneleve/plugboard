use std::io::{self, Read};
use std::time::Duration;

use clap::Args;
use log::debug;
use crate::domain::{Message, NewMessage};
use crate::cli::message_identifiers::emit_publish_identifiers;
use crate::cli::message_metadata::{merge_meta_into_metadata_json, parse_meta_args};
use crate::error::{PlugboardError, Result};
use crate::exchange::Exchange;

pub const DEFAULT_REPLY_RECHECK_INTERVAL: Duration = Duration::from_millis(250);
pub const DEFAULT_REPLY_WAIT_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Debug, Args)]
#[command(
    about = "Publish a request and wait for one correlated reply",
    long_about = "Publish a plain-text request to a topic, then wait for the first correlated follow-up message in the same conversation on either the configured success or failure topic.\n\nThis is a thin request/reply helper at the edge. It uses the normal Plugboard message log, conversation_id propagation, and advisory wakeups. It does not add a new request entity or subscription model.\n\nOn publish, the command emits the request message_id and conversation_id to stderr in a stable format so agents can capture them and later check replies by conversation.\n\nNotifier wakeups are advisory only. Correctness currently relies on bounded notifier waits and, when no notifier is available, bounded periodic SQLite re-checks. Both default to 250 ms, so worst-case reply detection latency under notifier failure is about 250 ms plus normal process and SQLite overhead.\n\nEnable targeted wakeup-path logs with RUST_LOG=debug."
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
    #[arg(
        long,
        help = "Optional producer label to record with the request message"
    )]
    pub producer: Option<String>,
    #[arg(
        long = "meta",
        help = "Repeatable request metadata entry in KEY=VALUE form; stored under metadata_json.meta"
    )]
    pub meta: Vec<String>,
    #[arg(
        long,
        default_value_t = DEFAULT_REPLY_WAIT_TIMEOUT.as_millis() as u64,
        help = "Maximum advisory notifier wait in milliseconds before forcing a reply re-check; default 250 ms"
    )]
    pub wait_timeout_ms: u64,
    #[arg(
        long,
        default_value_t = DEFAULT_REPLY_RECHECK_INTERVAL.as_millis() as u64,
        help = "Periodic fallback re-check interval in milliseconds when no notifier is available while waiting for a reply; default 250 ms"
    )]
    pub recheck_ms: u64,
    #[arg(
        long,
        help = "Emit publish-time request identifiers as JSON on stderr instead of key=value text"
    )]
    pub json: bool,
}

pub fn execute(exchange: &impl Exchange, args: RequestArgs) -> Result<()> {
    let body = match args.body {
        Some(body) => body,
        None => read_body_from_stdin()?,
    };
    let meta = parse_meta_args(&args.meta)?;
    let metadata_json = merge_meta_into_metadata_json(None, &meta)?;

    let request = exchange.publish(NewMessage {
        topic: args.topic,
        body,
        parent_id: None,
        conversation_id: None,
        producer: args.producer,
        metadata_json,
    })?;
    debug!(
        "published request id={} conversation_id={} topic={}",
        request.id, request.conversation_id, request.topic
    );
    emit_publish_identifiers(&request, args.json)?;

    let reply = await_reply(
        exchange,
        &request,
        &args.success_topic,
        &args.failure_topic,
        Duration::from_millis(args.wait_timeout_ms),
        Duration::from_millis(args.recheck_ms),
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
        message.id != request.id
            && (message.topic == success_topic || message.topic == failure_topic)
    }))
}

fn await_reply(
    exchange: &impl Exchange,
    request: &Message,
    success_topic: &str,
    failure_topic: &str,
    wait_timeout: Duration,
    wait_interval: Duration,
) -> Result<Message> {
    loop {
        debug!(
            "request {} arming wait ticket for conversation={} success_topic={} failure_topic={} wait_timeout_ms={} recheck_ms={}",
            request.id,
            request.conversation_id,
            success_topic,
            failure_topic,
            wait_timeout.as_millis(),
            wait_interval.as_millis()
        );
        let ticket = exchange.prepare_wait_for_change()?;
        debug!(
            "request {} checking SQLite for correlated reply in conversation={}",
            request.id, request.conversation_id
        );
        if let Some(reply) = find_reply(exchange, request, success_topic, failure_topic)? {
            debug!(
                "request {} matched reply {} on topic={}",
                request.id, reply.id, reply.topic
            );
            return Ok(reply);
        }

        match ticket {
            Some(ticket) => {
                debug!(
                    "request {} waiting on notifier for up to {} ms",
                    request.id,
                    wait_timeout.as_millis()
                );
                let woke = ticket.wait(Some(wait_timeout))?;
                if woke {
                    debug!(
                        "request {} received notifier event; re-checking",
                        request.id
                    );
                } else {
                    debug!(
                        "request {} notifier wait timed out after {} ms; forcing immediate reply re-check",
                        request.id,
                        wait_timeout.as_millis(),
                    );
                }
            }
            None => {
                debug!(
                    "request {} has no notifier; entering fallback sleep {} ms",
                    request.id,
                    wait_interval.as_millis()
                );
                std::thread::sleep(wait_interval);
                debug!("request {} fallback wake complete; re-checking", request.id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::await_reply;
    use crate::cli::message_identifiers::emit_publish_identifiers;
    use crate::cli::message_metadata::{merge_meta_into_metadata_json, parse_meta_args};
    use crate::domain::{Claim, Message, NewMessage};
    use crate::error::Result;
    use crate::exchange::Exchange;
    use crate::notifier::WaitTicket;
    use serde_json::{Value, json};
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
            _wait_timeout: Duration,
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
            Duration::from_millis(10),
        )
        .unwrap();

        assert_eq!(reply.topic, "review.done");
        assert_eq!(reply.body, "Looks good");
    }

    #[test]
    fn parses_multiple_meta_args_with_json_values() {
        let parsed = parse_meta_args(&[
            "model=llama3.2:3b".into(),
            "temperature=0.7".into(),
            "debug=true".into(),
        ])
        .unwrap();

        assert_eq!(parsed[0].0, "model");
        assert_eq!(parsed[0].1, json!("llama3.2:3b"));
        assert_eq!(parsed[1].1, json!(0.7));
        assert_eq!(parsed[2].1, json!(true));
    }

    #[test]
    fn rejects_invalid_meta_args() {
        assert!(parse_meta_args(&["missing_equals".into()]).is_err());
        assert!(parse_meta_args(&["=value".into()]).is_err());
    }

    #[test]
    fn merges_meta_under_top_level_field_without_overwriting_other_fields() {
        let merged = merge_meta_into_metadata_json(
            Some(r#"{"exit_code":0,"stdout":"ok"}"#),
            &[
                ("model".into(), json!("llama3.2:3b")),
                ("temperature".into(), json!(0.7)),
            ],
        )
        .unwrap()
        .unwrap();

        let parsed: Value = serde_json::from_str(&merged).unwrap();
        assert_eq!(parsed["exit_code"], json!(0));
        assert_eq!(parsed["stdout"], json!("ok"));
        assert_eq!(parsed["meta"]["model"], json!("llama3.2:3b"));
        assert_eq!(parsed["meta"]["temperature"], json!(0.7));
    }

    #[test]
    fn emits_json_request_identifiers() {
        let request = request_message();
        emit_publish_identifiers(&request, true).unwrap();
    }
}
