use plugboard::domain::{ClaimStatus, NewMessage};
use plugboard::exchange::Exchange;
use plugboard::exchange::sqlite::SqliteExchange;
use plugboard::plugin::command::CommandPlugin;
use plugboard::worker::{RunOnceOutcome, WorkerConfig, WorkerHost};
use std::fs;
use std::os::unix::fs::PermissionsExt;

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
