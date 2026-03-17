use std::env;
use std::io::{self, Read};
use std::process::{Command, ExitCode, Stdio};

use serde::Deserialize;

const DEFAULT_GEMINI_CLI: &str = "gemini";
const DEFAULT_APPROVAL_MODE: &str = "plan";

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct GeminiError {
    message: String,
    #[serde(default)]
    code: Option<i32>,
    #[serde(default)]
    r#type: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
enum GeminiOutput {
    Success {
        session_id: String,
        response: String,
    },
    Failure {
        session_id: Option<String>,
        error: GeminiError,
    },
}

// Plugboard's worker contract feeds the claimed message body to this binary on stdin.
// Gemini CLI behaves more reliably when that text becomes the explicit `-p/--prompt`
// value, so this adapter reads stdin once, maps it into `--prompt`, and then runs
// Gemini as a fresh one-shot process for that message.
fn build_gemini_args(prompt: &str, model: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "--prompt".to_string(),
        prompt.to_string(),
        "--output-format".to_string(),
        "json".to_string(),
        "--approval-mode".to_string(),
        DEFAULT_APPROVAL_MODE.to_string(),
    ];

    if let Some(model) = model {
        args.push("--model".to_string());
        args.push(model.to_string());
    }

    args
}

fn parse_gemini_output(stdout: &str) -> Result<GeminiOutput, serde_json::Error> {
    serde_json::from_str(stdout)
}

fn render_error_message(stdout: &str, stderr: &str) -> String {
    if let Ok(GeminiOutput::Failure { error, .. }) = parse_gemini_output(stdout) {
        return error.message;
    }

    let stderr = stderr.trim();
    if !stderr.is_empty() {
        return stderr.to_string();
    }

    let stdout = stdout.trim();
    if !stdout.is_empty() {
        return stdout.to_string();
    }

    "gemini invocation failed without output".to_string()
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
    let args = build_gemini_args(&prompt, gemini_model.as_deref());

    let output = Command::new(&gemini_cli)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    if !output.status.success() {
        return Err(render_error_message(&stdout, &stderr).into());
    }

    match parse_gemini_output(&stdout) {
        Ok(GeminiOutput::Success { response, .. }) => {
            print!("{response}");
            Ok(())
        }
        Ok(GeminiOutput::Failure { error, .. }) => Err(error.message.into()),
        Err(_) => {
            print!("{stdout}");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GeminiOutput, build_gemini_args, parse_gemini_output, render_error_message};

    #[test]
    fn builds_non_interactive_json_args_from_prompt_body() {
        assert_eq!(
            build_gemini_args("prompt body", None),
            vec![
                "--prompt",
                "prompt body",
                "--output-format",
                "json",
                "--approval-mode",
                "plan",
            ]
        );
    }

    #[test]
    fn appends_model_when_present() {
        assert_eq!(
            build_gemini_args("prompt body", Some("gemini-2.5-flash")),
            vec![
                "--prompt",
                "prompt body",
                "--output-format",
                "json",
                "--approval-mode",
                "plan",
                "--model",
                "gemini-2.5-flash",
            ]
        );
    }

    #[test]
    fn parses_success_payload() {
        let payload = parse_gemini_output(
            r#"{
  "session_id": "session-1",
  "response": "hello"
}"#,
        )
        .unwrap();

        assert_eq!(
            payload,
            GeminiOutput::Success {
                session_id: "session-1".into(),
                response: "hello".into(),
            }
        );
    }

    #[test]
    fn parses_failure_payload() {
        let payload = parse_gemini_output(
            r#"{
  "session_id": "session-1",
  "error": {
    "type": "Error",
    "message": "auth failed",
    "code": 1
  }
}"#,
        )
        .unwrap();

        assert_eq!(
            payload,
            GeminiOutput::Failure {
                session_id: Some("session-1".into()),
                error: super::GeminiError {
                    message: "auth failed".into(),
                    code: Some(1),
                    r#type: Some("Error".into()),
                },
            }
        );
    }

    #[test]
    fn prefers_json_error_message() {
        let stdout = r#"{
  "session_id": "session-1",
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
