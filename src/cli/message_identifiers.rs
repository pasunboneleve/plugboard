use serde_json::json;

use crate::domain::Message;
use crate::error::Result;

pub fn emit_publish_identifiers(message: &Message, json_output: bool) -> Result<()> {
    if json_output {
        eprintln!(
            "{}",
            serde_json::to_string(&json!({
                "event": "published",
                "message_id": message.id,
                "conversation_id": message.conversation_id,
                "topic": message.topic,
            }))?
        );
    } else {
        eprintln!(
            "published message_id={} conversation_id={} topic={}",
            message.id, message.conversation_id, message.topic
        );
    }
    Ok(())
}
