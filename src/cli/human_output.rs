use crate::error::Result;
use crate::util::time::now_timestamp;

pub fn prefix_timestamp(line: &str) -> Result<String> {
    Ok(format!("[{}] {}", now_timestamp()?, line))
}
