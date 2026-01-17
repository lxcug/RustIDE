use std::ops::Range;
use std::time::{Duration, Instant};

use ropey::Rope;
use streaming_iterator::StreamingIterator;
use tree_sitter::{InputEdit, Parser, Query, QueryCursor, TextProvider, Tree};

use crate::LanguageId;

#[derive(thiserror::Error, Debug)]
pub enum SyntaxError {
    #[error("failed to create tree-sitter parser")]
    ParserInit,
    #[error("tree-sitter parse failed")]
    ParseFailed,
    #[error("tree-sitter query error: {0}")]
    Query(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightTag {
    Comment,
    String,
    Number,
    Keyword,
    Type,
    Function,
    Constant,
    Variable,
    Property,
    Operator,
    Punctuation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    pub byte_range: Range<usize>,
    pub tag: HighlightTag,
}

pub struct SyntaxState {
    language: LanguageId,
    parser: Parser,
    tree: Option<Tree>,
    query: Option<Query>,
    cursor: QueryCursor,
    debounce: Duration,
    pending_since: Option<Instant>,
}

impl SyntaxState {
    pub fn new(language: LanguageId) -> Result<Self, SyntaxError> {
        let mut parser = Parser::new();
        let (query, debounce) = match language {
            LanguageId::Cpp => {
                parser
                    .set_language(&tree_sitter_cpp::LANGUAGE.into())
                    .map_err(|_| SyntaxError::ParserInit)?;
                let lang = tree_sitter_cpp::LANGUAGE.into();
                let query_src = format!(
                    "{}\n{}",
                    tree_sitter_cpp::HIGHLIGHT_QUERY,
                    r#"
(comment) @comment
(number_literal) @number
(char_literal) @string
(string_literal) @string
(raw_string_literal) @string
(concatenated_string) @string
(system_lib_string) @string
(preproc_directive) @keyword
"#
                );
                let query =
                    Query::new(&lang, &query_src).map_err(|e| SyntaxError::Query(e.message))?;
                (Some(query), Duration::from_millis(40))
            }
            LanguageId::Python => {
                parser
                    .set_language(&tree_sitter_python::LANGUAGE.into())
                    .map_err(|_| SyntaxError::ParserInit)?;
                let lang = tree_sitter_python::LANGUAGE.into();
                let query = Query::new(&lang, tree_sitter_python::HIGHLIGHTS_QUERY)
                    .map_err(|e| SyntaxError::Query(e.message))?;
                (Some(query), Duration::from_millis(40))
            }
            LanguageId::Hlsl => {
                parser
                    .set_language(&tree_sitter_hlsl::LANGUAGE_HLSL.into())
                    .map_err(|_| SyntaxError::ParserInit)?;
                let lang = tree_sitter_hlsl::LANGUAGE_HLSL.into();
                let query_src = r#"
(comment) @comment
(number_literal) @number
(string_literal) @string
(raw_string_literal) @string
(concatenated_string) @string
(system_lib_string) @string
(preproc_directive) @keyword
(primitive_type) @type
(type_identifier) @type

(call_expression
  function: (identifier) @function)

(call_expression
  function: (field_expression
              field: (field_identifier) @function))

(function_definition
  declarator: (function_declarator
                declarator: (identifier) @function))

(field_identifier) @property
"#;
                let query =
                    Query::new(&lang, query_src).map_err(|e| SyntaxError::Query(e.message))?;
                (Some(query), Duration::from_millis(40))
            }
            LanguageId::Markdown | LanguageId::PlainText => (None, Duration::from_millis(0)),
        };

        Ok(Self {
            language,
            parser,
            tree: None,
            query,
            cursor: QueryCursor::new(),
            debounce,
            pending_since: None,
        })
    }

    pub fn language(&self) -> LanguageId {
        self.language
    }

    pub fn set_text(&mut self, rope: &Rope) -> Result<(), SyntaxError> {
        if self.language == LanguageId::PlainText || self.language == LanguageId::Markdown {
            self.tree = None;
            return Ok(());
        }

        let tree = self
            .parser
            .parse_with(
                &mut |byte_offset, _| rope_chunk_from_byte(rope, byte_offset),
                None,
            )
            .ok_or(SyntaxError::ParseFailed)?;
        self.tree = Some(tree);
        self.pending_since = None;
        Ok(())
    }

    pub fn queue_edit(&mut self, edit: InputEdit) {
        if self.language == LanguageId::PlainText || self.language == LanguageId::Markdown {
            return;
        }

        if let Some(tree) = &mut self.tree {
            tree.edit(&edit);
        }
        self.pending_since = Some(Instant::now());
    }

    pub fn ensure_parsed(&mut self, rope: &Rope) -> Result<(), SyntaxError> {
        if self.language == LanguageId::PlainText || self.language == LanguageId::Markdown {
            return Ok(());
        }
        let Some(pending_since) = self.pending_since else {
            return Ok(());
        };
        if pending_since.elapsed() < self.debounce {
            return Ok(());
        }

        let tree = self
            .parser
            .parse_with(
                &mut |byte_offset, _| rope_chunk_from_byte(rope, byte_offset),
                self.tree.as_ref(),
            )
            .ok_or(SyntaxError::ParseFailed)?;
        self.tree = Some(tree);
        self.pending_since = None;
        Ok(())
    }

    pub fn highlight_spans(
        &mut self,
        rope: &Rope,
        byte_range: Range<usize>,
    ) -> Result<Vec<HighlightSpan>, SyntaxError> {
        self.ensure_parsed(rope)?;

        let (Some(query), Some(tree)) = (&self.query, &self.tree) else {
            return Ok(Vec::new());
        };

        let root = tree.root_node();
        self.cursor.set_byte_range(byte_range.clone());
        let provider = RopeTextProvider { rope };

        let mut spans: Vec<HighlightSpan> = Vec::new();
        let mut captures = self.cursor.captures(query, root, provider);
        while let Some((m, capture_index)) = captures.next() {
            let capture = m.captures[*capture_index];
            let name = query
                .capture_names()
                .get(capture.index as usize)
                .copied()
                .unwrap_or("");
            let Some(tag) = tag_from_capture_name(name) else {
                continue;
            };
            let r = capture.node.byte_range();
            let start = r.start.max(byte_range.start);
            let end = r.end.min(byte_range.end);
            if start < end {
                spans.push(HighlightSpan {
                    byte_range: start..end,
                    tag,
                });
            }
        }

        spans.sort_by_key(|s| (s.byte_range.start, s.byte_range.end));
        Ok(spans)
    }
}

fn tag_from_capture_name(name: &str) -> Option<HighlightTag> {
    let name = name.trim_start_matches('@');
    let head = name.split('.').next().unwrap_or(name);
    match head {
        "comment" => Some(HighlightTag::Comment),
        "string" => Some(HighlightTag::String),
        "number" => Some(HighlightTag::Number),
        "keyword" => Some(HighlightTag::Keyword),
        "type" => Some(HighlightTag::Type),
        "function" => Some(HighlightTag::Function),
        "constant" => Some(HighlightTag::Constant),
        "variable" => Some(HighlightTag::Variable),
        "property" | "field" => Some(HighlightTag::Property),
        "operator" => Some(HighlightTag::Operator),
        "punctuation" => Some(HighlightTag::Punctuation),
        "constructor" => Some(HighlightTag::Type),
        "escape" => Some(HighlightTag::String),
        "embedded" => Some(HighlightTag::String),
        _ => None,
    }
}

fn rope_chunk_from_byte(rope: &Rope, byte_offset: usize) -> &[u8] {
    if byte_offset >= rope.len_bytes() {
        return &[];
    }
    let (chunk, chunk_byte_idx, _, _) = rope.chunk_at_byte(byte_offset);
    let rel = byte_offset.saturating_sub(chunk_byte_idx);
    chunk.as_bytes().get(rel..).unwrap_or(&[])
}

struct RopeTextProvider<'a> {
    rope: &'a Rope,
}

impl<'a> TextProvider<&'a [u8]> for RopeTextProvider<'a> {
    type I = RopeNodeTextIter<'a>;

    fn text(&mut self, node: tree_sitter::Node) -> Self::I {
        let byte_range = node.byte_range();
        RopeNodeTextIter::new(self.rope, byte_range.start, byte_range.end)
    }
}

struct RopeNodeTextIter<'a> {
    rope: &'a Rope,
    pos: usize,
    end: usize,
}

impl<'a> RopeNodeTextIter<'a> {
    fn new(rope: &'a Rope, pos: usize, end: usize) -> Self {
        Self {
            rope,
            pos,
            end: end.min(rope.len_bytes()),
        }
    }
}

impl<'a> Iterator for RopeNodeTextIter<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.end {
            return None;
        }
        let (chunk, chunk_byte_idx, _, _) = self.rope.chunk_at_byte(self.pos);
        let rel = self.pos.saturating_sub(chunk_byte_idx);
        let bytes = chunk.as_bytes();
        let available = bytes.get(rel..)?;
        let take = (self.end - self.pos).min(available.len());
        let out = &available[..take];
        self.pos += take;
        Some(out)
    }
}
