use rusqlite::Connection;
use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

#[test]
fn publish_and_read_commands_work() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("plugboard.db");
    let binary = env!("CARGO_BIN_EXE_plugboard");

    let publish = Command::new(binary)
        .args([
            "--database",
            database.to_str().unwrap(),
            "publish",
            "code.generate",
            "hello",
        ])
        .output()
        .unwrap();
    assert!(publish.status.success());

    let read = Command::new(binary)
        .args([
            "--database",
            database.to_str().unwrap(),
            "read",
            "--topic",
            "code.generate",
        ])
        .output()
        .unwrap();
    assert!(read.status.success());

    let stdout = String::from_utf8_lossy(&read.stdout);
    assert!(stdout.contains("code.generate"));
    assert!(stdout.contains("hello"));
}

#[test]
fn run_help_describes_worker_host() {
    let binary = env!("CARGO_BIN_EXE_plugboard");

    let help = Command::new(binary)
        .args(["run", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success());

    let stdout = String::from_utf8_lossy(&help.stdout);
    assert!(stdout.contains("worker host"));
    assert!(stdout.contains("claims one message at a time"));
    assert!(stdout.contains("With --once"));
    assert!(stdout.contains("drains all currently claimable work"));
    assert!(stdout.contains("writes the claimed message body"));
    assert!(stdout.contains("default is 60 seconds"));
    assert!(stdout.contains("Raise it for slower backends such as Gemini"));
    assert!(stdout.contains("Interactive tools usually need a wrapper"));
}

#[test]
fn top_level_help_describes_topic_based_workflow() {
    let binary = env!("CARGO_BIN_EXE_plugboard");

    let help = Command::new(binary).arg("--help").output().unwrap();
    assert!(help.status.success());

    let stdout = String::from_utf8_lossy(&help.stdout);
    assert!(stdout.contains("built around topics"));
    assert!(stdout.contains("publish"));
    assert!(stdout.contains("read"));
    assert!(stdout.contains("request"));
    assert!(stdout.contains("long-running worker"));
}

#[test]
fn publish_and_read_help_are_concrete() {
    let binary = env!("CARGO_BIN_EXE_plugboard");

    let publish_help = Command::new(binary)
        .args(["publish", "--help"])
        .output()
        .unwrap();
    assert!(publish_help.status.success());
    let publish_stdout = String::from_utf8_lossy(&publish_help.stdout);
    assert!(publish_stdout.contains("Topics are the addressing mechanism"));
    assert!(publish_stdout.contains("Plain-text message body"));

    let read_help = Command::new(binary).args(["read", "--help"]).output().unwrap();
    assert!(read_help.status.success());
    let read_stdout = String::from_utf8_lossy(&read_help.stdout);
    assert!(read_stdout.contains("already published to the exchange"));
    assert!(read_stdout.contains("tab-separated"));

    let request_help = Command::new(binary)
        .args(["request", "--help"])
        .output()
        .unwrap();
    assert!(request_help.status.success());
    let request_stdout = String::from_utf8_lossy(&request_help.stdout);
    assert!(request_stdout.contains("correlated follow-up message"));
    assert!(request_stdout.contains("same conversation"));
}

#[test]
fn run_once_handles_one_message_and_exits() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("plugboard.db");
    let binary = env!("CARGO_BIN_EXE_plugboard");

    let publish = Command::new(binary)
        .args([
            "--database",
            database.to_str().unwrap(),
            "publish",
            "review.request",
            "hello world",
        ])
        .output()
        .unwrap();
    assert!(publish.status.success());

    let run_once = Command::new(binary)
        .args([
            "--database",
            database.to_str().unwrap(),
            "run",
            "--once",
            "--topic",
            "review.request",
            "--success-topic",
            "review.done",
            "--failure-topic",
            "review.failed",
            "--",
            "sh",
            "-c",
            "tr a-z A-Z",
        ])
        .output()
        .unwrap();
    assert!(run_once.status.success());

    let read = Command::new(binary)
        .args([
            "--database",
            database.to_str().unwrap(),
            "read",
            "--topic",
            "review.done",
        ])
        .output()
        .unwrap();
    assert!(read.status.success());

    let stdout = String::from_utf8_lossy(&read.stdout);
    assert!(stdout.contains("review.done"));
    assert!(stdout.contains("HELLO WORLD"));
}

