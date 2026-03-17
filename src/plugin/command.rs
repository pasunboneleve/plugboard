use std::io::{ErrorKind, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

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
        let mut child = Command::new(&self.command[0])
            .args(&self.command[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            if let Err(error) = stdin.write_all(input.message.body.as_bytes()) {
                if error.kind() != ErrorKind::BrokenPipe {
                    return Err(error.into());
                }
            } else {
                stdin.flush()?;
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

#[cfg(test)]
mod tests {
    use crate::domain::Message;
    use crate::plugin::{Plugin, PluginContext, PluginInput, PluginResult};

    use super::CommandPlugin;

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
}
