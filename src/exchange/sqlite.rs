use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};

use crate::domain::{Claim, ClaimStatus, Message, NewMessage};
use crate::error::{PlugboardError, Result};
use crate::exchange::Exchange;
use crate::notifier::{Notifier, SqliteFileNotifier};
use crate::util::id::new_id;
use crate::util::time::{add_seconds, format_timestamp, now_timestamp, now_utc};

const SCHEMA: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/schema.sql"));

pub struct SqliteExchange {
    connection: RefCell<Connection>,
    database_path: Option<PathBuf>,
}

impl SqliteExchange {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let connection = Connection::open(path)?;
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;

        Ok(Self {
            connection: RefCell::new(connection),
            database_path: Some(path.to_path_buf()),
        })
    }

    pub fn open_memory() -> Result<Self> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;

        Ok(Self {
            connection: RefCell::new(connection),
            database_path: None,
        })
    }

    fn notifier(&self) -> Option<SqliteFileNotifier> {
        self.database_path
            .as_ref()
            .map(|path| SqliteFileNotifier::new(path.clone()))
    }

    fn load_message(connection: &Connection, message_id: &str) -> Result<Option<Message>> {
        Ok(connection
            .query_row(
                "SELECT id, topic, body, created_at, parent_id, conversation_id, producer, metadata_json
                 FROM messages
                 WHERE id = ?1",
                params![message_id],
                map_message,
            )
            .optional()?)
    }

    fn load_claim(connection: &Connection, claim_id: &str) -> Result<Option<Claim>> {
        Ok(connection
            .query_row(
                "SELECT id, message_id, worker_group, worker_instance_id, claimed_at, lease_until, status, completed_at
                 FROM claims
                 WHERE id = ?1",
                params![claim_id],
                map_claim,
            )
            .optional()?)
    }

    fn ensure_claim_columns(connection: &Connection) -> Result<()> {
        let mut statement = connection.prepare("PRAGMA table_info(claims)")?;
        let columns = statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        if !columns.iter().any(|column| column == "worker_group") {
            connection.execute(
                "ALTER TABLE claims ADD COLUMN worker_group TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }

        if !columns.iter().any(|column| column == "worker_instance_id") {
            connection.execute(
                "ALTER TABLE claims ADD COLUMN worker_instance_id TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }

        connection.execute(
            "UPDATE claims
             SET worker_group = runner_name
             WHERE worker_group = ''",
            [],
        )?;
        connection.execute(
            "UPDATE claims
             SET worker_instance_id = id
             WHERE worker_instance_id = ''",
            [],
        )?;

        Ok(())
    }

    fn transition_claim(&self, claim_id: &str, next_status: ClaimStatus) -> Result<Claim> {
        let mut connection = self.connection.borrow_mut();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let completed_at = now_timestamp()?;
        let updated = transaction.execute(
            "UPDATE claims
             SET status = ?1, completed_at = ?2
             WHERE id = ?3 AND status = 'active'",
            params![next_status.as_str(), completed_at, claim_id],
        )?;

        if updated != 1 {
            let existing = Self::load_claim(&transaction, claim_id)?;
            return match existing {
                Some(_) => Err(PlugboardError::InvalidClaimTransition {
                    claim_id: claim_id.to_string(),
                }),
                None => Err(PlugboardError::NotFound(format!("claim {claim_id}"))),
            };
        }

        let claim = Self::load_claim(&transaction, claim_id)?
            .ok_or_else(|| PlugboardError::NotFound(format!("claim {claim_id}")))?;
        transaction.commit()?;
        Ok(claim)
    }

    fn claim_next_inner(
        &self,
        topic: &str,
        worker_group: &str,
        worker_instance_id: &str,
        lease_seconds: i64,
    ) -> Result<Option<(Message, Claim)>> {
        let mut connection = self.connection.borrow_mut();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let claimed_at_time = now_utc();
        let claimed_at = format_timestamp(claimed_at_time)?;

        transaction.execute(
            "DELETE FROM claims
             WHERE status = 'active' AND lease_until <= ?1",
            params![claimed_at],
        )?;

        let candidate = transaction
            .query_row(
                "SELECT id, topic, body, created_at, parent_id, conversation_id, producer, metadata_json
                 FROM messages
                 WHERE topic = ?1
                 AND NOT EXISTS (
                     SELECT 1
                     FROM claims
                     WHERE claims.message_id = messages.id
                 )
                 ORDER BY created_at ASC, id ASC
                 LIMIT 1",
                params![topic],
                map_message,
            )
            .optional()?;

        let Some(message) = candidate else {
            transaction.commit()?;
            return Ok(None);
        };

        let claim_id = new_id();
        let lease_until = format_timestamp(add_seconds(claimed_at_time, lease_seconds))?;

        transaction.execute(
            "INSERT INTO claims (
                 id,
                 message_id,
                 runner_name,
                 worker_group,
                 worker_instance_id,
                 claimed_at,
                 lease_until,
                 status,
                 completed_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active', NULL)",
            params![
                claim_id,
                message.id,
                worker_group,
                worker_group,
                worker_instance_id,
                claimed_at,
                lease_until,
            ],
        )?;

        let claim = Self::load_claim(&transaction, &claim_id)?
            .ok_or_else(|| PlugboardError::NotFound(format!("claim {claim_id}")))?;
        transaction.commit()?;
        Ok(Some((message, claim)))
    }
}

impl Exchange for SqliteExchange {
    fn init(&self) -> Result<()> {
        let connection = self.connection.borrow_mut();
        connection.execute_batch(SCHEMA)?;
        Self::ensure_claim_columns(&connection)?;
        Ok(())
    }

    fn publish(&self, message: NewMessage) -> Result<Message> {
        let mut connection = self.connection.borrow_mut();
        let transaction = connection.transaction()?;
        let parent = if let Some(parent_id) = message.parent_id.as_deref() {
            let parent = Self::load_message(&transaction, parent_id)?;
            if parent.is_none() {
                return Err(PlugboardError::NotFound(format!("message {parent_id}")));
            }
            parent
        } else {
            None
        };

        let id = new_id();
        let created_at = now_timestamp()?;
        let conversation_id = message.resolved_conversation_id(&id, parent.as_ref());

        transaction.execute(
            "INSERT INTO messages (id, topic, body, created_at, parent_id, conversation_id, producer, metadata_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                message.topic,
                message.body,
                created_at,
                message.parent_id,
                conversation_id,
                message.producer,
                message.metadata_json,
            ],
        )?;

        let stored = Self::load_message(&transaction, &id)?
            .ok_or_else(|| PlugboardError::NotFound(format!("message {id}")))?;
        transaction.commit()?;
        Ok(stored)
    }

    fn read_by_topic(&self, topic: &str) -> Result<Vec<Message>> {
        let connection = self.connection.borrow();
        let mut statement = connection.prepare(
            "SELECT id, topic, body, created_at, parent_id, conversation_id, producer, metadata_json
             FROM messages
             WHERE topic = ?1
             ORDER BY created_at ASC, id ASC",
        )?;

        let rows = statement.query_map(params![topic], map_message)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn read_by_conversation(&self, conversation_id: &str) -> Result<Vec<Message>> {
        let connection = self.connection.borrow();
        let mut statement = connection.prepare(
            "SELECT id, topic, body, created_at, parent_id, conversation_id, producer, metadata_json
             FROM messages
             WHERE conversation_id = ?1
             ORDER BY created_at ASC, id ASC",
        )?;

        let rows = statement.query_map(params![conversation_id], map_message)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn list_messages(&self) -> Result<Vec<Message>> {
        let connection = self.connection.borrow();
        let mut statement = connection.prepare(
            "SELECT id, topic, body, created_at, parent_id, conversation_id, producer, metadata_json
             FROM messages
             ORDER BY created_at ASC, id ASC",
        )?;

        let rows = statement.query_map([], map_message)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn get_message(&self, message_id: &str) -> Result<Option<Message>> {
        Self::load_message(&self.connection.borrow(), message_id)
    }

    fn claims_for_message(&self, message_id: &str) -> Result<Vec<Claim>> {
        let connection = self.connection.borrow();
        let mut statement = connection.prepare(
            "SELECT id, message_id, worker_group, worker_instance_id, claimed_at, lease_until, status, completed_at
             FROM claims
             WHERE message_id = ?1
             ORDER BY claimed_at ASC, id ASC",
        )?;

        let rows = statement.query_map(params![message_id], map_claim)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn claim_next(
        &self,
        topic: &str,
        worker_group: &str,
        worker_instance_id: &str,
        lease_seconds: i64,
    ) -> Result<Option<(Message, Claim)>> {
        self.claim_next_inner(topic, worker_group, worker_instance_id, lease_seconds)
    }

    fn claim_next_blocking(
        &self,
        topic: &str,
        worker_group: &str,
        worker_instance_id: &str,
        lease_seconds: i64,
        idle_sleep: Duration,
    ) -> Result<(Message, Claim)> {
        loop {
            if let Some(ticket) = self.prepare_wait_for_change()? {
                if let Some(claimed) =
                    self.claim_next_inner(topic, worker_group, worker_instance_id, lease_seconds)?
                {
                    return Ok(claimed);
                }
                ticket.wait(Some(idle_sleep))?;
                continue;
            }

            if let Some(claimed) =
                self.claim_next_inner(topic, worker_group, worker_instance_id, lease_seconds)?
            {
                return Ok(claimed);
            }
            std::thread::sleep(idle_sleep);
        }
    }

    fn prepare_wait_for_change(&self) -> Result<Option<Box<dyn crate::notifier::WaitTicket>>> {
        let Some(notifier) = self.notifier() else {
            return Ok(None);
        };

        notifier.prepare_wait().map(Some)
    }

    fn wait_for_change(&self, timeout: Option<Duration>) -> Result<bool> {
        let Some(ticket) = self.prepare_wait_for_change()? else {
            if let Some(timeout) = timeout {
                std::thread::sleep(timeout);
            }
            return Ok(false);
        };

        ticket.wait(timeout)
    }

    fn complete_claim(&self, claim_id: &str) -> Result<Claim> {
        self.transition_claim(claim_id, ClaimStatus::Completed)
    }

    fn fail_claim(&self, claim_id: &str) -> Result<Claim> {
        self.transition_claim(claim_id, ClaimStatus::Failed)
    }

    fn timeout_claim(&self, claim_id: &str) -> Result<Claim> {
        self.transition_claim(claim_id, ClaimStatus::TimedOut)
    }
}

fn map_message(row: &Row<'_>) -> rusqlite::Result<Message> {
    Ok(Message {
        id: row.get(0)?,
        topic: row.get(1)?,
        body: row.get(2)?,
        created_at: row.get(3)?,
        parent_id: row.get(4)?,
        conversation_id: row.get(5)?,
        producer: row.get(6)?,
        metadata_json: row.get(7)?,
    })
}

fn map_claim(row: &Row<'_>) -> rusqlite::Result<Claim> {
    let status: String = row.get(6)?;

    Ok(Claim {
        id: row.get(0)?,
        message_id: row.get(1)?,
        worker_group: row.get(2)?,
        worker_instance_id: row.get(3)?,
        claimed_at: row.get(4)?,
        lease_until: row.get(5)?,
        status: ClaimStatus::parse(&status).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                6,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        completed_at: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::SqliteExchange;
    use crate::domain::ClaimStatus;
    use crate::domain::NewMessage;
    use crate::exchange::Exchange;
    use crate::util::time::{format_timestamp, now_utc};
    use rusqlite::params;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn initializes_schema() {
        let exchange = SqliteExchange::open_memory().unwrap();

        exchange.init().unwrap();

        let messages = exchange.list_messages().unwrap();
        assert!(messages.is_empty());
    }

    #[test]
    fn publish_reads_by_topic_and_conversation() {
        let exchange = SqliteExchange::open_memory().unwrap();
        exchange.init().unwrap();

        let root = exchange
            .publish(NewMessage::new("code.generate", "generate"))
            .unwrap();
        let follow_up = exchange
            .publish(NewMessage {
                topic: "code.generated".into(),
                body: "done".into(),
                parent_id: Some(root.id.clone()),
                conversation_id: None,
                producer: Some("runner".into()),
                metadata_json: None,
            })
            .unwrap();

        assert_eq!(root.conversation_id, root.id);
        assert_eq!(follow_up.conversation_id, root.conversation_id);
        assert_eq!(exchange.read_by_topic("code.generate").unwrap().len(), 1);
        assert_eq!(
            exchange
                .read_by_conversation(&root.conversation_id)
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn claim_is_atomic_for_active_messages() {
        let exchange = SqliteExchange::open_memory().unwrap();
        exchange.init().unwrap();
        let message = exchange
            .publish(NewMessage::new("code.generate", "generate"))
            .unwrap();

        let first = exchange
            .claim_next("code.generate", "runner-group", "instance-1", 60)
            .unwrap()
            .unwrap();
        let second = exchange
            .claim_next("code.generate", "runner-group", "instance-2", 60)
            .unwrap();

        assert_eq!(first.0.id, message.id);
        assert!(second.is_none());
    }

    #[test]
    fn multiple_workers_in_same_group_can_claim_different_messages() {
        let exchange = SqliteExchange::open_memory().unwrap();
        exchange.init().unwrap();
        exchange
            .publish(NewMessage::new("code.generate", "first"))
            .unwrap();
        exchange
            .publish(NewMessage::new("code.generate", "second"))
            .unwrap();

        let first = exchange
            .claim_next("code.generate", "ollama-review", "instance-1", 60)
            .unwrap()
            .unwrap();
        let second = exchange
            .claim_next("code.generate", "ollama-review", "instance-2", 60)
            .unwrap()
            .unwrap();

        assert_ne!(first.0.id, second.0.id);
        assert_eq!(first.1.worker_group, "ollama-review");
        assert_eq!(second.1.worker_group, "ollama-review");
        assert_eq!(first.1.worker_instance_id, "instance-1");
        assert_eq!(second.1.worker_instance_id, "instance-2");
    }

    #[test]
    fn expired_active_claims_are_deleted_during_claim_and_do_not_block() {
        let exchange = SqliteExchange::open_memory().unwrap();
        exchange.init().unwrap();
        let message = exchange
            .publish(NewMessage::new("code.generate", "generate"))
            .unwrap();

        let expired_lease = format_timestamp(now_utc() - Duration::from_secs(5)).unwrap();
        exchange
            .connection
            .borrow_mut()
            .execute(
                "INSERT INTO claims (
                     id,
                     message_id,
                     runner_name,
                     worker_group,
                     worker_instance_id,
                     claimed_at,
                     lease_until,
                     status,
                     completed_at
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active', NULL)",
                params![
                    "stale-claim",
                    message.id,
                    "ollama-review",
                    "ollama-review",
                    "instance-old",
                    expired_lease,
                    expired_lease,
                ],
            )
            .unwrap();

        let (_, replacement) = exchange
            .claim_next("code.generate", "ollama-review", "instance-new", 60)
            .unwrap()
            .unwrap();

        let claims = exchange.claims_for_message(&message.id).unwrap();
        assert_eq!(claims.len(), 1);
        assert_eq!(replacement.worker_instance_id, "instance-new");
        assert_eq!(replacement.status, ClaimStatus::Active);
    }

    #[test]
    fn live_claim_from_another_instance_blocks_double_claim() {
        let exchange = SqliteExchange::open_memory().unwrap();
        exchange.init().unwrap();
        exchange
            .publish(NewMessage::new("code.generate", "generate"))
            .unwrap();

        let first = exchange
            .claim_next("code.generate", "ollama-review", "instance-1", 60)
            .unwrap()
            .unwrap();
        let second = exchange
            .claim_next("code.generate", "ollama-review", "instance-2", 60)
            .unwrap();

        assert_eq!(first.1.status, ClaimStatus::Active);
        assert!(second.is_none());
    }

    #[test]
    fn claim_transitions_record_completion_timestamp() {
        let exchange = SqliteExchange::open_memory().unwrap();
        exchange.init().unwrap();
        exchange
            .publish(NewMessage::new("code.generate", "generate"))
            .unwrap();

        let (_, claim) = exchange
            .claim_next("code.generate", "runner-group", "instance-1", 60)
            .unwrap()
            .unwrap();

        let completed = exchange.complete_claim(&claim.id).unwrap();
        assert_eq!(completed.status, ClaimStatus::Completed);
        assert!(completed.completed_at.is_some());

        let reclaimed = exchange
            .claim_next("code.generate", "runner-group", "instance-2", 60)
            .unwrap();
        assert!(reclaimed.is_none());

        let repeated = exchange.complete_claim(&claim.id);
        assert!(repeated.is_err());
    }

    #[test]
    fn fail_and_timeout_transitions_record_completion_timestamp() {
        let exchange = SqliteExchange::open_memory().unwrap();
        exchange.init().unwrap();

        exchange
            .publish(NewMessage::new("code.generate", "generate"))
            .unwrap();
        exchange
            .publish(NewMessage::new("code.generate", "generate again"))
            .unwrap();

        let (_, failed_claim) = exchange
            .claim_next("code.generate", "runner-group", "instance-1", 60)
            .unwrap()
            .unwrap();
        let (_, timed_out_claim) = exchange
            .claim_next("code.generate", "runner-group", "instance-2", 60)
            .unwrap()
            .unwrap();

        let failed = exchange.fail_claim(&failed_claim.id).unwrap();
        assert_eq!(failed.status, ClaimStatus::Failed);
        assert!(failed.completed_at.is_some());

        let timed_out = exchange.timeout_claim(&timed_out_claim.id).unwrap();
        assert_eq!(timed_out.status, ClaimStatus::TimedOut);
        assert!(timed_out.completed_at.is_some());
    }

    #[test]
    fn publish_with_missing_parent_fails() {
        let exchange = SqliteExchange::open_memory().unwrap();
        exchange.init().unwrap();

        let result = exchange.publish(NewMessage {
            topic: "code.generated".into(),
            body: "done".into(),
            parent_id: Some("missing".into()),
            conversation_id: None,
            producer: None,
            metadata_json: None,
        });

        assert!(result.is_err());
    }

    #[test]
    fn claim_next_blocking_wakes_after_publish() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("plugboard.db");
        let exchange = SqliteExchange::open(&database).unwrap();
        exchange.init().unwrap();

        thread::scope(|scope| {
            let database = database.clone();
            let handle = scope.spawn(move || {
                let blocking_exchange = SqliteExchange::open(&database).unwrap();
                blocking_exchange.init().unwrap();
                blocking_exchange
                    .claim_next_blocking(
                        "review.request",
                        "review-worker",
                        "instance-1",
                        5,
                        Duration::from_millis(10),
                    )
                    .unwrap()
            });

            thread::sleep(Duration::from_millis(100));
            exchange
                .publish(NewMessage::new("review.request", "hello"))
                .unwrap();

            let (message, claim) = handle.join().unwrap();
            assert_eq!(message.topic, "review.request");
            assert_eq!(message.body, "hello");
            assert_eq!(claim.status, ClaimStatus::Active);
            assert_eq!(claim.worker_group, "review-worker");
            assert_eq!(claim.worker_instance_id, "instance-1");
        });
    }
}
