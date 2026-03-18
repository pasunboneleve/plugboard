pub mod sqlite;

use std::time::Duration;

use crate::domain::{Claim, Message, NewMessage};
use crate::error::Result;

pub trait Exchange {
    fn init(&self) -> Result<()>;
    fn publish(&self, message: NewMessage) -> Result<Message>;
    fn read_by_topic(&self, topic: &str) -> Result<Vec<Message>>;
    fn read_by_conversation(&self, conversation_id: &str) -> Result<Vec<Message>>;
    fn list_messages(&self) -> Result<Vec<Message>>;
    fn get_message(&self, message_id: &str) -> Result<Option<Message>>;
    fn claims_for_message(&self, message_id: &str) -> Result<Vec<Claim>>;
    fn claim_next(
        &self,
        topic: &str,
        runner_name: &str,
        lease_seconds: i64,
    ) -> Result<Option<(Message, Claim)>>;
    fn claim_next_blocking(
        &self,
        topic: &str,
        runner_name: &str,
        lease_seconds: i64,
    ) -> Result<(Message, Claim)>;
    fn wait_for_change(&self, timeout: Option<Duration>) -> Result<bool>;
    fn complete_claim(&self, claim_id: &str) -> Result<Claim>;
    fn fail_claim(&self, claim_id: &str) -> Result<Claim>;
    fn timeout_claim(&self, claim_id: &str) -> Result<Claim>;
}
