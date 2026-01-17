#![cfg_attr(windows, windows_subsystem = "windows")]

use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};

use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use egui_dock::{DockArea, DockState, Style, TabViewer};
use rfd::FileDialog;
use rustide_syntax::{LanguageId, SyntaxState};
use serde::{Deserialize, Serialize};

mod config;
mod editor_view;
mod project;
mod theme;

#[derive(Debug, Clone)]
struct LoadRequest {
    tab_id: usize,
    path: PathBuf,
    encoding: rustide_editor::TextEncodingHint,
}

#[derive(Debug)]
enum LoadMessage {
    Loaded {
        tab_id: usize,
        result: Result<rustide_editor::Document, String>,
    },
}

#[derive(Debug)]
enum DialogMessage {
    FilePicked(Option<PathBuf>),
    FolderPicked(Option<PathBuf>),
}

struct SaveRequest {
    tab_id: usize,
    path: PathBuf,
    encoding: rustide_editor::TextEncoding,
    text: String,
    version: u64,
}

enum SaveMessage {
    Saved {
        tab_id: usize,
        version: u64,
    },
    Failed {
        tab_id: usize,
        version: u64,
        error: String,
    },
}

struct OpenDocument {
    doc: rustide_editor::Document,
    syntax: Option<SyntaxState>,
    markdown: Option<MarkdownState>,
    scroll_to_char: Option<usize>,
    nav_back: Vec<NavLocation>,
    nav_forward: Vec<NavLocation>,
    pending_jump: Option<(usize, usize)>, // (line_index, column_chars)
    pinned: bool,
    last_saved_version: u64,
    pending_save_version: Option<u64>,
    last_save_request: Option<Instant>,
}

struct MarkdownState {
    preview_enabled: bool,
    cache: CommonMarkCache,
    cached_text: String,
    cached_version: u64,
    preview_content_h: f32,
    preview_viewport_h: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NavLocation {
    cursor: usize,
}

impl OpenDocument {
    fn empty() -> Self {
        Self {
            doc: rustide_editor::Document::empty(),
            syntax: None,
            markdown: None,
            scroll_to_char: None,
            nav_back: Vec::new(),
            nav_forward: Vec::new(),
            pending_jump: None,
            pinned: false,
            last_saved_version: 0,
            pending_save_version: None,
            last_save_request: None,
        }
    }

