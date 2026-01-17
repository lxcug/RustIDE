use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct AppConfig {
    pub window: WindowConfig,
    pub file: FileConfig,
    pub ui: UiConfig,
    pub startup: StartupConfig,
    pub layout: LayoutConfig,
}

#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub pos_x: Option<f32>,
    pub pos_y: Option<f32>,
    pub width: f32,
    pub height: f32,
    pub maximized: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            pos_x: None,
            pos_y: None,
            width: 1200.0,
            height: 800.0,
            maximized: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileConfig {
    pub encoding: rustide_editor::TextEncodingHint,
}

impl Default for FileConfig {
    fn default() -> Self {
        Self {
            encoding: rustide_editor::TextEncodingHint::Auto,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UiConfig {
    pub font_file: Option<PathBuf>,
    pub monospace_font: MonospaceFont,
    pub monospace_size: f32,
    pub monospace_style: MonospaceStyle,
    pub theme: crate::theme::ThemeId,
    pub minimap_width: f32,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            font_file: None,
            monospace_font: MonospaceFont::Consolas,
            monospace_size: 14.0,
            monospace_style: MonospaceStyle::Regular,
            theme: crate::theme::ThemeId::Dark,
            minimap_width: 80.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MonospaceStyle {
    #[default]
    Regular,
    Bold,
    Italic,
    BoldItalic,
}

impl std::str::FromStr for MonospaceStyle {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "regular" | "normal" => Ok(Self::Regular),
            "bold" => Ok(Self::Bold),
            "italic" => Ok(Self::Italic),
            "bolditalic" | "bold-italic" | "bold_italic" => Ok(Self::BoldItalic),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for MonospaceStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Regular => f.write_str("regular"),
            Self::Bold => f.write_str("bold"),
            Self::Italic => f.write_str("italic"),
            Self::BoldItalic => f.write_str("bold-italic"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MonospaceFont {
    #[default]
    Consolas,
    SimHei,
}

impl std::str::FromStr for MonospaceFont {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "consolas" => Ok(Self::Consolas),
            "simhei" => Ok(Self::SimHei),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for MonospaceFont {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Consolas => f.write_str("consolas"),
            Self::SimHei => f.write_str("simhei"),
        }
    }
}

impl AppConfig {
    pub fn load_or_default(path: &Path) -> Self {
        // Minimal INI-like parser (sections + key=value), intentionally small and dependency-free.
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };

        let mut cfg = Self::default();
        let mut section = String::new();

        for raw_line in text.lines() {
            let line = raw_line
                .split_once('#')
                .map(|(left, _)| left)
                .unwrap_or(raw_line)
                .trim();
            if line.is_empty() {
                continue;
            }

            if let Some(section_name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = section_name.trim().to_ascii_lowercase();
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();

            match section.as_str() {
                "window" => apply_window_kv(&mut cfg.window, &key, value),
                "file" => apply_file_kv(&mut cfg.file, &key, value),
                "ui" => apply_ui_kv(&mut cfg.ui, &key, value),
                "startup" => apply_startup_kv(&mut cfg.startup, &key, value),
                "layout" => apply_layout_kv(&mut cfg.layout, &key, value),
                _ => {}
            }
        }

        cfg
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, self.to_string())?;
        // `rename` won't overwrite on Windows, so remove the old file first.
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
        std::fs::rename(tmp, path)?;
        Ok(())
    }
}

impl std::fmt::Display for AppConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "# RustIDE config")?;
        writeln!(f)?;

        writeln!(f, "[window]")?;
        if let Some(x) = self.window.pos_x {
            writeln!(f, "pos_x={x}")?;
        }
        if let Some(y) = self.window.pos_y {
            writeln!(f, "pos_y={y}")?;
        }
        writeln!(f, "width={}", self.window.width)?;
        writeln!(f, "height={}", self.window.height)?;
        writeln!(f, "maximized={}", self.window.maximized)?;
        writeln!(f)?;

        writeln!(f, "[file]")?;
        writeln!(f, "encoding={}", self.file.encoding)?;
        writeln!(f)?;

        writeln!(f, "[ui]")?;
        if let Some(font_file) = &self.ui.font_file {
            writeln!(f, "font_file={}", font_file.display())?;
        }
        writeln!(f, "monospace_font={}", self.ui.monospace_font)?;
        writeln!(f, "monospace_size={}", self.ui.monospace_size)?;
        writeln!(f, "monospace_style={}", self.ui.monospace_style)?;
        writeln!(f, "theme={}", self.ui.theme)?;
        writeln!(f, "minimap_width={}", self.ui.minimap_width)?;
        writeln!(f)?;

        writeln!(f, "[startup]")?;
        writeln!(f, "open_last_file={}", self.startup.open_last_file)?;
        if let Some(p) = &self.startup.last_file {
            writeln!(f, "last_file={}", p.display())?;
        }
        writeln!(f)?;

        writeln!(f, "[layout]")?;
        writeln!(f, "left_tool={}", self.layout.left_tool)?;
        writeln!(f, "left_panel_width={}", self.layout.left_panel_width)?;
        if let Some(json) = &self.layout.dock_layout_json {
            writeln!(f, "dock_layout_json={json}")?;
        }

        Ok(())
    }
}

fn apply_window_kv(window: &mut WindowConfig, key: &str, value: &str) {
    match key {
        "pos_x" => window.pos_x = value.parse::<f32>().ok(),
        "pos_y" => window.pos_y = value.parse::<f32>().ok(),
        "width" => {
            if let Ok(v) = value.parse::<f32>() {
                window.width = v.max(100.0);
            }
        }
        "height" => {
            if let Ok(v) = value.parse::<f32>() {
                window.height = v.max(100.0);
            }
        }
        "maximized" => window.maximized = parse_bool(value).unwrap_or(window.maximized),
        _ => {}
    }
}

fn apply_file_kv(file: &mut FileConfig, key: &str, value: &str) {
    if key == "encoding" {
        if let Ok(enc) = value.parse::<rustide_editor::TextEncodingHint>() {
            file.encoding = enc;
        }
    }
}

fn apply_ui_kv(ui: &mut UiConfig, key: &str, value: &str) {
    if key == "font_file" {
        let trimmed = value.trim().trim_matches('"');
        if !trimmed.is_empty() {
            ui.font_file = Some(PathBuf::from(trimmed));
        }
    }
    if key == "monospace_font" {
        if let Ok(v) = value.parse::<MonospaceFont>() {
            ui.monospace_font = v;
        }
    }
    if key == "monospace_size" {
        if let Ok(v) = value.parse::<f32>() {
            ui.monospace_size = v.clamp(8.0, 48.0);
        }
    }
    if key == "monospace_style" {
        if let Ok(v) = value.parse::<MonospaceStyle>() {
            ui.monospace_style = v;
        }
    }
    if key == "theme" {
        if let Ok(v) = value.parse::<crate::theme::ThemeId>() {
            ui.theme = v;
        }
    }
    if key == "minimap_width" {
        if let Ok(v) = value.parse::<f32>() {
            ui.minimap_width = v.clamp(40.0, 220.0);
        }
    }
}

fn apply_startup_kv(startup: &mut StartupConfig, key: &str, value: &str) {
    if key == "open_last_file" {
        if let Some(v) = parse_bool(value) {
            startup.open_last_file = v;
        }
    }
    if key == "last_file" {
        let trimmed = value.trim().trim_matches('"');
        if !trimmed.is_empty() {
            startup.last_file = Some(PathBuf::from(trimmed));
        }
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct StartupConfig {
    pub open_last_file: bool,
    pub last_file: Option<PathBuf>,
}

impl Default for StartupConfig {
    fn default() -> Self {
        Self {
            open_last_file: true,
            last_file: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LeftTool {
    #[default]
    Project,
    Search,
    None,
}

impl std::str::FromStr for LeftTool {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "project" => Ok(Self::Project),
            "search" => Ok(Self::Search),
            "none" | "off" | "hidden" => Ok(Self::None),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for LeftTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Project => f.write_str("project"),
            Self::Search => f.write_str("search"),
            Self::None => f.write_str("none"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LayoutConfig {
    pub left_tool: LeftTool,
    pub left_panel_width: f32,
    pub dock_layout_json: Option<String>,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            left_tool: LeftTool::Project,
            left_panel_width: 280.0,
            dock_layout_json: None,
        }
    }
}

fn apply_layout_kv(layout: &mut LayoutConfig, key: &str, value: &str) {
    if key == "left_tool" {
        if let Ok(v) = value.parse::<LeftTool>() {
            layout.left_tool = v;
        }
    }
    if key == "left_panel_width" {
        if let Ok(v) = value.parse::<f32>() {
            layout.left_panel_width = v.max(120.0);
        }
    }
    if key == "dock_layout_json" {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            layout.dock_layout_json = None;
        } else {
            layout.dock_layout_json = Some(trimmed.to_string());
        }
    }
}

pub fn default_config_path() -> PathBuf {
    // `RUSTIDE_CONFIG` overrides the default, which is under the per-user config dir.
    if let Some(p) = std::env::var_os("RUSTIDE_CONFIG") {
        return PathBuf::from(p);
    }

    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| {
            std::env::var_os("HOME").map(|home| {
                let mut p = PathBuf::from(home);
                p.push(".config");
                p
            })
        })
        .unwrap_or_else(|| PathBuf::from("."));

    base.join("RustIDE").join("config.ini")
}
