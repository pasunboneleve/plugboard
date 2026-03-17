use std::io::Write;
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
            stdin.write_all(input.message.body.as_bytes())?;
            stdin.flush()?;
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
