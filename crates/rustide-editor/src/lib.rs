mod document;
mod editor;
mod selection;

pub use document::decode_bytes;
pub use document::encode_text;
pub use document::{Document, EditorError, TextEncoding, TextEncodingHint};
pub use editor::{Editor, EditorEdit, EditorPoint};
pub use selection::Selection;

#[cfg(test)]
mod tests;
