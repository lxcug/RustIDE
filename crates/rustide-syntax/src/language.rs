#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageId {
    Cpp,
    Python,
    Hlsl,
    Markdown,
    PlainText,
}

impl LanguageId {
    pub fn from_path(path: Option<&std::path::Path>) -> Self {
        let Some(path) = path else {
            return Self::PlainText;
        };
        let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
            return Self::PlainText;
        };
        match ext.to_ascii_lowercase().as_str() {
            "cc" | "cpp" | "cxx" | "h" | "hpp" | "hh" => Self::Cpp,
            "py" => Self::Python,
            "hlsl" | "hlsli" | "fx" => Self::Hlsl,
            "md" | "markdown" => Self::Markdown,
            _ => Self::PlainText,
        }
    }
}
