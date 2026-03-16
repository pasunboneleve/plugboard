use std::io;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, PlugboardError>;

#[derive(Debug, Error)]
pub enum PlugboardError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("time formatting error: {0}")]
    TimeFormat(#[from] time::error::Format),
    #[error("time parsing error: {0}")]
    TimeParse(#[from] time::error::Parse),
    #[error("invalid claim transition for claim {claim_id}: expected active state")]
    InvalidClaimTransition { claim_id: String },
    #[error("entity not found: {0}")]
    NotFound(String),
    #[error("command must not be empty")]
    EmptyCommand,
}
