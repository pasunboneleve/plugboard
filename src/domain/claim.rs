use std::fmt;

use crate::error::{PlugboardError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimStatus {
    Active,
    Completed,
    Failed,
    TimedOut,
}

impl ClaimStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::TimedOut => "timed_out",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "active" => Ok(Self::Active),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "timed_out" => Ok(Self::TimedOut),
            other => Err(PlugboardError::NotFound(format!(
                "unknown claim status {other}"
            ))),
        }
    }
}

impl fmt::Display for ClaimStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    pub id: String,
    pub message_id: String,
    pub runner_name: String,
    pub claimed_at: String,
    pub lease_until: String,
    pub status: ClaimStatus,
    pub completed_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::ClaimStatus;

    #[test]
    fn claim_status_round_trips() {
        assert_eq!(ClaimStatus::parse("active").unwrap(), ClaimStatus::Active);
        assert_eq!(ClaimStatus::Failed.as_str(), "failed");
    }
}