    fn from_loaded(doc: rustide_editor::Document) -> Self {
        let language = LanguageId::from_path(doc.path.as_deref());
        let mut syntax = SyntaxState::new(language).ok();
        if let Some(s) = syntax.as_mut() {
            if let Err(err) = s.set_text(doc.editor.rope()) {
                tracing::warn!("Failed to initialize syntax: {err}");
                syntax = None;
            }
        }
        let markdown = if language == LanguageId::Markdown {
            Some(MarkdownState {
                preview_enabled: true,
                cache: CommonMarkCache::default(),
                cached_text: doc.editor.rope().to_string(),
                cached_version: doc.editor.version(),
                preview_content_h: 0.0,
                preview_viewport_h: 0.0,
            })
        } else {
            None
        };
        let last_saved_version = doc.editor.version();
        Self {
            doc,
            syntax,
            markdown,
            scroll_to_char: None,
            nav_back: Vec::new(),
            nav_forward: Vec::new(),
            pending_jump: None,
            pinned: false,
            last_saved_version,
            pending_save_version: None,
            last_save_request: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum DockTab {
    Editor,
    Project,
    Search,
}

impl DockTab {
    fn title(&self) -> &'static str {
        match self {
            Self::Editor => "Editor",
            Self::Project => "Project",
            Self::Search => "Search",
        }
    }
}

const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(2);

struct RustideApp {
    documents: Vec<OpenDocument>,
    active_doc: usize,
    status: String,
    load_tx: Sender<LoadRequest>,
    load_rx: Receiver<LoadMessage>,
    save_tx: Sender<SaveRequest>,
    save_rx: Receiver<SaveMessage>,
    dialog_tx: Sender<DialogMessage>,
    dialog_rx: Receiver<DialogMessage>,
    config: config::AppConfig,
    config_path: PathBuf,
    font_state: FontState,
    theme: theme::Theme,
    last_applied_ui: Option<(config::MonospaceFont, f32, theme::ThemeId, f32)>,
    dock_state: DockState<DockTab>,

    project_root: Option<PathBuf>,
    project_root_input: String,
    project_tree: Option<rustide_project::TreeNode>,
    project_status: String,
    project_worker: Option<project::ProjectWorker>,
    project_rx: Receiver<project::ProjectMessage>,
    project_tx: Sender<project::ProjectMessage>,

    search_query: String,
    search_case_sensitive: bool,
    search_request_focus: bool,
    search_results: Vec<project::SearchMatch>,
    search_status: String,
    search_worker: Option<project::SearchWorker>,
    search_rx: Receiver<project::SearchMessage>,
    search_tx: Sender<project::SearchMessage>,
    active_search_id: u64,

    find_open: bool,
    find_request_focus: bool,
    find_query: String,
    find_case_sensitive: bool,
    find_status: String,
}

impl RustideApp {
    fn new(
        initial_path: Option<PathBuf>,
        config: config::AppConfig,
        config_path: PathBuf,
        font_state: FontState,
    ) -> Self {
        let (load_tx, request_rx) = mpsc::channel::<LoadRequest>();
        let (result_tx, load_rx) = mpsc::channel::<LoadMessage>();
        let (save_tx, save_rx_req) = mpsc::channel::<SaveRequest>();
        let (save_result_tx, save_rx) = mpsc::channel::<SaveMessage>();
        let (dialog_tx, dialog_rx) = mpsc::channel::<DialogMessage>();
        let (project_tx, project_rx) = mpsc::channel::<project::ProjectMessage>();
        let (search_tx, search_rx) = mpsc::channel::<project::SearchMessage>();

        std::thread::spawn(move || {
            while let Ok(req) = request_rx.recv() {
                let loaded = rustide_editor::Document::load_with_encoding(&req.path, req.encoding)
                    .map_err(|e| e.to_string());
                let _ = result_tx.send(LoadMessage::Loaded {
                    tab_id: req.tab_id,
                    result: loaded,
                });
            }
        });

        std::thread::spawn(move || {
            while let Ok(req) = save_rx_req.recv() {
                let bytes = rustide_editor::encode_text(&req.text, req.encoding);
                let message = match std::fs::write(&req.path, bytes) {
                    Ok(()) => SaveMessage::Saved {
                        tab_id: req.tab_id,
                        version: req.version,
                    },
                    Err(err) => SaveMessage::Failed {
                        tab_id: req.tab_id,
                        version: req.version,
                        error: err.to_string(),
                    },
                };
                let _ = save_result_tx.send(message);
            }
        });

        let theme = theme::build_theme(config.ui.theme);
        let dock_state = load_or_default_dock_state(&config);
        let mut app = Self {
            documents: Vec::new(),
            active_doc: 0,
            status: "Drop a file into the window, or pass a file path as the first CLI argument."
                .to_string(),
            load_tx,
            load_rx,
            save_tx,
            save_rx,
            dialog_tx,
            dialog_rx,
            config,
            config_path,
            font_state,
            theme,
            last_applied_ui: None,
            dock_state,

            project_root: None,
            project_root_input: String::new(),
            project_tree: None,
            project_status: "Drop a folder to open a project.".to_string(),
            project_worker: None,
            project_rx,
            project_tx,

            search_query: String::new(),
            search_case_sensitive: false,
            search_request_focus: false,
            search_results: Vec::new(),
            search_status: String::new(),
            search_worker: None,
            search_rx,
            search_tx,
            active_search_id: 0,

            find_open: false,
            find_request_focus: false,
            find_query: String::new(),
            find_case_sensitive: false,
            find_status: String::new(),
        };

        if let Some(path) = initial_path {
            if path.is_dir() {
                app.set_project_root(path);
            } else {
                if let Some(parent) = path.parent().map(PathBuf::from) {
                    app.set_project_root(parent);
                }
                app.open_in_new_tab(path, None);
            }
        }

        app
    }

    fn active_document(&self) -> &OpenDocument {
        self.documents
            .get(self.active_doc)
            .or_else(|| self.documents.first())
            .expect("no open tabs")
    }

    fn active_document_mut(&mut self) -> &mut OpenDocument {
        if self.documents.is_empty() {
            panic!("no open tabs");
        }
        if self.active_doc >= self.documents.len() {
            self.active_doc = 0;
        }
        &mut self.documents[self.active_doc]
    }

    fn open_in_new_tab(&mut self, path: PathBuf, jump: Option<(usize, usize)>) {
        if let Some(existing) = self
            .documents
            .iter()
            .enumerate()
            .find(|(_idx, d)| d.doc.path.as_ref() == Some(&path))
            .map(|(idx, _)| idx)
        {
            self.active_doc = existing;
            if let Some(j) = jump {
                let doc = self.active_document_mut();
                doc.pending_jump = Some(j);
                // If already loaded, jump immediately; otherwise wait for load completion.
                if doc.doc.path.as_ref() == Some(&path) && doc.doc.editor.rope().len_chars() > 0 {
                    let tab_id = self.active_doc;
                    self.apply_pending_jump(tab_id);
                }
            }
            return;
        }

        let mut doc = OpenDocument::empty();
        doc.doc.path = Some(path.clone());
        doc.pending_jump = jump;
        self.documents.push(doc);
        self.active_doc = self.documents.len().saturating_sub(1);
        self.queue_load(self.active_doc, path);
    }

    fn open_file_dialog(&mut self) {
        let tx = self.dialog_tx.clone();
        let start_dir = self
            .documents
            .get(self.active_doc)
            .and_then(|d| d.doc.path.as_ref().and_then(|p| p.parent()).map(PathBuf::from))
            .or_else(|| self.project_root.clone());
        std::thread::spawn(move || {
            let mut dialog = FileDialog::new();
            if let Some(dir) = start_dir {
                dialog = dialog.set_directory(dir);
            }
            let picked = dialog.pick_file();
            let _ = tx.send(DialogMessage::FilePicked(picked));
        });
    }

    fn open_folder_dialog(&mut self) {
        let tx = self.dialog_tx.clone();
        let start_dir = self
            .project_root
            .clone()
            .or_else(|| std::env::current_dir().ok());
        std::thread::spawn(move || {
            let mut dialog = FileDialog::new();
            if let Some(dir) = start_dir {
                dialog = dialog.set_directory(dir);
            }
            let picked = dialog.pick_folder();
            let _ = tx.send(DialogMessage::FolderPicked(picked));
        });
    }

    fn queue_load(&mut self, tab_id: usize, path: PathBuf) {
        let req = LoadRequest {
            tab_id,
            path,
            encoding: self.config.file.encoding,
        };
        match self.load_tx.send(req) {
            Ok(()) => self.status = "Loading…".to_string(),
            Err(e) => self.status = format!("Failed to queue load: {e}"),
        }
    }

    fn poll_load_results(&mut self) {
        while let Ok(msg) = self.load_rx.try_recv() {
            match msg {
                LoadMessage::Loaded {
                    tab_id,
                    result: Ok(doc),
                } => {
                    self.status.clear();
                    if let Some(p) = doc.path.clone() {
                        self.config.startup.last_file = Some(p);
                    }
                    if let Some(slot) = self.documents.get_mut(tab_id) {
                        let pending_jump = slot.pending_jump.take();
                        *slot = OpenDocument::from_loaded(doc);
                        slot.pending_jump = pending_jump;
                        self.apply_pending_jump(tab_id);
                    }
                }
                LoadMessage::Loaded {
                    tab_id: _,
                    result: Err(err),
                } => {
                    self.status = format!("Load failed: {err}");
                }
            }
        }
    }

    fn poll_dialog_results(&mut self) {
        while let Ok(msg) = self.dialog_rx.try_recv() {
            match msg {
                DialogMessage::FilePicked(Some(path)) => {
                    self.open_in_new_tab(path, None);
                }
                DialogMessage::FolderPicked(Some(path)) => {
                    self.set_project_root(path);
                }
                DialogMessage::FilePicked(None) | DialogMessage::FolderPicked(None) => {}
            }
        }
    }

    fn poll_save_results(&mut self) {
        while let Ok(msg) = self.save_rx.try_recv() {
            match msg {
                SaveMessage::Saved { tab_id, version } => {
                    if let Some(doc) = self.documents.get_mut(tab_id) {
                        let path_label = doc
                            .doc
                            .path
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "<untitled>".to_string());
                        doc.pending_save_version = None;
                        doc.last_saved_version = version;
                        self.status = format!("Autosaved {path_label}");
                    }
                }
                SaveMessage::Failed {
                    tab_id,
                    version,
                    error,
                } => {
                    if let Some(doc) = self.documents.get_mut(tab_id) {
                        if doc.pending_save_version == Some(version) {
                            doc.pending_save_version = None;
                        }
                        let path_label = doc
                            .doc
                            .path
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "<untitled>".to_string());
                        self.status = format!("Autosave failed for {path_label}: {error}");
                    } else {
                        self.status = format!("Autosave failed: {error}");
                    }
                }
            }
        }
    }

