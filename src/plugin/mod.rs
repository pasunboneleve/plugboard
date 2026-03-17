pub mod command;

use crate::domain::Message;
use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginContext {
    pub worker_name: String,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInput<'a> {
    pub message: &'a Message,
    pub context: &'a PluginContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginResult {
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

pub trait Plugin {
    fn name(&self) -> &str;

    fn run(&self, input: PluginInput<'_>) -> Result<PluginResult>;
}
