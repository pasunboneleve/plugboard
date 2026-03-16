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