    fn maybe_autosave_active_doc(&mut self) {
        if self.documents.is_empty() {
            return;
        }
        let now = Instant::now();
        let (path, encoding, version, text) = {
            let doc = self.active_document_mut();
            if doc.doc.path.is_none() {
                return;
            }
            if doc.pending_save_version.is_some() {
                return;
            }
            let version = doc.doc.editor.version();
            if version == doc.last_saved_version {
                return;
            }
            if doc
                .last_save_request
                .map(|instant| now.duration_since(instant) < AUTOSAVE_INTERVAL)
                .unwrap_or(false)
            {
                return;
            }
            let path = doc.doc.path.clone().unwrap();
            let encoding = doc.doc.encoding;
            let text = doc.doc.editor.rope().to_string();
            (path, encoding, version, text)
        };

        let request = SaveRequest {
            tab_id: self.active_doc,
            path: path.clone(),
            encoding,
            text,
            version,
        };

        if self.save_tx.send(request).is_ok() {
            let doc = self.active_document_mut();
            doc.pending_save_version = Some(version);
            doc.last_save_request = Some(now);
            self.status = format!("Autosaving {}", path.display());
        } else {
            self.status = "Autosave queue failed".to_string();
        }
    }

    fn poll_project(&mut self) {
        while let Ok(msg) = self.project_rx.try_recv() {
            match msg {
                project::ProjectMessage::TreeUpdated(tree) => {
                    self.project_tree = Some(tree);
                    self.project_status = "Ready".to_string();
                }
                project::ProjectMessage::Error(e) => {
                    self.project_status = format!("Project error: {e}");
                }
            }
        }

        while let Ok(msg) = self.search_rx.try_recv() {
            match msg {
                project::SearchMessage::Started(id) => {
                    if id != self.active_search_id {
                        continue;
                    }
                    self.search_results.clear();
                    self.search_status = "Searching…".to_string();
                }
                project::SearchMessage::Match(id, m) => {
                    if id == self.active_search_id {
                        self.search_results.push(m);
                    }
                }
                project::SearchMessage::Finished(id) => {
                    if id == self.active_search_id {
                        self.search_status = "Done".to_string();
                    }
                }
                project::SearchMessage::Error(id, e) => {
                    if id == self.active_search_id {
                        self.search_status = format!("Search error: {e}");
                    }
                }
            }
        }
    }

    fn apply_pending_jump(&mut self, tab_id: usize) {
        let Some(doc) = self.documents.get_mut(tab_id) else {
            return;
        };
        let Some((line_index, column_chars)) = doc.pending_jump.take() else {
            return;
        };
        let rope = doc.doc.editor.rope();
        let line = line_index.min(rope.len_lines().saturating_sub(1));
        let line_start = rope.line_to_char(line);
        let cursor = line_start + column_chars;
        doc.doc.editor.set_cursor(cursor, false);
        doc.scroll_to_char = Some(cursor);
    }

    fn current_location(&self) -> NavLocation {
        if self.documents.is_empty() {
            return NavLocation { cursor: 0 };
        }
        NavLocation {
            cursor: self.active_document().doc.editor.selection().cursor,
        }
    }

    fn record_nav_from(&mut self, from: NavLocation) {
        if self.documents.is_empty() {
            return;
        }
        let doc = self.active_document_mut();
        if doc.nav_back.last().copied() != Some(from) {
            doc.nav_back.push(from);
        }
        doc.nav_forward.clear();
    }

    fn navigate_back(&mut self) {
        if self.documents.is_empty() {
            return;
        }
        let doc = self.active_document_mut();
        let Some(prev) = doc.nav_back.pop() else {
            return;
        };
        let current = NavLocation {
            cursor: doc.doc.editor.selection().cursor,
        };
        if doc.nav_forward.last().copied() != Some(current) {
            doc.nav_forward.push(current);
        }
        doc.doc.editor.set_cursor(prev.cursor, false);
        doc.scroll_to_char = Some(prev.cursor);
    }

    fn navigate_forward(&mut self) {
        if self.documents.is_empty() {
            return;
        }
        let doc = self.active_document_mut();
        let Some(next) = doc.nav_forward.pop() else {
            return;
        };
        let current = NavLocation {
            cursor: doc.doc.editor.selection().cursor,
        };
        if doc.nav_back.last().copied() != Some(current) {
            doc.nav_back.push(current);
        }
        doc.doc.editor.set_cursor(next.cursor, false);
        doc.scroll_to_char = Some(next.cursor);
    }

    fn set_project_root(&mut self, root: PathBuf) {
        self.project_root_input = root.display().to_string();
        self.project_root = Some(root.clone());
        self.project_tree = None;
        self.project_status = "Loading…".to_string();
        self.project_worker = Some(project::ProjectWorker::start(root, self.project_tx.clone()));
    }

    fn start_search(&mut self) {
        let Some(root) = self.project_root.clone() else {
            self.search_status = "No project root".to_string();
            return;
        };
        self.active_search_id = self.active_search_id.wrapping_add(1);
        let request_id = self.active_search_id;
        if let Some(worker) = &self.search_worker {
            worker.cancel();
        }
        self.search_worker = Some(project::SearchWorker::start(
            root,
            self.search_query.clone(),
            self.search_case_sensitive,
            self.config.file.encoding,
            request_id,
            self.search_tx.clone(),
        ));
    }

    fn cancel_search(&mut self) {
        self.active_search_id = self.active_search_id.wrapping_add(1);
        if let Some(worker) = &self.search_worker {
            worker.cancel();
        }
        self.search_worker = None;
        self.search_status = "Canceled".to_string();
    }

    fn capture_window_state(&mut self, ctx: &egui::Context) {
        let snapshot = ctx.input(|i| {
            let vp = i.viewport();
            (
                vp.outer_rect,
                vp.inner_rect.map(|r| (r.width(), r.height())),
                vp.maximized,
                vp.minimized,
            )
        });

        let (outer_rect, inner_size, maximized, minimized) = snapshot;
        if minimized == Some(true) {
            return;
        }

        if let Some(outer) = outer_rect {
            self.config.window.pos_x = Some(outer.min.x);
            self.config.window.pos_y = Some(outer.min.y);
        }
        if let Some((w, h)) = inner_size {
            self.config.window.width = w.max(100.0);
            self.config.window.height = h.max(100.0);
        }
        if let Some(maximized) = maximized {
            self.config.window.maximized = maximized;
        }
    }

