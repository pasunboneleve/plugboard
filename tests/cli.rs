use std::process::Command;

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
    assert!(stdout.contains("writes the claimed message body"));
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
}
