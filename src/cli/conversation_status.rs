use crate::domain::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationState {
    Success,
    Failure,
}

pub struct TerminalReply<'a> {
    pub state: ConversationState,
    pub message: &'a Message,
}

pub fn find_terminal_reply<'a>(
    messages: &'a [Message],
    success_topic: &str,
    failure_topic: &str,
) -> Option<TerminalReply<'a>> {
    messages
        .iter()
        .rev()
        .find(|message| message.topic == success_topic || message.topic == failure_topic)
        .map(|message| TerminalReply {
            state: if message.topic == success_topic {
                ConversationState::Success
            } else {
                ConversationState::Failure
            },
            message,
        })
}

pub fn state_name(state: ConversationState) -> &'static str {
    match state {
        ConversationState::Success => "success",
        ConversationState::Failure => "failure",
    }
}