    fn ui_project_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Project");
        ui.horizontal(|ui| {
            ui.label("Root");
            ui.text_edit_singleline(&mut self.project_root_input);
            if ui.button("Browse…").clicked() {
                self.open_folder_dialog();
            }
            if ui.button("Set").clicked() {
                self.set_project_root(PathBuf::from(self.project_root_input.trim()));
            }
        });
        ui.label(&self.project_status);
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            if let Some(tree) = &self.project_tree {
                let root_path = self.project_root.as_deref().unwrap_or(tree.path.as_path());
                if let Some(path) = show_tree(ui, tree, root_path) {
                    self.open_in_new_tab(path, None);
                }
            }
        });
    }

    fn ui_search_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Search");
        if self.project_root.is_none() {
            ui.label("Open a project root first.");
            return;
        }

        ui.horizontal(|ui| {
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.search_query)
                    .hint_text("query")
                    .desired_width(f32::INFINITY),
            );
            if self.search_request_focus {
                resp.request_focus();
                self.search_request_focus = false;
            }
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.start_search();
            }
            if ui.button("Go").clicked() {
                self.start_search();
            }
            if ui.button("Cancel").clicked() {
                self.cancel_search();
            }
        });
        ui.checkbox(&mut self.search_case_sensitive, "Case sensitive");
        ui.label(&self.search_status);
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            for idx in 0..self.search_results.len() {
                let label = {
                    let m = &self.search_results[idx];
                    let display_path = self
                        .project_root
                        .as_ref()
                        .and_then(|root| m.path.strip_prefix(root).ok())
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| m.path.display().to_string());
                    format!(
                        "{}:{}:{}  {}",
                        display_path,
                        m.line_index + 1,
                        m.column_chars + 1,
                        m.preview.trim()
                    )
                };
                if ui.selectable_label(false, label).clicked() {
                    let m = self.search_results[idx].clone();
                    self.open_in_new_tab(m.path, Some((m.line_index, m.column_chars)));
                }
            }
        });
    }

    fn find_next(&mut self, backwards: bool) {
        let needle_raw = self.find_query.trim();
        if needle_raw.is_empty() {
            self.find_status = "Empty query".to_string();
            return;
        }
        if self.documents.is_empty() {
            self.find_status = "No file open".to_string();
            return;
        }

        let rope = self.active_document().doc.editor.rope();
        let cursor = self
            .active_document()
            .doc
            .editor
            .selection()
            .cursor
            .min(rope.len_chars());
        let total_lines = rope.len_lines().max(1);

        let (needle, needle_len_chars) = if self.find_case_sensitive {
            (needle_raw.to_string(), needle_raw.chars().count())
        } else {
            (needle_raw.to_ascii_lowercase(), needle_raw.chars().count())
        };
        if needle_len_chars == 0 {
            self.find_status = "Empty query".to_string();
            return;
        }

        let start_line = rope.char_to_line(cursor);
        let start_col = cursor.saturating_sub(rope.line_to_char(start_line));

        if backwards {
            for step in 0..total_lines {
                let line_index = (start_line + total_lines - step) % total_lines;
                let line_start = rope.line_to_char(line_index);
                let (hay, line_len_chars) =
                    find_normalized_line(rope.line(line_index), self.find_case_sensitive);
                let limit = if step == 0 && line_index == start_line {
                    start_col
                } else {
                    line_len_chars
                };
                if let Some(pos) = find_last_before(&hay, &needle, limit) {
                    let start = line_start + pos;
                    let end = start + needle_len_chars;
                    self.record_nav_from(self.current_location());
                    let doc = self.active_document_mut();
                    doc.doc.editor.select_range(start..end);
                    doc.scroll_to_char = Some(start);
                    self.find_status = format!("Found at {}:{}", line_index + 1, pos + 1);
                    return;
                }
            }
        } else {
            for step in 0..total_lines {
                let line_index = (start_line + step) % total_lines;
                let line_start = rope.line_to_char(line_index);
                let (hay, _line_len_chars) =
                    find_normalized_line(rope.line(line_index), self.find_case_sensitive);
                let from = if step == 0 && line_index == start_line {
                    start_col.saturating_add(1)
                } else {
                    0
                };
                if let Some(pos) = find_first_from(&hay, &needle, from) {
                    let start = line_start + pos;
                    let end = start + needle_len_chars;
                    self.record_nav_from(self.current_location());
                    let doc = self.active_document_mut();
                    doc.doc.editor.select_range(start..end);
                    doc.scroll_to_char = Some(start);
                    self.find_status = format!("Found at {}:{}", line_index + 1, pos + 1);
                    return;
                }
            }
        }

        self.find_status = "No matches".to_string();
    }
}

impl eframe::App for RustideApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_load_results();
        self.poll_dialog_results();
        self.capture_window_state(ctx);
        self.ensure_ui_applied(ctx);
        self.poll_project();
        self.poll_save_results();

        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if let Some(path) = dropped_files.into_iter().filter_map(|f| f.path).next() {
            if path.is_dir() {
                self.set_project_root(path);
            } else {
                self.open_in_new_tab(path, None);
            }
        }

        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O)) {
            if ctx.input(|i| i.modifiers.shift) {
                self.open_folder_dialog();
            } else {
                self.open_file_dialog();
            }
        }
        if ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::F)) {
            self.focus_tab(DockTab::Search);
            self.search_request_focus = true;
            self.config.layout.left_tool = config::LeftTool::Search;
        }

        egui::TopBottomPanel::top("top")
            .frame(egui::Frame::NONE.fill(self.theme.visuals.panel_fill))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("RustIDE");
                    ui.separator();
                    ui.menu_button("Open", |ui| {
                        if ui.button("File…").clicked() {
                            self.open_file_dialog();
                            ui.close();
                        }
                        if ui.button("Folder…").clicked() {
                            self.open_folder_dialog();
                            ui.close();
                        }
                    });
                    ui.separator();
                    if !self.status.is_empty() {
                        ui.label(&self.status);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(8.0);
                        self.ui_font_controls(ui, ctx);
                    });
                });
            });

        egui::SidePanel::left("left_toolstrip")
            .resizable(false)
            .min_width(40.0)
            .max_width(40.0)
            .frame(egui::Frame::NONE.fill(self.theme.visuals.panel_fill))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(6.0);
                    if ui.button("P").on_hover_text("Project").clicked() {
                        self.focus_tab(DockTab::Project);
                        self.config.layout.left_tool = config::LeftTool::Project;
                    }
                    if ui.button("S").on_hover_text("Search").clicked() {
                        self.focus_tab(DockTab::Search);
                        self.search_request_focus = true;
                        self.config.layout.left_tool = config::LeftTool::Search;
                    }
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            let mut style = Style::from_egui(ui.style().as_ref());
            style.overlay.overlay_type = egui_dock::OverlayType::HighlightedAreas;
            let mut dock_state = std::mem::replace(&mut self.dock_state, DockState::new(vec![]));
            DockArea::new(&mut dock_state)
                .style(style)
                .show_inside(ui, &mut RustideTabViewer { app: self, ctx });
            self.dock_state = dock_state;
        });

        self.maybe_autosave_active_doc();
        self.persist_dock_layout();
    }
}

