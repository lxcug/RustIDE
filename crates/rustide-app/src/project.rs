use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::Sender,
    Arc,
};
use std::time::{Duration, Instant};

use rustide_project::{build_tree, ProjectEvent, TreeNode};

#[derive(Debug, Clone)]
pub enum ProjectMessage {
    TreeUpdated(TreeNode),
    Error(String),
}

pub struct ProjectWorker {
    stop: Arc<AtomicBool>,
}

impl ProjectWorker {
    pub fn start(root: PathBuf, tx: Sender<ProjectMessage>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();
        std::thread::spawn(move || run_worker(root, tx, stop_thread));
        Self { stop }
    }
}

impl Drop for ProjectWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn run_worker(root: PathBuf, tx: Sender<ProjectMessage>, stop: Arc<AtomicBool>) {
    if !root.exists() || !root.is_dir() {
        let _ = tx.send(ProjectMessage::Error(format!(
            "Project root does not exist: {}",
            root.display()
        )));
        return;
    }

    let _ = tx.send(ProjectMessage::TreeUpdated(build_tree(&root)));

    let (raw_tx, raw_rx) = std::sync::mpsc::channel::<ProjectEvent>();
    let (debounced_tx, debounced_rx) = std::sync::mpsc::channel::<ProjectEvent>();

    let _watcher = match rustide_project::ProjectWatcher::start(&root, raw_tx) {
        Ok(w) => w,
        Err(e) => {
            let _ = tx.send(ProjectMessage::Error(e));
            return;
        }
    };

    std::thread::spawn(move || {
        rustide_project::debounce_events(raw_rx, debounced_tx, Duration::from_millis(250))
    });

    let mut last_refresh = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        match debounced_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(ProjectEvent::Changed) => {
                if last_refresh.elapsed() < Duration::from_millis(100) {
                    continue;
                }
                last_refresh = Instant::now();
                let tree = build_tree(&root);
                let _ = tx.send(ProjectMessage::TreeUpdated(tree));
            }
            Ok(ProjectEvent::Error(e)) => {
                let _ = tx.send(ProjectMessage::Error(e));
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

pub struct SearchWorker {
    stop: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
pub struct SearchMatch {
    pub path: PathBuf,
    pub line_index: usize,
    pub column_chars: usize,
    pub preview: String,
}

#[derive(Debug, Clone)]
pub enum SearchMessage {
    Started(u64),
    Match(u64, SearchMatch),
    Finished(u64),
    Error(u64, String),
}

impl SearchWorker {
    pub fn start(
        root: PathBuf,
        query: String,
        case_sensitive: bool,
        encoding_hint: rustide_editor::TextEncodingHint,
        request_id: u64,
        tx: Sender<SearchMessage>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();
        std::thread::spawn(move || {
            run_search(
                root,
                query,
                case_sensitive,
                encoding_hint,
                request_id,
                tx,
                stop_thread,
            )
        });
        Self { stop }
    }

    pub fn cancel(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn run_search(
    root: PathBuf,
    query: String,
    case_sensitive: bool,
    encoding_hint: rustide_editor::TextEncodingHint,
    request_id: u64,
    tx: Sender<SearchMessage>,
    stop: Arc<AtomicBool>,
) {
    if !root.exists() || !root.is_dir() {
        let _ = tx.send(SearchMessage::Error(
            request_id,
            format!("Project root does not exist: {}", root.display()),
        ));
        let _ = tx.send(SearchMessage::Finished(request_id));
        return;
    }
    if query.trim().is_empty() {
        let _ = tx.send(SearchMessage::Finished(request_id));
        return;
    }
    let needle = if case_sensitive {
        query
    } else {
        query.to_ascii_lowercase()
    };

    let _ = tx.send(SearchMessage::Started(request_id));

    for entry in ignore::WalkBuilder::new(&root)
        .hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(true)
        .follow_links(false)
        .build()
        .flatten()
    {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let path = entry.path();
        if entry
            .file_type()
            .map(|t| t.is_dir())
            .unwrap_or_else(|| path.is_dir())
        {
            continue;
        }

        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let (content, _encoding) = rustide_editor::decode_bytes(&bytes, encoding_hint);
        for (line_index, line) in content.lines().enumerate() {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            if !case_sensitive {
                let lower = line.to_ascii_lowercase();
                if let Some(byte_idx) = lower.find(&needle) {
                    let column_chars = line[..byte_idx].chars().count();
                    let preview = line.chars().take(200).collect();
                    let _ = tx.send(SearchMessage::Match(
                        request_id,
                        SearchMatch {
                            path: path.to_path_buf(),
                            line_index,
                            column_chars,
                            preview,
                        },
                    ));
                }
                continue;
            }
            if let Some(byte_idx) = line.find(&needle) {
                let column_chars = line[..byte_idx].chars().count();
                let preview = line.chars().take(200).collect();
                let _ = tx.send(SearchMessage::Match(
                    request_id,
                    SearchMatch {
                        path: path.to_path_buf(),
                        line_index,
                        column_chars,
                        preview,
                    },
                ));
            }
        }
    }

    let _ = tx.send(SearchMessage::Finished(request_id));
}
