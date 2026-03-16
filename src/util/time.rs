use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

use crate::error::Result;

pub fn now_utc() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

pub fn format_timestamp(timestamp: OffsetDateTime) -> Result<String> {
    Ok(timestamp.format(&Rfc3339)?)
}

pub fn now_timestamp() -> Result<String> {
    format_timestamp(now_utc())
}

pub fn add_seconds(timestamp: OffsetDateTime, seconds: i64) -> OffsetDateTime {
    timestamp + Duration::seconds(seconds)
}
