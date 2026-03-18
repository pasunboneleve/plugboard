use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::Instant;
use std::time::Duration;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};

use crate::error::Result;

pub trait WaitTicket {
    fn wait(self: Box<Self>, timeout: Option<Duration>) -> Result<bool>;
}

pub trait Notifier {
    fn prepare_wait(&self) -> Result<Box<dyn WaitTicket>>;
}

#[derive(Debug, Clone)]
pub struct SqliteFileNotifier {
    database_path: PathBuf,
}

impl SqliteFileNotifier {
    pub fn new(database_path: impl Into<PathBuf>) -> Self {
        Self {
            database_path: database_path.into(),
        }
    }

    pub fn emit(&self) -> Result<()> {
        let wake_path = wake_marker_path(&self.database_path);
        if let Some(parent) = wake_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(wake_path)?;
        file.write_all(b".")?;
        file.flush()?;
        Ok(())
    }
}

impl Notifier for SqliteFileNotifier {
    fn prepare_wait(&self) -> Result<Box<dyn WaitTicket>> {
        let watched_dir = self
            .database_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let watched_paths = related_sqlite_paths(&self.database_path);
        let (sender, receiver) = mpsc::channel();
        let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |event| {
            let _ = sender.send(event);
        })?;
        watcher.watch(&watched_dir, RecursiveMode::NonRecursive)?;

        Ok(Box::new(FileWaitTicket {
            _watcher: watcher,
            receiver,
            watched_paths,
        }))
    }
}

struct FileWaitTicket {
    _watcher: RecommendedWatcher,
    receiver: Receiver<notify::Result<Event>>,
    watched_paths: Vec<PathBuf>,
}

impl WaitTicket for FileWaitTicket {
    fn wait(self: Box<Self>, timeout: Option<Duration>) -> Result<bool> {
        let ticket = *self;
        let deadline = timeout.map(|timeout| Instant::now() + timeout);
        loop {
            let event = match timeout {
                Some(_) => {
                    let Some(remaining) = deadline.and_then(|deadline| {
                        remaining_until(deadline, Instant::now())
                    }) else {
                        return Ok(false);
                    };
                    match ticket.receiver.recv_timeout(remaining) {
                    Ok(event) => event?,
                    Err(RecvTimeoutError::Timeout) => return Ok(false),
                    Err(RecvTimeoutError::Disconnected) => return Ok(false),
                }
                }
                None => match ticket.receiver.recv() {
                    Ok(event) => event?,
                    Err(_) => return Ok(false),
                },
            };

            if event.paths.iter().any(|path| matches_related_path(path, &ticket.watched_paths)) {
                return Ok(true);
            }
        }
    }
}

fn related_sqlite_paths(database_path: &Path) -> Vec<PathBuf> {
    let mut paths = vec![database_path.to_path_buf()];

    if let Some(file_name) = database_path.file_name().and_then(|name| name.to_str()) {
        paths.push(database_path.with_file_name(format!("{file_name}-wal")));
        paths.push(database_path.with_file_name(format!("{file_name}-shm")));
        paths.push(wake_marker_path(database_path));
    }

    paths
}

fn wake_marker_path(database_path: &Path) -> PathBuf {
    let file_name = database_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("plugboard.db");
    database_path.with_file_name(format!("{file_name}.wake"))
}

fn matches_related_path(candidate: &Path, watched_paths: &[PathBuf]) -> bool {
    watched_paths.iter().any(|path| candidate == path)
}

fn remaining_until(deadline: Instant, now: Instant) -> Option<Duration> {
    deadline.checked_duration_since(now)
}

#[cfg(test)]
mod tests {
    use super::{matches_related_path, related_sqlite_paths, remaining_until, wake_marker_path};
    use std::path::Path;
    use std::time::{Duration, Instant};

    #[test]
    fn expands_sqlite_related_paths() {
        let paths = related_sqlite_paths(Path::new("/tmp/plugboard.db"));
        assert!(paths.contains(&"/tmp/plugboard.db".into()));
        assert!(paths.contains(&"/tmp/plugboard.db-wal".into()));
        assert!(paths.contains(&"/tmp/plugboard.db-shm".into()));
        assert!(paths.contains(&"/tmp/plugboard.db.wake".into()));
    }

    #[test]
    fn matches_database_and_wal_paths() {
        let watched = related_sqlite_paths(Path::new("/tmp/plugboard.db"));
        assert!(matches_related_path(Path::new("/tmp/plugboard.db"), &watched));
        assert!(matches_related_path(
            Path::new("/tmp/plugboard.db-wal"),
            &watched
        ));
        assert!(matches_related_path(
            Path::new("/tmp/plugboard.db.wake"),
            &watched
        ));
        assert!(!matches_related_path(Path::new("/tmp/other.db"), &watched));
    }

    #[test]
    fn derives_wake_marker_path() {
        assert_eq!(
            wake_marker_path(Path::new("/tmp/plugboard.db")),
            Path::new("/tmp/plugboard.db.wake")
        );
    }

    #[test]
    fn remaining_until_respects_deadline() {
        let start = Instant::now();
        let deadline = start + Duration::from_millis(50);
        assert!(remaining_until(deadline, start).unwrap() <= Duration::from_millis(50));
        assert_eq!(remaining_until(deadline, deadline), Some(Duration::ZERO));
        assert_eq!(remaining_until(deadline, deadline + Duration::from_millis(1)), None);
    }
}
