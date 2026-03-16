use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use serde_json::json;
use wait_timeout::ChildExt;

use crate::domain::{Message, NewMessage};
use crate::error::{PlugboardError, Result};
use crate::exchange::Exchange;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeTopics {
    pub success: String,
    pub failure: String,
    // Timeout follows the original topic by default to keep v1 runner setup small.
    pub timeout: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerConfig {
    pub topic: String,
    pub runner_name: String,
    pub timeout_seconds: u64,
    pub idle_sleep: Duration,
    pub command: Vec<String>,
    pub outcome_topics: OutcomeTopics,
}

impl RunnerConfig {
    // v1 keeps timeout topic derivation implicit: "<watched topic>.timed_out".
    pub fn new(
        topic: impl Into<String>,
        success_topic: impl Into<String>,
        failure_topic: impl Into<String>,
        timeout_seconds: u64,
        command: Vec<String>,
    ) -> Self {
        let topic = topic.into();
        Self {
            runner_name: format!("{topic}-runner"),
            outcome_topics: OutcomeTopics {
                success: success_topic.into(),
                failure: failure_topic.into(),
                timeout: format!("{topic}.timed_out"),
            },
            topic,
            timeout_seconds,
            idle_sleep: Duration::from_millis(250),
            command,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunOnceOutcome {
    Idle,
    Handled {
        message_id: String,
        follow_up_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandResult {
    Success {
        stdout: String,
        stderr: String,
        exit_code: i32,
    },
    Failed {
        stdout: String,
        stderr: String,
        exit_code: i32,
    },
    TimedOut {
        stdout: String,
        stderr: String,
    },
}

pub struct CommandRunner<'a, E: Exchange> {
    exchange: &'a E,
    config: RunnerConfig,
}

impl<'a, E: Exchange> CommandRunner<'a, E> {
    pub fn new(exchange: &'a E, config: RunnerConfig) -> Self {
        Self { exchange, config }
    }

    pub fn run_forever(&self) -> Result<()> {
        loop {
            match self.run_once()? {
                RunOnceOutcome::Idle => thread::sleep(self.config.idle_sleep),
                RunOnceOutcome::Handled { .. } => {}
            }
        }
    }

    pub fn run_once(&self) -> Result<RunOnceOutcome> {
        let Some((message, claim)) = self.exchange.claim_next(
            &self.config.topic,
            &self.config.runner_name,
            self.config.timeout_seconds as i64,
        )?
        else {
            return Ok(RunOnceOutcome::Idle);
        };

        let result = self.execute(&message.body)?;
        let follow_up = self.publish_result(&message, &claim.id, result)?;

        Ok(RunOnceOutcome::Handled {
            message_id: message.id,
            follow_up_id: follow_up.id,
        })
    }

    fn publish_result(
        &self,
        message: &Message,
        claim_id: &str,
        result: CommandResult,
    ) -> Result<Message> {
        let follow_up = build_follow_up_message(message, &self.config, &result);

        match result {
            CommandResult::Success { .. } => {
                self.exchange.complete_claim(claim_id)?;
            }
            CommandResult::Failed { .. } => {
                self.exchange.fail_claim(claim_id)?;
            }
            CommandResult::TimedOut { .. } => {
                self.exchange.timeout_claim(claim_id)?;
            }
        }

        self.exchange.publish(follow_up)
    }

    fn execute(&self, body: &str) -> Result<CommandResult> {
        let Some(program) = self.config.command.first() else {
            return Err(PlugboardError::EmptyCommand);
        };

        let mut child = Command::new(program)
            .args(&self.config.command[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(body.as_bytes())?;
        }

        let timeout = Duration::from_secs(self.config.timeout_seconds);
        if child.wait_timeout(timeout)?.is_none() {
            child.kill()?;
            let output = child.wait_with_output()?;
            return Ok(CommandResult::TimedOut {
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        let output = child.wait_with_output()?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let exit_code = output.status.code().unwrap_or(-1);

        if output.status.success() {
            Ok(CommandResult::Success {
                stdout,
                stderr,
                exit_code,
            })
        } else {
            Ok(CommandResult::Failed {
                stdout,
                stderr,
                exit_code,
            })
        }
    }
}

pub fn build_follow_up_message(
    message: &Message,
    config: &RunnerConfig,
    result: &CommandResult,
) -> NewMessage {
    let mut follow_up = NewMessage {
        topic: String::new(),
        body: String::new(),
        parent_id: Some(message.id.clone()),
        conversation_id: Some(message.conversation_id.clone()),
        producer: Some(config.runner_name.clone()),
        metadata_json: None,
    };

    match result {
        CommandResult::Success {
            stdout,
            stderr,
            exit_code,
        } => {
            follow_up.topic = config.outcome_topics.success.clone();
            follow_up.body = stdout.clone();
            follow_up.metadata_json = Some(
                json!({
                    "stdout": stdout,
                    "stderr": stderr,
                    "exit_code": exit_code,
                    "status": "completed",
                })
                .to_string(),
            );
        }
        CommandResult::Failed {
            stdout,
            stderr,
            exit_code,
        } => {
            follow_up.topic = config.outcome_topics.failure.clone();
            follow_up.body = if stderr.is_empty() {
                if stdout.is_empty() {
                    format!("command exited with code {exit_code}")
                } else {
                    stdout.clone()
                }
            } else {
                stderr.clone()
            };
            follow_up.metadata_json = Some(
                json!({
                    "stdout": stdout,
                    "stderr": stderr,
                    "exit_code": exit_code,
                    "status": "failed",
                })
                .to_string(),
            );
        }
        CommandResult::TimedOut { stdout, stderr } => {
            follow_up.topic = config.outcome_topics.timeout.clone();
            let mut body = format!("command timed out after {} seconds", config.timeout_seconds);
            if !stderr.is_empty() {
                body.push('\n');
                body.push_str(stderr);
            }
            follow_up.body = body;
            follow_up.metadata_json = Some(
                json!({
                    "stdout": stdout,
                    "stderr": stderr,
                    "status": "timed_out",
                })
                .to_string(),
            );
        }
    }

    follow_up
}

#[cfg(test)]
mod tests {
    use super::{CommandResult, OutcomeTopics, RunnerConfig, build_follow_up_message};
    use crate::domain::Message;

    fn base_message() -> Message {
        Message {
            id: "message-1".into(),
            topic: "code.generate".into(),
            body: "make it".into(),
            created_at: "2026-03-16T00:00:00Z".into(),
            parent_id: None,
            conversation_id: "conversation-1".into(),
            producer: Some("planner".into()),
            metadata_json: None,
        }
    }

    fn base_config() -> RunnerConfig {
        RunnerConfig {
            topic: "code.generate".into(),
            runner_name: "generator".into(),
            timeout_seconds: 5,
            idle_sleep: std::time::Duration::from_millis(10),
            command: vec!["cat".into()],
            outcome_topics: OutcomeTopics {
                success: "code.generated".into(),
                failure: "code.generate.failed".into(),
                timeout: "code.generate.timed_out".into(),
            },
        }
    }

    #[test]
    fn success_follow_up_keeps_parent_and_conversation() {
        let follow_up = build_follow_up_message(
            &base_message(),
            &base_config(),
            &CommandResult::Success {
                stdout: "done".into(),
                stderr: String::new(),
                exit_code: 0,
            },
        );

        assert_eq!(follow_up.topic, "code.generated");
        assert_eq!(follow_up.parent_id.as_deref(), Some("message-1"));
        assert_eq!(follow_up.conversation_id.as_deref(), Some("conversation-1"));
    }

    #[test]
    fn failure_follow_up_prefers_stderr_body() {
        let follow_up = build_follow_up_message(
            &base_message(),
            &base_config(),
            &CommandResult::Failed {
                stdout: String::new(),
                stderr: "bad input".into(),
                exit_code: 2,
            },
        );

        assert_eq!(follow_up.topic, "code.generate.failed");
        assert_eq!(follow_up.body, "bad input");
    }

    #[test]
    fn timeout_follow_up_uses_timeout_topic() {
        let follow_up = build_follow_up_message(
            &base_message(),
            &base_config(),
            &CommandResult::TimedOut {
                stdout: String::new(),
                stderr: String::new(),
            },
        );

        assert_eq!(follow_up.topic, "code.generate.timed_out");
        assert!(follow_up.body.contains("timed out"));
    }

    #[test]
    fn runner_config_derives_timeout_topic_from_watched_topic() {
        let config = RunnerConfig::new(
            "code.generate",
            "code.generated",
            "code.generate.failed",
            5,
            vec!["cat".into()],
        );

        assert_eq!(config.outcome_topics.timeout, "code.generate.timed_out");
    }
}
