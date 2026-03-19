use plugboard::domain::{ClaimStatus, NewMessage};
use plugboard::exchange::Exchange;
use plugboard::exchange::sqlite::SqliteExchange;
use plugboard::plugin::command::CommandPlugin;
use plugboard::worker::{RunOnceOutcome, WorkerConfig, WorkerHost};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::sync::mpsc;
use std::thread;

fn spawn_fake_ollama(status_line: &str, response_body: &str) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let status_line = status_line.to_string();
    let response_body = response_body.to_string();
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = Vec::new();
        let mut chunk = [0u8; 1024];
        let mut header_end = None;

        while header_end.is_none() {
            let read = stream.read(&mut chunk).unwrap();
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..read]);
            header_end = buffer.windows(4).position(|window| window == b"\r\n\r\n");
        }

        let header_end = header_end.expect("missing header terminator");
        let header_bytes = &buffer[..header_end + 4];
        let headers = String::from_utf8_lossy(header_bytes);
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.eq_ignore_ascii_case("content-length") {
                    Some(value.trim().parse::<usize>().unwrap())
                } else {
                    None
                }
            })
            .unwrap_or(0);

        let mut body_bytes = buffer[header_end + 4..].to_vec();
        while body_bytes.len() < content_length {
            let read = stream.read(&mut chunk).unwrap();
            if read == 0 {
                break;
            }
            body_bytes.extend_from_slice(&chunk[..read]);
        }

        tx.send(String::from_utf8(body_bytes).unwrap()).unwrap();

        let response = format!(
            "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
            response_body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
    });

    (address, rx)
}

#[test]
fn runner_claims_executes_and_emits_follow_up() {
    let exchange = SqliteExchange::open_memory().unwrap();
    exchange.init().unwrap();

    let root = exchange
        .publish(NewMessage::new("code.generate", "hello world"))
        .unwrap();

    let plugin = CommandPlugin::new(vec!["sh".into(), "-c".into(), "tr a-z A-Z".into()]).unwrap();
    let runner = WorkerHost::new(
        &exchange,
        &plugin,
        WorkerConfig::new("code.generate", "code.generated", "code.generate.failed", 5),
    );

    let outcome = runner.run_once().unwrap();
    assert!(matches!(outcome, RunOnceOutcome::Handled { .. }));

    let conversation = exchange
        .read_by_conversation(&root.conversation_id)
        .unwrap();
    assert_eq!(conversation.len(), 2);
    assert_eq!(conversation[1].topic, "code.generated");
    assert_eq!(conversation[1].body.trim(), "HELLO WORLD");
    assert_eq!(conversation[1].parent_id.as_deref(), Some(root.id.as_str()));

    let claims = exchange.claims_for_message(&root.id).unwrap();
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].status, ClaimStatus::Completed);
}

#[test]
fn runner_emits_failure_follow_up_for_non_zero_exit() {
    let exchange = SqliteExchange::open_memory().unwrap();
    exchange.init().unwrap();

    let root = exchange
        .publish(NewMessage::new("code.generate", "hello world"))
        .unwrap();

    let plugin = CommandPlugin::new(vec![
        "sh".into(),
        "-c".into(),
        "printf 'bad input' >&2; exit 2".into(),
    ])
    .unwrap();
    let runner = WorkerHost::new(
        &exchange,
        &plugin,
        WorkerConfig::new("code.generate", "code.generated", "code.generate.failed", 5),
    );

    let outcome = runner.run_once().unwrap();
    assert!(matches!(outcome, RunOnceOutcome::Handled { .. }));

    let conversation = exchange
        .read_by_conversation(&root.conversation_id)
        .unwrap();
    assert_eq!(conversation.len(), 2);
    assert_eq!(conversation[1].topic, "code.generate.failed");
    assert_eq!(conversation[1].body, "bad input");
    assert_eq!(conversation[1].parent_id.as_deref(), Some(root.id.as_str()));

    let claims = exchange.claims_for_message(&root.id).unwrap();
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].status, ClaimStatus::Failed);
}

#[test]
fn runner_emits_timeout_follow_up_for_long_command() {
    let exchange = SqliteExchange::open_memory().unwrap();
    exchange.init().unwrap();

    let root = exchange
        .publish(NewMessage::new("code.generate", "hello world"))
        .unwrap();

    let plugin = CommandPlugin::new(vec!["sh".into(), "-c".into(), "sleep 2".into()]).unwrap();
    let runner = WorkerHost::new(
        &exchange,
        &plugin,
        WorkerConfig::new("code.generate", "code.generated", "code.generate.failed", 1),
    );

    let outcome = runner.run_once().unwrap();
    assert!(matches!(outcome, RunOnceOutcome::Handled { .. }));

    let conversation = exchange
        .read_by_conversation(&root.conversation_id)
        .unwrap();
    assert_eq!(conversation.len(), 2);
    assert_eq!(conversation[1].topic, "code.generate.timed_out");
    assert!(conversation[1].body.contains("timed out"));
    assert_eq!(conversation[1].parent_id.as_deref(), Some(root.id.as_str()));

    let claims = exchange.claims_for_message(&root.id).unwrap();
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].status, ClaimStatus::TimedOut);
}

