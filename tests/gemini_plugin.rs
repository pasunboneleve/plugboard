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
if [ "$1" != "--output-format" ]; then
  printf 'missing output-format flag' >&2
  exit 1
fi
if [ "$2" != "stream-json" ]; then
  printf 'unexpected output format: %s' "$2" >&2
  exit 1
fi
if [ "$3" != "--approval-mode" ] || [ "$4" != "plan" ]; then
  printf 'unexpected approval mode args: %s %s' "$3" "$4" >&2
  exit 1
fi
printf '{"type":"result","result":"Gemini says hello"}\n'
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
fn gemini_plugin_uses_message_body_as_prompt() {
    let temp = write_fake_gemini_script(
        r#"#!/bin/sh
stdin_contents=$(cat)
if [ "$stdin_contents" != "compute 2+2" ]; then
  printf 'unexpected stdin: %s' "$stdin_contents" >&2
  exit 1
fi
printf '{"type":"assistant","message":{"content":"Gemini says hello"}}\n'
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
            child.stdin.take().unwrap().write_all(b"compute 2+2")?;
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
cat >/dev/null
printf '{ "error": { "type": "Error", "message": "Gemini auth failed", "code": 1 } }'
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
