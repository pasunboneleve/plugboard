use std::io::{ErrorKind, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::Value;
use wait_timeout::ChildExt;

use crate::error::{PlugboardError, Result};

use super::{Plugin, PluginInput, PluginResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPlugin {
    name: String,
    command: Vec<String>,
}

impl CommandPlugin {
    pub fn new(command: Vec<String>) -> Result<Self> {
        let Some(program) = command.first() else {
            return Err(PlugboardError::EmptyCommand);
        };

        Ok(Self {
            name: program.clone(),
            command,
        })
    }
}

impl Plugin for CommandPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn run(&self, input: PluginInput<'_>) -> Result<PluginResult> {
        let mut command = Command::new(&self.command[0]);
        command
            .args(&self.command[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in metadata_env_vars(input.message.metadata_json.as_deref()) {
            command.env(key, value);
        }
        let mut child = command.spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            if let Err(error) = stdin.write_all(input.message.body.as_bytes()) {
                if error.kind() != ErrorKind::BrokenPipe {
                    return Err(error.into());
                }
            } else {
                if let Err(error) = stdin.flush() {
                    if error.kind() != ErrorKind::BrokenPipe {
                        return Err(error.into());
                    }
                }
            }
            drop(stdin);
        }

        let timeout = Duration::from_secs(input.context.timeout_seconds);
        if child.wait_timeout(timeout)?.is_none() {
            child.kill()?;
            let output = child.wait_with_output()?;
            return Ok(PluginResult::TimedOut {
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        let output = child.wait_with_output()?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let exit_code = output.status.code().unwrap_or(-1);

        if output.status.success() {
            Ok(PluginResult::Success {
                stdout,
                stderr,
                exit_code,
            })
        } else {
            Ok(PluginResult::Failed {
                stdout,
                stderr,
                exit_code,
            })
        }
    }
}

fn metadata_env_vars(metadata_json: Option<&str>) -> Vec<(String, String)> {
    let Some(metadata_json) = metadata_json else {
        return Vec::new();
    };
    let Ok(Value::Object(root)) = serde_json::from_str::<Value>(metadata_json) else {
        return Vec::new();
    };
    let Some(Value::Object(meta)) = root.get("meta") else {
        return Vec::new();
    };

    meta.iter()
        .map(|(key, value)| {
            (
                format!("PLUGBOARD_META_{}", normalize_env_key(key)),
                env_value(value),
            )
        })
        .collect()
}

fn normalize_env_key(key: &str) -> String {
    key.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn env_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::Message;
    use crate::plugin::{Plugin, PluginContext, PluginInput, PluginResult};
    use serde_json::json;

    use super::{CommandPlugin, metadata_env_vars};

    fn message(body: &str) -> Message {
        Message {
            id: "message-1".into(),
            topic: "review.request".into(),
            body: body.into(),
            created_at: "2026-03-17T00:00:00Z".into(),
            parent_id: None,
            conversation_id: "conversation-1".into(),
            producer: Some("tester".into()),
            metadata_json: None,
        }
    }

    fn context() -> PluginContext {
        PluginContext {
            worker_name: "worker-1".into(),
            timeout_seconds: 5,
        }
    }

    #[test]
    fn ignores_broken_pipe_when_command_closes_stdin_early() {
        let plugin = CommandPlugin::new(vec!["sh".into(), "-c".into(), "exit 0".into()]).unwrap();
        let message = message("body that will not be read");
        let context = context();

        let result = plugin
            .run(PluginInput {
                message: &message,
                context: &context,
            })
            .unwrap();

        assert!(matches!(result, PluginResult::Success { exit_code: 0, .. }));
    }

    #[test]
    fn injects_meta_env_vars_per_invocation() {
        let plugin = CommandPlugin::new(vec![
            "sh".into(),
            "-c".into(),
            "printf '%s|%s|%s' \"$PLUGBOARD_META_MODEL\" \"$PLUGBOARD_META_TEMPERATURE\" \"$PLUGBOARD_META_DEBUG\"".into(),
        ])
        .unwrap();
        let mut message = message("body");
        message.metadata_json = Some(
            json!({
                "meta": {
                    "model": "llama3.2:3b",
                    "temperature": 0.7,
                    "debug": true
                }
            })
            .to_string(),
        );

        let result = plugin
            .run(PluginInput {
                message: &message,
                context: &context(),
            })
            .unwrap();

        match result {
            PluginResult::Success { stdout, .. } => assert_eq!(stdout, "llama3.2:3b|0.7|true"),
            other => panic!("expected success, got {other:?}"),
        }
    }

    #[test]
    fn extracts_only_meta_object_from_metadata_json() {
        let vars = metadata_env_vars(Some(
            &json!({
                "stdout": "ignored",
                "meta": {
                    "model-name": "llama3.2:3b",
                    "temperature": 0.7
                }
            })
            .to_string(),
        ));

        assert!(vars.contains(&("PLUGBOARD_META_MODEL_NAME".into(), "llama3.2:3b".into())));
        assert!(vars.contains(&("PLUGBOARD_META_TEMPERATURE".into(), "0.7".into())));
        assert_eq!(vars.len(), 2);
    }
}