#[test]
fn example_review_plugin_emits_deterministic_follow_up() {
    let exchange = SqliteExchange::open_memory().unwrap();
    exchange.init().unwrap();

    let root = exchange
        .publish(NewMessage::new("review.request", "Check timeout handling"))
        .unwrap();

    let plugin =
        CommandPlugin::new(vec![env!("CARGO_BIN_EXE_example-review-plugin").into()]).unwrap();
    let runner = WorkerHost::new(
        &exchange,
        &plugin,
        WorkerConfig::new("review.request", "review.done", "review.failed", 5),
    );

    let outcome = runner.run_once().unwrap();
    assert!(matches!(outcome, RunOnceOutcome::Handled { .. }));

    let conversation = exchange
        .read_by_conversation(&root.conversation_id)
        .unwrap();
    assert_eq!(conversation.len(), 2);
    assert_eq!(conversation[1].topic, "review.done");
    assert!(conversation[1].body.contains("Review status: ok"));
    assert!(
        conversation[1]
            .body
            .contains("Reviewer: example-review-plugin")
    );
    assert!(
        conversation[1]
            .body
            .contains("Input: Check timeout handling")
    );
    assert_eq!(conversation[1].parent_id.as_deref(), Some(root.id.as_str()));

    let claims = exchange.claims_for_message(&root.id).unwrap();
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].status, ClaimStatus::Completed);
}

#[test]
fn gemini_plugin_runs_through_worker_host() {
    let exchange = SqliteExchange::open_memory().unwrap();
    exchange.init().unwrap();

    let temp = tempfile::tempdir().unwrap();
    let fake_gemini = temp.path().join("fake-gemini");
    fs::write(
        &fake_gemini,
        r#"#!/bin/sh
stdin_contents=$(cat)
if [ -n "$stdin_contents" ]; then
  printf 'stdin should be empty' >&2
  exit 1
fi
if [ "$1" != "--prompt" ]; then
  printf 'missing prompt flag' >&2
  exit 1
fi
if [ "$2" != "Review this Rust code for timeout bugs" ]; then
  printf 'unexpected prompt: %s' "$2" >&2
  exit 1
fi
printf '{ "session_id": "session-1", "response": "Gemini worker reply" }'
"#,
    )
    .unwrap();
    let mut perms = fs::metadata(&fake_gemini).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake_gemini, perms).unwrap();

    let root = exchange
        .publish(NewMessage::new(
            "gemini.review.request",
            "Review this Rust code for timeout bugs",
        ))
        .unwrap();

    let plugin = CommandPlugin::new(vec![
        "env".into(),
        format!("GEMINI_PLUGIN_CLI={}", fake_gemini.display()),
        env!("CARGO_BIN_EXE_gemini-plugin").into(),
    ])
    .unwrap();
    let runner = WorkerHost::new(
        &exchange,
        &plugin,
        WorkerConfig::new(
            "gemini.review.request",
            "gemini.review.done",
            "gemini.review.failed",
            5,
        ),
    );

    let outcome = runner.run_once().unwrap();
    assert!(matches!(outcome, RunOnceOutcome::Handled { .. }));

    let conversation = exchange
        .read_by_conversation(&root.conversation_id)
        .unwrap();
    assert_eq!(conversation.len(), 2);
    assert_eq!(conversation[1].topic, "gemini.review.done");
    assert_eq!(conversation[1].body, "Gemini worker reply");
    assert_eq!(conversation[1].parent_id.as_deref(), Some(root.id.as_str()));

    let claims = exchange.claims_for_message(&root.id).unwrap();
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].status, ClaimStatus::Completed);
}

#[test]
fn ollama_plugin_runs_through_worker_host() {
    let exchange = SqliteExchange::open_memory().unwrap();
    exchange.init().unwrap();

    let (base_url, request_rx) = spawn_fake_ollama(
        "200 OK",
        r#"{ "response": "Local worker reply", "done": true }"#,
    );

    let root = exchange
        .publish(NewMessage {
            topic: "local.review.request".into(),
            body: "Explain the timeout behavior in one line".into(),
            parent_id: None,
            conversation_id: None,
            producer: None,
            metadata_json: Some(r#"{"meta":{"model":"llama3.2:3b"}}"#.into()),
        })
        .unwrap();

    let plugin = CommandPlugin::new(vec![
        "env".into(),
        format!("OLLAMA_PLUGIN_BASE_URL={base_url}"),
        "OLLAMA_PLUGIN_MODEL=gemma3:1b".into(),
        env!("CARGO_BIN_EXE_ollama-plugin").into(),
    ])
    .unwrap();
    let runner = WorkerHost::new(
        &exchange,
        &plugin,
        WorkerConfig::new(
            "local.review.request",
            "local.review.done",
            "local.review.failed",
            5,
        ),
    );

    let outcome = runner.run_once().unwrap();
    assert!(matches!(outcome, RunOnceOutcome::Handled { .. }));

    let conversation = exchange
        .read_by_conversation(&root.conversation_id)
        .unwrap();
    assert_eq!(conversation.len(), 2);
    assert_eq!(conversation[1].topic, "local.review.done");
    assert_eq!(conversation[1].body, "Local worker reply");
    assert_eq!(conversation[1].parent_id.as_deref(), Some(root.id.as_str()));

    let request = request_rx.recv().unwrap();
    assert!(request.contains(r#""model":"llama3.2:3b""#));
    assert!(request.contains(r#""prompt":"Explain the timeout behavior in one line""#));
    assert!(request.contains(r#""stream":false"#));

    let claims = exchange.claims_for_message(&root.id).unwrap();
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].status, ClaimStatus::Completed);
}
