use std::time::Duration;

use serde_json::json;

use crate::domain::{Message, NewMessage};
use crate::error::Result;
use crate::exchange::Exchange;
use crate::plugin::{Plugin, PluginContext, PluginInput, PluginResult};
use crate::util::id::new_id;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeTopics {
    pub success: String,
    pub failure: String,
    pub timeout: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerConfig {
    pub topic: String,
    // worker_group is stable across equivalent workers so claims can be inspected by pool.
    pub worker_group: String,
    // worker_instance_id identifies one concrete worker process that owns a claim lease.
    pub worker_instance_id: String,
    pub timeout_seconds: u64,
    // lease_seconds bounds how long an active claim stays live without renewal.
    pub lease_seconds: u64,
    pub idle_sleep: Duration,
    pub outcome_topics: OutcomeTopics,
}

impl WorkerConfig {
    pub fn new(
        topic: impl Into<String>,
        success_topic: impl Into<String>,
        failure_topic: impl Into<String>,
        timeout_seconds: u64,
    ) -> Self {
        let topic = topic.into();
        let timeout_seconds = timeout_seconds;
        Self {
            worker_group: format!("{topic}-worker"),
            worker_instance_id: new_id(),
            outcome_topics: OutcomeTopics {
                success: success_topic.into(),
                failure: failure_topic.into(),
                timeout: format!("{topic}.timed_out"),
            },
            topic,
            timeout_seconds,
            lease_seconds: default_lease_seconds(Some(timeout_seconds)),
            idle_sleep: Duration::from_millis(250),
        }
    }
}

fn default_lease_seconds(timeout_seconds: Option<u64>) -> u64 {
    match timeout_seconds {
        Some(timeout_seconds) => timeout_seconds.saturating_add(30),
        None => 300,
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

pub struct WorkerHost<'a, E: Exchange, P: Plugin> {
    exchange: &'a E,
    plugin: &'a P,
    config: WorkerConfig,
}

impl<'a, E: Exchange, P: Plugin> WorkerHost<'a, E, P> {
    pub fn new(exchange: &'a E, plugin: &'a P, config: WorkerConfig) -> Self {
        Self {
            exchange,
            plugin,
            config,
        }
    }

    pub fn run_forever(&self) -> Result<()> {
        loop {
            self.run_once_blocking()?;
            self.drain_ready_work()?;
        }
    }

    pub fn run_once(&self) -> Result<RunOnceOutcome> {
        let Some((message, claim)) = self.exchange.claim_next(
            &self.config.topic,
            &self.config.worker_group,
            &self.config.worker_instance_id,
            self.config.lease_seconds as i64,
        )?
        else {
            return Ok(RunOnceOutcome::Idle);
        };

        self.handle_claimed_message(message, claim)
    }

    pub fn run_once_blocking(&self) -> Result<RunOnceOutcome> {
        let (message, claim) = self.exchange.claim_next_blocking(
            &self.config.topic,
            &self.config.worker_group,
            &self.config.worker_instance_id,
            self.config.lease_seconds as i64,
            self.config.idle_sleep,
        )?;

        self.handle_claimed_message(message, claim)
    }

    fn drain_ready_work(&self) -> Result<()> {
        while matches!(self.run_once()?, RunOnceOutcome::Handled { .. }) {}
        Ok(())
    }

    fn handle_claimed_message(
        &self,
        message: Message,
        claim: crate::domain::Claim,
    ) -> Result<RunOnceOutcome> {
        let context = PluginContext {
            worker_name: self.config.worker_group.clone(),
            timeout_seconds: self.config.timeout_seconds,
        };
        let result = self.plugin.run(PluginInput {
            message: &message,
            context: &context,
        })?;
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
        result: PluginResult,
    ) -> Result<Message> {
        let follow_up = build_follow_up_message(message, &self.config, self.plugin.name(), &result);

        match result {
            PluginResult::Success { .. } => {
                self.exchange.complete_claim(claim_id)?;
            }
            PluginResult::Failed { .. } => {
                self.exchange.fail_claim(claim_id)?;
            }
            PluginResult::TimedOut { .. } => {
                self.exchange.timeout_claim(claim_id)?;
            }
        }

        self.exchange.publish(follow_up)
    }
}

pub fn build_follow_up_message(
    message: &Message,
    config: &WorkerConfig,
    plugin_name: &str,
    result: &PluginResult,
) -> NewMessage {
    let mut follow_up = NewMessage {
        topic: String::new(),
        body: String::new(),
        parent_id: Some(message.id.clone()),
        conversation_id: Some(message.conversation_id.clone()),
        producer: Some(config.worker_group.clone()),
        metadata_json: None,
    };

    match result {
        PluginResult::Success {
            stdout,
            stderr,
            exit_code,
        } => {
            follow_up.topic = config.outcome_topics.success.clone();
            follow_up.body = stdout.clone();
            follow_up.metadata_json = Some(
                json!({
                    "plugin": plugin_name,
                    "stdout": stdout,
                    "stderr": stderr,
                    "exit_code": exit_code,
                    "status": "completed",
                })
                .to_string(),
            );
        }
        PluginResult::Failed {
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
                    "plugin": plugin_name,
                    "stdout": stdout,
                    "stderr": stderr,
                    "exit_code": exit_code,
                    "status": "failed",
                })
                .to_string(),
            );
        }
        PluginResult::TimedOut { stdout, stderr } => {
            follow_up.topic = config.outcome_topics.timeout.clone();
            let mut body = format!("command timed out after {} seconds", config.timeout_seconds);
            if !stderr.is_empty() {
                body.push('\n');
                body.push_str(stderr);
            }
            follow_up.body = body;
            follow_up.metadata_json = Some(
                json!({
                    "plugin": plugin_name,
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
    use super::{OutcomeTopics, WorkerConfig, build_follow_up_message};
    use crate::domain::Message;
    use crate::plugin::PluginResult;

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

    fn base_config() -> WorkerConfig {
        WorkerConfig {
            topic: "code.generate".into(),
            worker_group: "generator".into(),
            worker_instance_id: "instance-1".into(),
            timeout_seconds: 5,
            lease_seconds: 35,
            idle_sleep: std::time::Duration::from_millis(10),
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
            "command",
            &PluginResult::Success {
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
            "command",
            &PluginResult::Failed {
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
            "command",
            &PluginResult::TimedOut {
                stdout: String::new(),
                stderr: String::new(),
            },
        );

        assert_eq!(follow_up.topic, "code.generate.timed_out");
        assert!(follow_up.body.contains("timed out"));
    }

    #[test]
    fn worker_config_derives_timeout_topic_from_watched_topic() {
        let config =
            WorkerConfig::new("code.generate", "code.generated", "code.generate.failed", 5);

        assert_eq!(config.outcome_topics.timeout, "code.generate.timed_out");
        assert_eq!(config.lease_seconds, 35);
        assert_eq!(config.worker_group, "code.generate-worker");
    }
}