struct RustideTabViewer<'a> {
    app: &'a mut RustideApp,
    ctx: &'a egui::Context,
}

impl TabViewer for RustideTabViewer<'_> {
    type Tab = DockTab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.title().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            DockTab::Editor => {
                if ui.input(|i| {
                    i.modifiers.command && !i.modifiers.shift && i.key_pressed(egui::Key::F)
                }) {
                    self.app.find_open = true;
                    self.app.find_request_focus = true;
                }
                if self.app.find_open && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.app.find_open = false;
                    self.app.find_request_focus = false;
                }

                if self.app.find_open {
                    ui.horizontal(|ui| {
                        ui.label("Find");
                        let resp = ui.text_edit_singleline(&mut self.app.find_query);
                        if self.app.find_request_focus {
                            resp.request_focus();
                            self.app.find_request_focus = false;
                        }
                        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            let backwards = ui.input(|i| i.modifiers.shift);
                            self.app.find_next(backwards);
                        }
                        if ui.button("Prev").clicked() {
                            self.app.find_next(true);
                        }
                        if ui.button("Next").clicked() {
                            self.app.find_next(false);
                        }
                        ui.checkbox(&mut self.app.find_case_sensitive, "Aa");
                        if ui.button("X").clicked() {
                            self.app.find_open = false;
                        }
                        ui.label(&self.app.find_status);
                    });
                    ui.separator();
                }

                if self.app.documents.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        ui.label("No file open.");
                        if ui.button("Open File…").clicked() {
                            self.app.open_file_dialog();
                        }
                    });
                    return;
                }

                let path_label = self
                    .app
                    .active_document()
                    .doc
                    .path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "<no file>".to_string());

                // Tab strip (VSCode-like: one window, multiple open files)
                let mut close_tab: Option<usize> = None;
                let mut action_pin: Option<(usize, bool)> = None;
                let mut action_close_others: Option<usize> = None;
                let mut action_close_right: Option<usize> = None;
                let mut action_close_all = false;
                let mut action_close_all_but_pinned = false;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for (idx, doc) in self.app.documents.iter().enumerate() {
                        let title = doc
                            .doc
                            .path
                            .as_ref()
                            .and_then(|p| p.file_name())
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| "Untitled".to_string());
                        let selected = idx == self.app.active_doc;
                        let label = if doc.pinned {
                            format!("{title} 📌")
                        } else {
                            title
                        };
                        let resp = ui.selectable_label(selected, label);
                        if resp.clicked() {
                            self.app.active_doc = idx;
                        }
                        resp.context_menu(|ui| {
                            let mut pinned = doc.pinned;
                            if ui.checkbox(&mut pinned, "Pin").changed() {
                                action_pin = Some((idx, pinned));
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("Close Others").clicked() {
                                action_close_others = Some(idx);
                                ui.close();
                            }
                            if ui.button("Close Tabs to the Right").clicked() {
                                action_close_right = Some(idx);
                                ui.close();
                            }
                            ui.separator();
                            if ui.button("Close All").clicked() {
                                action_close_all = true;
                                ui.close();
                            }
                            if ui.button("Close All But Pinned").clicked() {
                                action_close_all_but_pinned = true;
                                ui.close();
                            }
                        });

                        let can_close = !doc.pinned;
                        if ui
                            .add_enabled(can_close, egui::Button::new("x"))
                            .on_hover_text(if can_close {
                                "Close"
                            } else {
                                "Pinned tabs can't be closed"
                            })
                            .clicked()
                        {
                            close_tab = Some(idx);
                        }
                    }
                });
                ui.separator();

                let header_h = ui
                    .spacing()
                    .interact_size
                    .y
                    .max(ui.text_style_height(&egui::TextStyle::Body));
                let (header_rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), header_h),
                    egui::Sense::hover(),
                );
                let mut go_back = false;
                let mut go_forward = false;
                let mut header_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(header_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                header_ui.scope(|ui| {
                    let font_id = egui::TextStyle::Body.resolve(ui.style());
                    ui.painter().text(
                        header_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        path_label,
                        font_id,
                        ui.visuals().text_color(),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        go_forward |= ui
                            .add_enabled(
                                !self.app.active_document().nav_forward.is_empty(),
                                egui::Button::new("->"),
                            )
                            .on_hover_text("Forward")
                            .clicked();
                        go_back |= ui
                            .add_enabled(
                                !self.app.active_document().nav_back.is_empty(),
                                egui::Button::new("<-"),
                            )
                            .on_hover_text("Back")
                            .clicked();
                    });
                });
                if go_back {
                    self.app.navigate_back();
                }
                if go_forward {
                    self.app.navigate_forward();
                }
                ui.separator();

                if let Some(active) = self.app.documents.get_mut(self.app.active_doc) {
                    let cursor_before = active.doc.editor.selection().cursor;
                    let mut editor_metrics: Option<editor_view::EditorScrollMetrics> = None;

                    {
                        let theme = &self.app.theme;
                        let ui_cfg = &mut self.app.config.ui;
                        let scroll_to = &mut active.scroll_to_char;

                        if let Some(md) = active.markdown.as_mut() {
                            let current_version = active.doc.editor.version();
                            if md.cached_version != current_version {
                                md.cached_text = active.doc.editor.rope().to_string();
                                md.cached_version = current_version;
                            }

                            ui.horizontal(|ui| {
                                ui.checkbox(&mut md.preview_enabled, "Preview");
                            });
                            ui.separator();

                            if md.preview_enabled {
                                ui.columns(2, |cols| {
                                    editor_metrics = Some(editor_view::show_editor(
                                        &mut cols[0],
                                        self.ctx,
                                        &mut active.doc.editor,
                                        active.doc.max_line_chars,
                                        &mut active.syntax,
                                        theme,
                                        ui_cfg,
                                        scroll_to,
                                    ));
                                    let editor_metrics = editor_metrics.unwrap();

                                    cols[1].heading("Markdown");
                                    cols[1].separator();

                                    let src_max_scroll = (editor_metrics.content_h
                                        - editor_metrics.viewport_h)
                                        .max(1.0);
                                    let t =
                                        (editor_metrics.offset_y / src_max_scroll).clamp(0.0, 1.0);
                                    let preview_max_scroll =
                                        (md.preview_content_h - md.preview_viewport_h).max(0.0);
                                    let desired_preview_offset = t * preview_max_scroll;

                                    let preview_output = egui::ScrollArea::vertical()
                                        .auto_shrink([false, false])
                                        .id_salt("rustide_md_preview_scroll")
                                        .vertical_scroll_offset(desired_preview_offset)
                                        .show(&mut cols[1], |ui| {
                                            CommonMarkViewer::new().show(
                                                ui,
                                                &mut md.cache,
                                                &md.cached_text,
                                            );
                                        });
                                    md.preview_content_h = preview_output.content_size.y;
                                    md.preview_viewport_h = preview_output.inner_rect.height();
                                });
                            } else {
                                editor_metrics = Some(editor_view::show_editor(
                                    ui,
                                    self.ctx,
                                    &mut active.doc.editor,
                                    active.doc.max_line_chars,
                                    &mut active.syntax,
                                    theme,
                                    ui_cfg,
                                    scroll_to,
                                ));
                            }
                        } else {
                            editor_metrics = Some(editor_view::show_editor(
                                ui,
                                self.ctx,
                                &mut active.doc.editor,
                                active.doc.max_line_chars,
                                &mut active.syntax,
                                theme,
                                ui_cfg,
                                scroll_to,
                            ));
                        }
                    }

                    if let Some(metrics) = editor_metrics {
                        if metrics.cursor_moved_by_pointer {
                            let cursor_after = active.doc.editor.selection().cursor;
                            if cursor_after != cursor_before {
                                self.app.record_nav_from(NavLocation {
                                    cursor: cursor_before,
                                });
                            }
                        }
                    }
                }

                if let Some((idx, pinned)) = action_pin {
                    if let Some(doc) = self.app.documents.get_mut(idx) {
                        doc.pinned = pinned;
                    }
                }
                if action_close_all {
                    self.app.documents.clear();
                    self.app.active_doc = 0;
                    return;
                }
                if action_close_all_but_pinned {
                    self.app.documents.retain(|d| d.pinned);
                    self.app.active_doc = self.app.active_doc.min(self.app.documents.len().saturating_sub(1));
                    if self.app.documents.is_empty() {
                        return;
                    }
                }
                if let Some(idx) = action_close_others {
                    if self.app.documents.len() > 1 {
                        let keep_path = self.app.documents[idx].doc.path.clone();
                        self.app.documents.retain(|d| {
                            d.pinned
                                || d.doc.path == keep_path
                                || (d.doc.path.is_none() && keep_path.is_none())
                        });
                        self.app.active_doc = self
                            .app
                            .documents
                            .iter()
                            .position(|d| d.doc.path == keep_path)
                            .unwrap_or(0);
                    }
                }
                if let Some(idx) = action_close_right {
                    if self.app.documents.len() > 1 && idx + 1 < self.app.documents.len() {
                        // Remove from the right side, but keep pinned tabs.
                        let mut i = self.app.documents.len();
                        while i > idx + 1 {
                            i -= 1;
                            if !self.app.documents[i].pinned {
                                self.app.documents.remove(i);
                            }
                        }
                        self.app.active_doc = self.app.active_doc.min(self.app.documents.len() - 1);
                    }
                }
                if let Some(idx) = close_tab {
                    if self.app.documents.len() > 1 {
                        self.app.documents.remove(idx);
                        self.app.active_doc = self.app.active_doc.min(self.app.documents.len() - 1);
                    } else {
                        // Keep at least one tab.
                        self.app.documents[0] = OpenDocument::empty();
                        self.app.active_doc = 0;
                    }
                }
            }
            DockTab::Project => self.app.ui_project_panel(ui),
            DockTab::Search => self.app.ui_search_panel(ui),
        }
    }

    fn is_closeable(&self, _tab: &Self::Tab) -> bool {
        false
    }
}

