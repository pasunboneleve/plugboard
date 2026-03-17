use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

fn write_fake_gemini_script(body: &str) -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let script = temp.path().join("fake-gemini");
    fs::write(&script, body).unwrap();
    let mut perms = fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script, perms).unwrap();
    temp
}

#[test]
fn gemini_plugin_emits_response_from_json_output() {
    let temp = write_fake_gemini_script(
        r#"#!/bin/sh
stdin_contents=$(cat)
if [ "$stdin_contents" != "prompt body" ]; then
  printf 'unexpected stdin: %s' "$stdin_contents" >&2
  exit 1
fi
if [ "$1" != "--prompt" ]; then
  printf 'missing prompt flag' >&2
  exit 1
fi
if [ "$2" != "Reply to the message provided on stdin." ]; then
  printf 'unexpected prompt: %s' "$2" >&2
  exit 1
fi
printf '{ "session_id": "session-1", "response": "Gemini says hello" }'
"#,
    );
    let binary = env!("CARGO_BIN_EXE_gemini-plugin");

    let output = Command::new(binary)
        .env("GEMINI_PLUGIN_CLI", temp.path().join("fake-gemini"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.take().unwrap().write_all(b"prompt body")?;
            child.wait_with_output()
        })
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "Gemini says hello");
}

#[test]
fn gemini_plugin_allows_prompt_override() {
    let temp = write_fake_gemini_script(
        r#"#!/bin/sh
stdin_contents=$(cat)
if [ "$stdin_contents" != "prompt body" ]; then
  printf 'unexpected stdin: %s' "$stdin_contents" >&2
  exit 1
fi
if [ "$2" != "Custom instruction" ]; then
  printf 'unexpected prompt: %s' "$2" >&2
  exit 1
fi
printf '{ "session_id": "session-1", "response": "Gemini says hello" }'
"#,
    );
    let binary = env!("CARGO_BIN_EXE_gemini-plugin");

    let output = Command::new(binary)
        .env("GEMINI_PLUGIN_CLI", temp.path().join("fake-gemini"))
        .env("GEMINI_PLUGIN_PROMPT", "Custom instruction")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.take().unwrap().write_all(b"prompt body")?;
            child.wait_with_output()
        })
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "Gemini says hello");
}

#[test]
fn gemini_plugin_reports_json_error_message() {
    let temp = write_fake_gemini_script(
        r#"#!/bin/sh
printf '{ "session_id": "session-1", "error": { "type": "Error", "message": "Gemini auth failed", "code": 1 } }'
exit 1
"#,
    );
    let binary = env!("CARGO_BIN_EXE_gemini-plugin");

    let output = Command::new(binary)
        .env("GEMINI_PLUGIN_CLI", temp.path().join("fake-gemini"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.take().unwrap().write_all(b"prompt body")?;
            child.wait_with_output()
        })
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Gemini auth failed"));
}

#[test]
fn gemini_plugin_reports_raw_stderr_when_output_is_not_json() {
    let temp = write_fake_gemini_script(
        r#"#!/bin/sh
printf 'gemini transport failed' >&2
exit 1
"#,
    );
    let binary = env!("CARGO_BIN_EXE_gemini-plugin");

    let output = Command::new(binary)
        .env("GEMINI_PLUGIN_CLI", temp.path().join("fake-gemini"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.take().unwrap().write_all(b"prompt body")?;
            child.wait_with_output()
        })
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("gemini transport failed"));
}