#[test]
fn run_once_blocks_until_message_is_published() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("plugboard.db");
    let binary = env!("CARGO_BIN_EXE_plugboard");

    let worker = Command::new(binary)
        .args([
            "--database",
            database.to_str().unwrap(),
            "run",
            "--once",
            "--topic",
            "review.request",
            "--success-topic",
            "review.done",
            "--failure-topic",
            "review.failed",
            "--",
            "sh",
            "-c",
            "tr a-z A-Z",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    thread::sleep(Duration::from_millis(250));

    let publish = Command::new(binary)
        .args([
            "--database",
            database.to_str().unwrap(),
            "publish",
            "review.request",
            "wake up now",
        ])
        .output()
        .unwrap();
    assert!(publish.status.success());

    let worker_output = worker.wait_with_output().unwrap();
    assert!(worker_output.status.success());

    let read = Command::new(binary)
        .args([
            "--database",
            database.to_str().unwrap(),
            "read",
            "--topic",
            "review.done",
        ])
        .output()
        .unwrap();
    assert!(read.status.success());

    let stdout = String::from_utf8_lossy(&read.stdout);
    assert!(stdout.contains("review.done"));
    assert!(stdout.contains("WAKE UP NOW"));
}

#[test]
fn persistent_worker_drains_burst_after_single_change_cycle() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("plugboard.db");
    let binary = env!("CARGO_BIN_EXE_plugboard");

    let mut worker = Command::new(binary)
        .args([
            "--database",
            database.to_str().unwrap(),
            "run",
            "--topic",
            "review.request",
            "--success-topic",
            "review.done",
            "--failure-topic",
            "review.failed",
            "--",
            "sh",
            "-c",
            "tr a-z A-Z",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    thread::sleep(Duration::from_millis(250));

    for body in ["first message", "second message"] {
        let publish = Command::new(binary)
            .args([
                "--database",
                database.to_str().unwrap(),
                "publish",
                "review.request",
                body,
            ])
            .output()
            .unwrap();
        assert!(publish.status.success());
    }

    thread::sleep(Duration::from_millis(1000));

    let read = Command::new(binary)
        .args([
            "--database",
            database.to_str().unwrap(),
            "read",
            "--topic",
            "review.done",
        ])
        .output()
        .unwrap();
    assert!(read.status.success());

    let stdout = String::from_utf8_lossy(&read.stdout);
    assert_eq!(stdout.matches("review.done").count(), 2);
    assert!(stdout.contains("FIRST MESSAGE"));
    assert!(stdout.contains("SECOND MESSAGE"));

    worker.kill().unwrap();
    let _ = worker.wait_with_output().unwrap();
}

fn latest_message_for_topic(database: &std::path::Path, topic: &str) -> (String, String) {
    let connection = Connection::open(database).unwrap();
    connection
        .query_row(
            "SELECT id, conversation_id
             FROM messages
             WHERE topic = ?1
             ORDER BY created_at DESC, id DESC
             LIMIT 1",
            [topic],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
}

#[test]
fn request_publishes_and_waits_for_success_reply() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("plugboard.db");
    let binary = env!("CARGO_BIN_EXE_plugboard");

    let request = Command::new(binary)
        .args([
            "--database",
            database.to_str().unwrap(),
            "request",
            "review.request",
            "--success-topic",
            "review.done",
            "--failure-topic",
            "review.failed",
            "--body",
            "Review this code",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    thread::sleep(Duration::from_millis(250));
    let (message_id, conversation_id) = latest_message_for_topic(&database, "review.request");

    let reply = Command::new(binary)
        .args([
            "--database",
            database.to_str().unwrap(),
            "publish",
            "review.done",
            "Looks good",
            "--parent-id",
            &message_id,
            "--conversation-id",
            &conversation_id,
        ])
        .output()
        .unwrap();
    assert!(reply.status.success());

    let output = request.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "Looks good");
}

#[test]
fn request_exits_nonzero_on_failure_reply() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("plugboard.db");
    let binary = env!("CARGO_BIN_EXE_plugboard");

    let request = Command::new(binary)
        .args([
            "--database",
            database.to_str().unwrap(),
            "request",
            "review.request",
            "--success-topic",
            "review.done",
            "--failure-topic",
            "review.failed",
            "--body",
            "Review this code",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    thread::sleep(Duration::from_millis(250));
    let (message_id, conversation_id) = latest_message_for_topic(&database, "review.request");

    let reply = Command::new(binary)
        .args([
            "--database",
            database.to_str().unwrap(),
            "publish",
            "review.failed",
            "Needs tests",
            "--parent-id",
            &message_id,
            "--conversation-id",
            &conversation_id,
        ])
        .output()
        .unwrap();
    assert!(reply.status.success());

    let output = request.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "Needs tests");
}

#[test]
fn request_matches_reply_by_conversation_not_topic_alone() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("plugboard.db");
    let binary = env!("CARGO_BIN_EXE_plugboard");

    let mut request = Command::new(binary)
        .args([
            "--database",
            database.to_str().unwrap(),
            "request",
            "review.request",
            "--success-topic",
            "review.done",
            "--failure-topic",
            "review.failed",
            "--body",
            "Review this code carefully",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    thread::sleep(Duration::from_millis(250));
    let (message_id, conversation_id) = latest_message_for_topic(&database, "review.request");

    let unrelated = Command::new(binary)
        .args([
            "--database",
            database.to_str().unwrap(),
            "publish",
            "review.done",
            "Wrong conversation",
            "--conversation-id",
            "other-conversation",
        ])
        .output()
        .unwrap();
    assert!(unrelated.status.success());

    thread::sleep(Duration::from_millis(250));
    assert!(request.try_wait().unwrap().is_none());

    let related = Command::new(binary)
        .args([
            "--database",
            database.to_str().unwrap(),
            "publish",
            "review.done",
            "Right conversation",
            "--parent-id",
            &message_id,
            "--conversation-id",
            &conversation_id,
        ])
        .output()
        .unwrap();
    assert!(related.status.success());

    let output = request.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "Right conversation"
    );
}

#[test]
fn request_reads_body_from_stdin_when_flag_is_omitted() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("plugboard.db");
    let binary = env!("CARGO_BIN_EXE_plugboard");

    let mut request = Command::new(binary)
        .args([
            "--database",
            database.to_str().unwrap(),
            "request",
            "review.request",
            "--success-topic",
            "review.done",
            "--failure-topic",
            "review.failed",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    request
        .stdin
        .take()
        .unwrap()
        .write_all(b"stdin body")
        .unwrap();

    thread::sleep(Duration::from_millis(250));
    let (message_id, conversation_id) = latest_message_for_topic(&database, "review.request");

    let request_body = Command::new(binary)
        .args([
            "--database",
            database.to_str().unwrap(),
            "read",
            "--topic",
            "review.request",
        ])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&request_body.stdout).contains("stdin body"));

    let reply = Command::new(binary)
        .args([
            "--database",
            database.to_str().unwrap(),
            "publish",
            "review.done",
            "stdin ok",
            "--parent-id",
            &message_id,
            "--conversation-id",
            &conversation_id,
        ])
        .output()
        .unwrap();
    assert!(reply.status.success());

    let output = request.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "stdin ok");
}