fn default_dock_state() -> DockState<DockTab> {
    let mut dock_state = DockState::new(vec![DockTab::Editor]);
    let root = egui_dock::NodeIndex::root();
    let [left, _] = dock_state.main_surface_mut().split_left(
        root,
        0.25,
        vec![DockTab::Project, DockTab::Search],
    );
    dock_state.main_surface_mut().set_focused_node(left);
    dock_state
}

fn load_or_default_dock_state(cfg: &config::AppConfig) -> DockState<DockTab> {
    let Some(json) = cfg.layout.dock_layout_json.as_deref() else {
        return default_dock_state();
    };
    serde_json::from_str(json).unwrap_or_else(|_| default_dock_state())
}

impl RustideApp {
    fn persist_dock_layout(&mut self) {
        let Ok(json) = serde_json::to_string(&self.dock_state) else {
            return;
        };
        if self.config.layout.dock_layout_json.as_deref() != Some(json.as_str()) {
            self.config.layout.dock_layout_json = Some(json);
        }
    }

    fn focus_tab(&mut self, tab: DockTab) {
        if let Some((surface, node, index)) = self.dock_state.find_tab(&tab) {
            self.dock_state.set_active_tab((surface, node, index));
            self.dock_state
                .set_focused_node_and_surface((surface, node));
        }
    }
}

fn find_normalized_line(line: ropey::RopeSlice<'_>, case_sensitive: bool) -> (String, usize) {
    let mut text = line.to_string();
    if text.ends_with('\n') {
        text.pop();
        if text.ends_with('\r') {
            text.pop();
        }
    }
    let len_chars = text.chars().count();
    let hay = if case_sensitive {
        text
    } else {
        text.to_ascii_lowercase()
    };
    (hay, len_chars)
}

fn find_first_from(hay: &str, needle: &str, from_char: usize) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    let from_byte = char_to_byte_index(hay, from_char);
    let idx = hay.get(from_byte..)?.find(needle)?;
    let byte = from_byte + idx;
    Some(hay[..byte].chars().count())
}

