use plugboard::domain::{ClaimStatus, NewMessage};
use plugboard::exchange::Exchange;
use plugboard::exchange::sqlite::SqliteExchange;
use plugboard::runner::{CommandRunner, RunOnceOutcome, RunnerConfig};

#[test]
fn runner_claims_executes_and_emits_follow_up() {
    let exchange = SqliteExchange::open_memory().unwrap();
    exchange.init().unwrap();

    let root = exchange
        .publish(NewMessage::new("code.generate", "hello world"))
        .unwrap();

    let runner = CommandRunner::new(
        &exchange,
        RunnerConfig::new(
            "code.generate",
            "code.generated",
            "code.generate.failed",
            5,
            vec!["sh".into(), "-c".into(), "tr a-z A-Z".into()],
        ),
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
