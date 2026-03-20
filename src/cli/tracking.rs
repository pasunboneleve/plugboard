use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::domain::Message;
use crate::error::Result;

const TRACKED_STATE_FILENAME: &str = "tracked-conversations.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrackedConversation {
    pub conversation_id: String,
    pub success_topic: String,
    pub failure_topic: String,
    pub notified: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct TrackingState {
    tracked: Vec<TrackedConversation>,
}

pub fn tracking_state_path(database_path: &Path) -> PathBuf {
    database_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(TRACKED_STATE_FILENAME)
}

pub fn maybe_track_publish(message: &Message, state_path: &Path) -> Result<()> {
    let Some(prefix) = message.topic.strip_suffix(".request") else {
        return Ok(());
    };

    let tracked = TrackedConversation {
        conversation_id: message.conversation_id.clone(),
        success_topic: format!("{prefix}.done"),
        failure_topic: format!("{prefix}.failed"),
        notified: false,
    };
    upsert_tracked_conversation(state_path, tracked)
}

pub fn load_tracked_conversations(state_path: &Path) -> Result<Vec<TrackedConversation>> {
    Ok(load_state(state_path)?.tracked)
}

pub fn mark_notified(state_path: &Path, conversation_id: &str) -> Result<()> {
    let mut state = load_state(state_path)?;
    if let Some(entry) = state
        .tracked
        .iter_mut()
        .find(|entry| entry.conversation_id == conversation_id)
    {
        entry.notified = true;
        save_state(state_path, &state)?;
    }
    Ok(())
}

fn upsert_tracked_conversation(state_path: &Path, tracked: TrackedConversation) -> Result<()> {
    let mut state = load_state(state_path)?;
    if state
        .tracked
        .iter()
        .any(|entry| entry.conversation_id == tracked.conversation_id)
    {
        return Ok(());
    }
    state.tracked.push(tracked);
    save_state(state_path, &state)
}

fn load_state(state_path: &Path) -> Result<TrackingState> {
    if !state_path.exists() {
        return Ok(TrackingState::default());
    }
    let contents = fs::read_to_string(state_path)?;
    if contents.trim().is_empty() {
        return Ok(TrackingState::default());
    }
    Ok(serde_json::from_str(&contents)?)
}

fn save_state(state_path: &Path, state: &TrackingState) -> Result<()> {
    if let Some(parent) = state_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(state_path, serde_json::to_string_pretty(state)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{load_tracked_conversations, mark_notified, maybe_track_publish, tracking_state_path};
    use crate::domain::Message;

    #[test]
    fn tracking_state_path_lives_next_to_database() {
        let path = tracking_state_path(Path::new("/tmp/plugboard/plugboard.db"));
        assert_eq!(path, Path::new("/tmp/plugboard/tracked-conversations.json"));
    }

    #[test]
    fn maybe_track_publish_records_request_topics_only() {
        let temp = tempfile::tempdir().unwrap();
        let state = temp.path().join("tracked.json");
        let message = Message {
            id: "m1".into(),
            topic: "ollama.request".into(),
            body: "body".into(),
            created_at: "2026-03-20T00:00:00Z".into(),
            parent_id: None,
            conversation_id: "c1".into(),
            producer: None,
            metadata_json: None,
        };

        maybe_track_publish(&message, &state).unwrap();
        let tracked = load_tracked_conversations(&state).unwrap();
        assert_eq!(tracked.len(), 1);
        assert_eq!(tracked[0].success_topic, "ollama.done");
        assert_eq!(tracked[0].failure_topic, "ollama.failed");
        assert!(!tracked[0].notified);
    }

    #[test]
    fn mark_notified_updates_local_state() {
        let temp = tempfile::tempdir().unwrap();
        let state = temp.path().join("tracked.json");
        let message = Message {
            id: "m1".into(),
            topic: "ollama.request".into(),
            body: "body".into(),
            created_at: "2026-03-20T00:00:00Z".into(),
            parent_id: None,
            conversation_id: "c1".into(),
            producer: None,
            metadata_json: None,
        };

        maybe_track_publish(&message, &state).unwrap();
        mark_notified(&state, "c1").unwrap();
        let tracked = load_tracked_conversations(&state).unwrap();
        assert!(tracked[0].notified);
    }

    use std::path::Path;
}