fn find_last_before(hay: &str, needle: &str, before_char: usize) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    let before_byte = char_to_byte_index(hay, before_char);
    let prefix = hay.get(..before_byte).unwrap_or(hay);
    let mut best: Option<usize> = None;
    let mut start = 0usize;
    loop {
        let Some(rest) = prefix.get(start..) else {
            break;
        };
        let Some(found) = rest.find(needle) else {
            break;
        };
        let at = start + found;
        best = Some(at);
        start = at + 1;
    }
    let best_byte = best?;
    Some(prefix[..best_byte].chars().count())
}

fn char_to_byte_index(text: &str, char_index: usize) -> usize {
    if char_index == 0 {
        return 0;
    }
    match text.char_indices().nth(char_index) {
        Some((byte, _)) => byte,
        None => text.len(),
    }
}

fn show_tree(
    ui: &mut egui::Ui,
    node: &rustide_project::TreeNode,
    root_path: &Path,
) -> Option<PathBuf> {
    fn file_tag(path: &Path) -> &'static str {
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "rs" => "[RS]",
            "cpp" | "cc" | "cxx" | "c" | "hpp" | "hh" | "hxx" | "h" => "[CPP]",
            "py" => "[PY]",
            "md" | "markdown" => "[MD]",
            "hlsl" | "hlsli" | "fx" => "[HLSL]",
            "toml" => "[TOML]",
            "json" => "[JSON]",
            "yaml" | "yml" => "[YAML]",
            _ => "[FILE]",
        }
    }

    if node.is_dir {
        let id = ui.make_persistent_id(&node.path);
        let mut clicked: Option<PathBuf> = None;
        egui::CollapsingHeader::new(format!("[DIR] {}", node.name))
            .id_salt(id)
            .default_open(node.path == root_path)
            .show(ui, |ui| {
                for child in &node.children {
                    if clicked.is_none() {
                        clicked = show_tree(ui, child, root_path);
                    }
                }
            });
        return clicked;
    }

    let label = format!("{} {}", file_tag(&node.path), node.name);
    if ui.selectable_label(false, label).clicked() {
        return Some(node.path.clone());
    }

    None
}

impl Drop for RustideApp {
    fn drop(&mut self) {
        if let Err(err) = self.config.save(&self.config_path) {
            tracing::warn!(
                "Failed to save config to {}: {err}",
                self.config_path.display()
            );
        }
    }
}

impl RustideApp {
    fn ensure_ui_applied(&mut self, ctx: &egui::Context) {
        let desired = (
            self.config.ui.monospace_font,
            self.config.ui.monospace_size,
            self.config.ui.theme,
            ctx.pixels_per_point(),
        );

        if self.last_applied_ui != Some(desired) {
            self.theme = theme::build_theme(self.config.ui.theme);
            apply_font_families(ctx, &self.font_state, &self.config.ui);
            self.last_applied_ui = Some(desired);
        }

        // Apply every frame to override any system/default theme changes in eframe/egui.
        theme::apply_theme(ctx, &self.theme);
        apply_ui_style(ctx, &self.config.ui);
    }

    fn ui_font_controls(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.checkbox(&mut self.config.startup.open_last_file, "Open last");
        ui.separator();

        ui.label("Theme");
        let theme_before = self.config.ui.theme;
        egui::ComboBox::from_id_salt("theme")
            .selected_text(self.config.ui.theme.to_string())
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.config.ui.theme, theme::ThemeId::Dark, "dark");
                ui.selectable_value(&mut self.config.ui.theme, theme::ThemeId::Light, "light");
                ui.selectable_value(
                    &mut self.config.ui.theme,
                    theme::ThemeId::SolarizedDark,
                    "solarized-dark",
                );
                ui.selectable_value(
                    &mut self.config.ui.theme,
                    theme::ThemeId::Monokai,
                    "monokai",
                );
            });
        if self.config.ui.theme != theme_before {
            self.last_applied_ui = None;
            self.ensure_ui_applied(ctx);
        }

        ui.label("Font");
        let font_before = self.config.ui.monospace_font;
        egui::ComboBox::from_id_salt("monospace_font")
            .selected_text(self.config.ui.monospace_font.to_string())
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.config.ui.monospace_font,
                    config::MonospaceFont::Consolas,
                    "consolas",
                );
                ui.selectable_value(
                    &mut self.config.ui.monospace_font,
                    config::MonospaceFont::SimHei,
                    "simhei",
                );
            });
        if self.config.ui.monospace_font != font_before {
            self.last_applied_ui = None;
            self.ensure_ui_applied(ctx);
        }

        ui.label("Style");
        let style_before = self.config.ui.monospace_style;
        egui::ComboBox::from_id_salt("monospace_style")
            .selected_text(self.config.ui.monospace_style.to_string())
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.config.ui.monospace_style,
                    config::MonospaceStyle::Regular,
                    "regular",
                );
                ui.selectable_value(
                    &mut self.config.ui.monospace_style,
                    config::MonospaceStyle::Bold,
                    "bold",
                );
                ui.selectable_value(
                    &mut self.config.ui.monospace_style,
                    config::MonospaceStyle::Italic,
                    "italic",
                );
                ui.selectable_value(
                    &mut self.config.ui.monospace_style,
                    config::MonospaceStyle::BoldItalic,
                    "bold-italic",
                );
            });
        if self.config.ui.monospace_style != style_before {
            self.last_applied_ui = None;
            self.ensure_ui_applied(ctx);
        }

        ui.label("Size");
        let size_before = self.config.ui.monospace_size;
        ui.add(egui::DragValue::new(&mut self.config.ui.monospace_size).range(8.0..=48.0));
        if (self.config.ui.monospace_size - size_before).abs() > f32::EPSILON {
            self.last_applied_ui = None;
            self.ensure_ui_applied(ctx);
        }
    }
}

#[derive(Clone)]
struct FontState {
    base_definitions: egui::FontDefinitions,
    user_font_name: Option<String>,
    consolas_font_name: Option<String>,
    consolas_bold_font_name: Option<String>,
    consolas_italic_font_name: Option<String>,
    consolas_bold_italic_font_name: Option<String>,
    simhei_font_name: Option<String>,
}

