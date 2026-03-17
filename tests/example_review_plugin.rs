use std::io::Write;
use std::process::{Command, Stdio};

fn run_plugin(stdin_payload: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_example-review-plugin"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(stdin_payload.as_bytes()).unwrap();
    }

    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn plugin_formats_non_empty_input() {
    let output = run_plugin("Check timeout handling");

    assert!(output.contains("Review status: ok"));
    assert!(output.contains("Reviewer: example-review-plugin"));
    assert!(output.contains("Input: Check timeout handling"));
}

#[test]
fn plugin_uses_placeholder_for_empty_input() {
    let output = run_plugin("   ");

    assert!(output.contains("Input: <empty>"));
}

#[test]
fn plugin_escapes_control_characters_in_output() {
    let output = run_plugin("line 1\nline 2\t\u{1b}[31m");

    assert!(output.contains(r"Input: line 1\nline 2\t\u{1b}[31m"));
}
