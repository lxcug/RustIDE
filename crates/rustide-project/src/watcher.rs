use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

#[derive(Debug, Clone)]
pub enum ProjectEvent {
    Changed,
    Error(String),
}

pub struct ProjectWatcher {
    // Keep the watcher alive for the lifetime of this struct.
    #[allow(dead_code)]
    watcher: RecommendedWatcher,
    root: PathBuf,
}

impl ProjectWatcher {
    pub fn start(root: &Path, tx: Sender<ProjectEvent>) -> Result<Self, String> {
        let root = root.to_path_buf();
        let mut watcher = notify::recommended_watcher(
            move |res: Result<notify::Event, notify::Error>| match res {
                Ok(_event) => {
                    let _ = tx.send(ProjectEvent::Changed);
                }
                Err(e) => {
                    let _ = tx.send(ProjectEvent::Error(e.to_string()));
                }
            },
        )
        .map_err(|e| e.to_string())?;

        watcher
            .watch(&root, RecursiveMode::Recursive)
            .map_err(|e| e.to_string())?;

        Ok(Self { watcher, root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

pub fn debounce_events(rx: Receiver<ProjectEvent>, tx: Sender<ProjectEvent>, delay: Duration) {
    // Coalesce watcher bursts into a single Changed event.
    let mut last_changed: Option<Instant> = None;
    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(ProjectEvent::Changed) => last_changed = Some(Instant::now()),
            Ok(ProjectEvent::Error(e)) => {
                let _ = tx.send(ProjectEvent::Error(e));
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }

        if let Some(t) = last_changed {
            if t.elapsed() >= delay {
                let _ = tx.send(ProjectEvent::Changed);
                last_changed = None;
            }
        }
    }
}