fn build_font_state(cfg: &config::UiConfig) -> FontState {
    let mut defs = egui::FontDefinitions::default();

    let user_font_name = cfg.font_file.as_ref().and_then(|path| {
        let data = std::fs::read(path).ok()?;
        let name = format!("rustide_user_font:{}", path.display());
        defs.font_data
            .insert(name.clone(), egui::FontData::from_owned(data).into());
        Some(name)
    });

    let mut consolas_font_name = None;
    let mut consolas_bold_font_name = None;
    let mut consolas_italic_font_name = None;
    let mut consolas_bold_italic_font_name = None;
    let mut simhei_font_name = None;
    if cfg!(windows) {
        if let Some(windir) = std::env::var_os("WINDIR") {
            let fonts_dir = PathBuf::from(windir).join("Fonts");

            let consolas_path = fonts_dir.join("consola.ttf");
            if let Ok(data) = std::fs::read(&consolas_path) {
                let name = "rustide_font:consolas".to_string();
                defs.font_data
                    .insert(name.clone(), egui::FontData::from_owned(data).into());
                consolas_font_name = Some(name);
            }

            let consolas_bold_path = fonts_dir.join("consolab.ttf");
            if let Ok(data) = std::fs::read(&consolas_bold_path) {
                let name = "rustide_font:consolas_bold".to_string();
                defs.font_data
                    .insert(name.clone(), egui::FontData::from_owned(data).into());
                consolas_bold_font_name = Some(name);
            }

            let consolas_italic_path = fonts_dir.join("consolai.ttf");
            if let Ok(data) = std::fs::read(&consolas_italic_path) {
                let name = "rustide_font:consolas_italic".to_string();
                defs.font_data
                    .insert(name.clone(), egui::FontData::from_owned(data).into());
                consolas_italic_font_name = Some(name);
            }

            let consolas_bold_italic_path = fonts_dir.join("consolaz.ttf");
            if let Ok(data) = std::fs::read(&consolas_bold_italic_path) {
                let name = "rustide_font:consolas_bold_italic".to_string();
                defs.font_data
                    .insert(name.clone(), egui::FontData::from_owned(data).into());
                consolas_bold_italic_font_name = Some(name);
            }

            let simhei_path = fonts_dir.join("simhei.ttf");
            if let Ok(data) = std::fs::read(&simhei_path) {
                let name = "rustide_font:simhei".to_string();
                defs.font_data
                    .insert(name.clone(), egui::FontData::from_owned(data).into());
                simhei_font_name = Some(name);
            }
        }
    }

    FontState {
        base_definitions: defs,
        user_font_name,
        consolas_font_name,
        consolas_bold_font_name,
        consolas_italic_font_name,
        consolas_bold_italic_font_name,
        simhei_font_name,
    }
}

fn apply_font_families(ctx: &egui::Context, fonts: &FontState, cfg: &config::UiConfig) {
    let mut defs = fonts.base_definitions.clone();

    let mut monospace = Vec::<String>::new();
    if let Some(name) = &fonts.user_font_name {
        monospace.push(name.clone());
    }

    let (primary_regular, primary_bold, primary_italic, primary_bold_italic, secondary_regular) =
        match cfg.monospace_font {
            config::MonospaceFont::Consolas => (
                &fonts.consolas_font_name,
                &fonts.consolas_bold_font_name,
                &fonts.consolas_italic_font_name,
                &fonts.consolas_bold_italic_font_name,
                &fonts.simhei_font_name,
            ),
            config::MonospaceFont::SimHei => (
                &fonts.simhei_font_name,
                &fonts.simhei_font_name,
                &fonts.simhei_font_name,
                &fonts.simhei_font_name,
                &fonts.consolas_font_name,
            ),
        };

    let primary = match cfg.monospace_style {
        config::MonospaceStyle::Regular => primary_regular,
        config::MonospaceStyle::Bold => primary_bold,
        config::MonospaceStyle::Italic => primary_italic,
        config::MonospaceStyle::BoldItalic => primary_bold_italic,
    };

    if let Some(name) = primary {
        monospace.push(name.clone());
    }
    if let Some(name) = secondary_regular {
        monospace.push(name.clone());
    }

    if let Some(family) = defs.families.get_mut(&egui::FontFamily::Monospace) {
        for name in monospace.into_iter().rev() {
            family.insert(0, name);
        }
    }

    if let Some(family) = defs.families.get_mut(&egui::FontFamily::Proportional) {
        // Prefer the configured editor font for UI as well, but keep egui defaults as fallbacks.
        let existing = family.clone();
        family.clear();

        let mut preferred: Vec<String> = Vec::new();
        if let Some(name) = &fonts.user_font_name {
            preferred.push(name.clone());
        }
        if let Some(name) = primary_regular {
            preferred.push(name.clone());
        }
        if let Some(name) = secondary_regular {
            preferred.push(name.clone());
        }
        for name in preferred {
            if !family.contains(&name) {
                family.push(name);
            }
        }
        for name in existing {
            if !family.contains(&name) {
                family.push(name);
            }
        }
    }

    ctx.set_fonts(defs);
}

fn apply_ui_style(ctx: &egui::Context, cfg: &config::UiConfig) {
    let mut style = (*ctx.style()).clone();
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        egui::FontId::new(cfg.monospace_size, egui::FontFamily::Monospace),
    );
    ctx.set_style(style);
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config_path = config::default_config_path();
    let config = config::AppConfig::load_or_default(&config_path);

    let initial_path = std::env::args_os().nth(1).map(PathBuf::from).or_else(|| {
        if config.startup.open_last_file {
            config
                .startup
                .last_file
                .clone()
                .filter(|p| p.exists() && p.is_file())
        } else {
            None
        }
    });

    let mut native_options = eframe::NativeOptions::default();
    native_options.viewport = native_options
        .viewport
        .with_app_id("RustIDE")
        .with_inner_size(egui::vec2(config.window.width, config.window.height))
        .with_maximized(config.window.maximized);
    if let (Some(x), Some(y)) = (config.window.pos_x, config.window.pos_y) {
        native_options.viewport = native_options.viewport.with_position(egui::pos2(x, y));
    }

    let result = eframe::run_native(
        "RustIDE",
        native_options,
        Box::new(move |cc| {
            let font_state = build_font_state(&config.ui);
            let theme = theme::build_theme(config.ui.theme);
            theme::apply_theme(&cc.egui_ctx, &theme);
            apply_ui_style(&cc.egui_ctx, &config.ui);
            apply_font_families(&cc.egui_ctx, &font_state, &config.ui);
            Ok(Box::new(RustideApp::new(
                initial_path.clone(),
                config.clone(),
                config_path.clone(),
                font_state,
            )))
        }),
    );

    result.map_err(|e| anyhow::anyhow!(e.to_string()))
}
