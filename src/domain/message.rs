#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub id: String,
    pub topic: String,
    pub body: String,
    pub created_at: String,
    pub parent_id: Option<String>,
    pub conversation_id: String,
    pub producer: Option<String>,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewMessage {
    pub topic: String,
    pub body: String,
    pub parent_id: Option<String>,
    pub conversation_id: Option<String>,
    pub producer: Option<String>,
    pub metadata_json: Option<String>,
}

impl NewMessage {
    pub fn new(topic: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            topic: topic.into(),
            body: body.into(),
            parent_id: None,
            conversation_id: None,
            producer: None,
            metadata_json: None,
        }
    }

    pub fn resolved_conversation_id(&self, message_id: &str, parent: Option<&Message>) -> String {
        self.conversation_id
            .clone()
            .or_else(|| parent.map(|message| message.conversation_id.clone()))
            .unwrap_or_else(|| message_id.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{Message, NewMessage};

    #[test]
    fn new_message_defaults_to_optional_fields() {
        let message = NewMessage::new("code.generate", "body");

        assert_eq!(message.topic, "code.generate");
        assert_eq!(message.body, "body");
        assert_eq!(message.parent_id, None);
        assert_eq!(message.conversation_id, None);
        assert_eq!(message.producer, None);
        assert_eq!(message.metadata_json, None);
    }

    #[test]
    fn follow_up_inherits_parent_conversation_when_missing() {
        let parent = Message {
            id: "message-1".into(),
            topic: "code.generate".into(),
            body: "body".into(),
            created_at: "2026-03-16T00:00:00Z".into(),
            parent_id: None,
            conversation_id: "conversation-1".into(),
            producer: Some("planner".into()),
            metadata_json: None,
        };

        let message = NewMessage::new("code.generated", "result");

        assert_eq!(
            message.resolved_conversation_id("message-2", Some(&parent)),
            "conversation-1"
        );
    }
}
