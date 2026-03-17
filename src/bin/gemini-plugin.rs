use std::env;
use std::io::{self, BufRead, BufReader, Read};
use std::process::{Command, ExitCode, Stdio};

use serde::Deserialize;

const DEFAULT_GEMINI_CLI: &str = "gemini";
const DEFAULT_APPROVAL_MODE: &str = "plan";
const DEFAULT_OUTPUT_FORMAT: &str = "stream-json";
const MAX_STDERR_LEN: usize = 1024;

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct GeminiError {
    message: String,
    #[serde(default)]
    code: Option<i32>,
    #[serde(default)]
    r#type: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct GeminiJsonError {
    error: GeminiError,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct GeminiStreamMessage {
    #[serde(default)]
    r#type: String,
    #[serde(default)]
    role: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    result: String,
    #[serde(default)]
    message: NestedMessage,
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
struct NestedMessage {
    #[serde(default)]
    content: String,
}

// Plugboard delivers the claimed message body to this binary on stdin.
// This adapter reads that body once, then invokes Gemini separately with:
//   gemini --output-format stream-json --approval-mode plan
// The message body is piped to Gemini on stdin, matching RoboRev's working
// Gemini integration. stdin is therefore used at both boundaries:
// Plugboard -> plugin and plugin -> Gemini.
fn build_gemini_args(model: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "--output-format".to_string(),
        DEFAULT_OUTPUT_FORMAT.to_string(),
        "--approval-mode".to_string(),
        DEFAULT_APPROVAL_MODE.to_string(),
    ];

    if let Some(model) = model {
        args.push("--model".to_string());
        args.push(model.to_string());
    }

    args
}

fn truncate_stderr(stderr: &str) -> String {
    if stderr.len() <= MAX_STDERR_LEN {
        return stderr.to_string();
    }
    format!("{}... (truncated)", &stderr[..MAX_STDERR_LEN])
}

fn parse_json_error(stdout: &str) -> Option<String> {
    serde_json::from_str::<GeminiJsonError>(stdout)
        .ok()
        .map(|payload| payload.error.message)
}

fn render_error_message(stdout: &str, stderr: &str) -> String {
    if let Some(message) = parse_json_error(stdout) {
        return message;
    }

    let stderr = stderr.trim();
    if !stderr.is_empty() {
        return truncate_stderr(stderr);
    }

    let stdout = stdout.trim();
    if !stdout.is_empty() {
        return stdout.to_string();
    }

    "gemini invocation failed without output".to_string()
}

fn parse_stream_json(stdout: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    let reader = BufReader::new(stdout);
    let mut valid_events = 0usize;
    let mut final_result = String::new();
    let mut assistant_messages: Vec<String> = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Ok(message) = serde_json::from_str::<GeminiStreamMessage>(trimmed) else {
            continue;
        };
        valid_events += 1;

        if message.r#type == "message" && message.role == "assistant" && !message.content.is_empty()
        {
            assistant_messages.push(message.content);
        }
        if message.r#type == "assistant" && !message.message.content.is_empty() {
            assistant_messages.push(message.message.content);
        }
        if message.r#type == "tool" || message.r#type == "tool_result" {
            assistant_messages.clear();
        }
        if message.r#type == "result" && !message.result.is_empty() {
            final_result = message.result;
        }
    }

    if valid_events == 0 {
        return Err("no valid stream-json events parsed from output".into());
    }
    if !final_result.is_empty() {
        return Ok(final_result);
    }
    if !assistant_messages.is_empty() {
        return Ok(assistant_messages.join("\n"));
    }
    Ok(String::new())
}

fn main() -> ExitCode {
    if let Err(error) = run() {
        eprintln!("{error}");
        return ExitCode::from(1);
    }

    ExitCode::SUCCESS
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut prompt = String::new();
    io::stdin().read_to_string(&mut prompt)?;

    let gemini_cli =
        env::var("GEMINI_PLUGIN_CLI").unwrap_or_else(|_| DEFAULT_GEMINI_CLI.to_string());
    let gemini_model = env::var("GEMINI_PLUGIN_MODEL").ok();
    let args = build_gemini_args(gemini_model.as_deref());

    let mut child = Command::new(&gemini_cli)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(prompt.as_bytes())?;
        drop(stdin);
    }

    let output = child.wait_with_output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    if !output.status.success() {
        return Err(render_error_message(&stdout, &stderr).into());
    }

    let result = parse_stream_json(&output.stdout)?;
    print!("{result}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{build_gemini_args, parse_stream_json, render_error_message};

    #[test]
    fn builds_stream_json_args() {
        assert_eq!(
            build_gemini_args(None),
            vec!["--output-format", "stream-json", "--approval-mode", "plan",]
        );
    }

    #[test]
    fn appends_model_when_present() {
        assert_eq!(
            build_gemini_args(Some("gemini-2.5-flash")),
            vec![
                "--output-format",
                "stream-json",
                "--approval-mode",
                "plan",
                "--model",
                "gemini-2.5-flash",
            ]
        );
    }

    #[test]
    fn prefers_result_event() {
        let output = parse_stream_json(
            br#"{"type":"assistant","message":{"content":"Working"}}
{"type":"result","result":"Done"}"#,
        )
        .unwrap();

        assert_eq!(output, "Done");
    }

    #[test]
    fn falls_back_to_assistant_messages() {
        let output = parse_stream_json(
            br#"{"type":"assistant","message":{"content":"First"}}
{"type":"assistant","message":{"content":"Second"}}"#,
        )
        .unwrap();

        assert_eq!(output, "First\nSecond");
    }

    #[test]
    fn drops_pre_tool_messages() {
        let output = parse_stream_json(
            br#"{"type":"assistant","message":{"content":"Planning"}}
{"type":"tool","name":"Read"}
{"type":"assistant","message":{"content":"Final finding"}}"#,
        )
        .unwrap();

        assert_eq!(output, "Final finding");
    }

    #[test]
    fn prefers_json_error_message() {
        let stdout = r#"{
  "error": {
    "type": "Error",
    "message": "auth failed",
    "code": 1
  }
}"#;

        assert_eq!(render_error_message(stdout, "ignored"), "auth failed");
    }

    #[test]
    fn falls_back_to_stderr_when_stdout_is_not_json() {
        assert_eq!(
            render_error_message("not json", "gemini crashed"),
            "gemini crashed"
        );
    }

    #[test]
    fn falls_back_to_stdout_when_stderr_is_empty() {
        assert_eq!(
            render_error_message("plain failure output", ""),
            "plain failure output"
        );
    }
}
