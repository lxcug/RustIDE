use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::Editor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEncodingHint {
    Auto,
    Utf8,
    Utf16Le,
    Utf16Be,
    Gbk,
    Big5,
}

impl FromStr for TextEncodingHint {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "utf8" | "utf-8" => Ok(Self::Utf8),
            "utf16le" | "utf-16le" | "utf16-le" => Ok(Self::Utf16Le),
            "utf16be" | "utf-16be" | "utf16-be" => Ok(Self::Utf16Be),
            "gbk" | "gb2312" | "cp936" => Ok(Self::Gbk),
            "big5" | "big-5" | "cp950" => Ok(Self::Big5),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for TextEncodingHint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => f.write_str("auto"),
            Self::Utf8 => f.write_str("utf-8"),
            Self::Utf16Le => f.write_str("utf-16le"),
            Self::Utf16Be => f.write_str("utf-16be"),
            Self::Gbk => f.write_str("gbk"),
            Self::Big5 => f.write_str("big5"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEncoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Gbk,
    Big5,
}

#[derive(Debug, Clone)]
pub struct Document {
    pub path: Option<PathBuf>,
    pub encoding: TextEncoding,
    pub max_line_chars: usize,
    pub editor: Editor,
}

impl Document {
    pub fn empty() -> Self {
        Self {
            path: None,
            encoding: TextEncoding::Utf8,
            max_line_chars: 0,
            editor: Editor::empty(),
        }
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, EditorError> {
        Self::load_with_encoding(path, TextEncodingHint::Auto)
    }

    pub fn load_with_encoding(
        path: impl AsRef<Path>,
        encoding_hint: TextEncodingHint,
    ) -> Result<Self, EditorError> {
        let path = path.as_ref().to_path_buf();
        let bytes = std::fs::read(&path).map_err(|source| EditorError::Io {
            path: path.clone(),
            source,
        })?;

        let (text, encoding) = decode_bytes(&bytes, encoding_hint);
        let max_line_chars = text
            .lines()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0);
        Ok(Self {
            path: Some(path),
            encoding,
            max_line_chars,
            editor: Editor::from_text(&text),
        })
    }

    pub fn save(&self) -> Result<(), EditorError> {
        let path = self.path.as_ref().ok_or(EditorError::MissingPath)?;
        let bytes = encode_text(&self.editor.rope().to_string(), self.encoding);
        std::fs::write(path, bytes).map_err(|source| EditorError::Io {
            path: path.clone(),
            source,
        })
    }
}

pub fn decode_bytes(bytes: &[u8], hint: TextEncodingHint) -> (String, TextEncoding) {
    // Decode order:
    // 1) BOM (UTF-8/UTF-16LE/UTF-16BE)
    // 2) Explicit user hint (if any)
    // 3) UTF-8 strict
    // 4) Heuristic fallback for common CJK encodings (GBK/Big5)
    if let Some(without_bom) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return (
            String::from_utf8_lossy(without_bom).into_owned(),
            TextEncoding::Utf8Bom,
        );
    }
    if let Some(without_bom) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let (text, _, _had_errors) = encoding_rs::UTF_16LE.decode(without_bom);
        return (text.into_owned(), TextEncoding::Utf16Le);
    }
    if let Some(without_bom) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        let (text, _, _had_errors) = encoding_rs::UTF_16BE.decode(without_bom);
        return (text.into_owned(), TextEncoding::Utf16Be);
    }

    match hint {
        TextEncodingHint::Utf8 => {
            return (
                String::from_utf8_lossy(bytes).into_owned(),
                TextEncoding::Utf8,
            )
        }
        TextEncodingHint::Utf16Le => {
            let (text, _, _had_errors) = encoding_rs::UTF_16LE.decode(bytes);
            return (text.into_owned(), TextEncoding::Utf16Le);
        }
        TextEncodingHint::Utf16Be => {
            let (text, _, _had_errors) = encoding_rs::UTF_16BE.decode(bytes);
            return (text.into_owned(), TextEncoding::Utf16Be);
        }
        TextEncodingHint::Gbk => {
            let (text, _, _had_errors) = encoding_rs::GBK.decode(bytes);
            return (text.into_owned(), TextEncoding::Gbk);
        }
        TextEncodingHint::Big5 => {
            let (text, _, _had_errors) = encoding_rs::BIG5.decode(bytes);
            return (text.into_owned(), TextEncoding::Big5);
        }
        TextEncodingHint::Auto => {}
    }

    if let Ok(text) = std::str::from_utf8(bytes) {
        return (text.to_string(), TextEncoding::Utf8);
    }

    let (gbk_text, _, gbk_errors) = encoding_rs::GBK.decode(bytes);
    let (big5_text, _, big5_errors) = encoding_rs::BIG5.decode(bytes);

    match (gbk_errors, big5_errors) {
        (false, true) => (gbk_text.into_owned(), TextEncoding::Gbk),
        (true, false) => (big5_text.into_owned(), TextEncoding::Big5),
        _ => (gbk_text.into_owned(), TextEncoding::Gbk),
    }
}

pub fn encode_text(text: &str, encoding: TextEncoding) -> Vec<u8> {
    match encoding {
        TextEncoding::Utf8 => text.as_bytes().to_vec(),
        TextEncoding::Utf8Bom => {
            let mut output = Vec::with_capacity(text.len() + 3);
            output.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
            output.extend_from_slice(text.as_bytes());
            output
        }
        TextEncoding::Utf16Le => {
            let (encoded, _, _) = encoding_rs::UTF_16LE.encode(text);
            let mut output = Vec::with_capacity(encoded.len() + 2);
            output.extend_from_slice(&[0xFF, 0xFE]);
            output.extend_from_slice(&encoded);
            output
        }
        TextEncoding::Utf16Be => {
            let (encoded, _, _) = encoding_rs::UTF_16BE.encode(text);
            let mut output = Vec::with_capacity(encoded.len() + 2);
            output.extend_from_slice(&[0xFE, 0xFF]);
            output.extend_from_slice(&encoded);
            output
        }
        TextEncoding::Gbk => encoding_rs::GBK.encode(text).0.into_owned(),
        TextEncoding::Big5 => encoding_rs::BIG5.encode(text).0.into_owned(),
    }
}

#[derive(thiserror::Error, Debug)]
pub enum EditorError {
    #[error("failed to read file '{path}': {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("document has no path")]
    MissingPath,
}
