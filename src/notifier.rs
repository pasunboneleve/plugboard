use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
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
        loop {
            let event = match timeout {
                Some(timeout) => match ticket.receiver.recv_timeout(timeout) {
                    Ok(event) => event?,
                    Err(RecvTimeoutError::Timeout) => return Ok(false),
                    Err(RecvTimeoutError::Disconnected) => return Ok(false),
                },
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
    }

    paths
}

fn matches_related_path(candidate: &Path, watched_paths: &[PathBuf]) -> bool {
    watched_paths.iter().any(|path| candidate == path)
}

#[cfg(test)]
mod tests {
    use super::{matches_related_path, related_sqlite_paths};
    use std::path::Path;

    #[test]
    fn expands_sqlite_related_paths() {
        let paths = related_sqlite_paths(Path::new("/tmp/plugboard.db"));
        assert!(paths.contains(&"/tmp/plugboard.db".into()));
        assert!(paths.contains(&"/tmp/plugboard.db-wal".into()));
        assert!(paths.contains(&"/tmp/plugboard.db-shm".into()));
    }

    #[test]
    fn matches_database_and_wal_paths() {
        let watched = related_sqlite_paths(Path::new("/tmp/plugboard.db"));
        assert!(matches_related_path(Path::new("/tmp/plugboard.db"), &watched));
        assert!(matches_related_path(
            Path::new("/tmp/plugboard.db-wal"),
            &watched
        ));
        assert!(!matches_related_path(Path::new("/tmp/other.db"), &watched));
    }
}
