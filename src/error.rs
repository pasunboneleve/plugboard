use std::io;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, PlugboardError>;

#[derive(Debug, Error)]
pub enum PlugboardError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("filesystem notification error: {0}")]
    Notify(#[from] notify::Error),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("time formatting error: {0}")]
    TimeFormat(#[from] time::error::Format),
    #[error("time parsing error: {0}")]
    TimeParse(#[from] time::error::Parse),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid claim transition for claim {claim_id}: expected active state")]
    InvalidClaimTransition { claim_id: String },
    #[error("entity not found: {0}")]
    NotFound(String),
    #[error("invalid metadata argument `{input}`: expected KEY=VALUE with a non-empty key")]
    InvalidMetadataArgument { input: String },
    #[error("metadata_json must be a JSON object when merging request metadata")]
    InvalidMetadataJsonObject,
    #[error("command must not be empty")]
    EmptyCommand,
    #[error("command should exit with code {code} without additional stderr output")]
    SilentExit { code: i32 },
}
