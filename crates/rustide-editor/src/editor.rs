use std::ops::Range;

use ropey::Rope;

use crate::Selection;

// Character-indexed editor state backed by a rope, with a simple per-edit undo/redo log.
#[derive(Debug, Clone)]
pub struct Editor {
    rope: Rope,
    selection: Selection,
    preferred_column: Option<usize>,
    history: History,
    version: u64,
    last_edit: Option<EditorEdit>,
}

impl Editor {
    pub fn empty() -> Self {
        Self::from_text("")
    }

    pub fn from_text(text: &str) -> Self {
        let rope = Rope::from_str(text);
        let selection = Selection::collapsed(0);
        Self {
            rope,
            selection,
            preferred_column: None,
            history: History::default(),
            version: 0,
            last_edit: None,
        }
    }

    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn selection(&self) -> Selection {
        self.selection
    }

    pub fn take_last_edit(&mut self) -> Option<EditorEdit> {
        self.last_edit.take()
    }

    pub fn set_cursor(&mut self, cursor: usize, extend: bool) {
        let cursor = cursor.min(self.rope.len_chars());
        self.selection.set_cursor(cursor, extend);
        if !extend {
            self.preferred_column = None;
        }
    }

    pub fn select_all(&mut self) {
        self.selection.anchor = 0;
        self.selection.cursor = self.rope.len_chars();
        self.preferred_column = None;
    }

    pub fn select_range(&mut self, range: Range<usize>) {
        let start = range.start.min(self.rope.len_chars());
        let end = range.end.min(self.rope.len_chars());
        self.selection.anchor = start;
        self.selection.cursor = end;
        self.preferred_column = None;
    }

    pub fn selected_text(&self) -> String {
        let range = self.selection.range();
        if range.is_empty() {
            return String::new();
        }
        self.rope.slice(range).to_string()
    }

    pub fn insert_text(&mut self, text: &str) {
        let range = self.selection.range();
        if text.is_empty() && range.is_empty() {
            return;
        }
        self.replace_range(range, text);
    }

    pub fn insert_newline_auto_indent(&mut self) {
        let rope = &self.rope;
        let cursor = self.selection.cursor.min(rope.len_chars());
        let line_index = rope.char_to_line(cursor);
        let line_start = rope.line_to_char(line_index);
        let line_end = line_start + self.line_visible_len(line_index);

        let base_indent = leading_indent(rope, line_start, line_end);
        let before = rope.slice(line_start..cursor.min(line_end)).to_string();
        let after = rope.slice(cursor.min(line_end)..line_end).to_string();

        let before_trim = before.trim_end_matches(|c: char| c.is_whitespace());
        let after_trim = after.trim_start_matches(|c: char| c.is_whitespace());

        let indent_unit = "    ";
        let mut next_indent = base_indent.clone();
        if before_trim.ends_with('{') {
            next_indent.push_str(indent_unit);
        }

        if after_trim.starts_with('}') {
            // If we are splitting before a closing brace, prefer decreasing indentation.
            next_indent = decrease_indent(&base_indent, indent_unit);
        }

        if before_trim.ends_with('{') && after_trim.starts_with('}') {
            // Create a two-line block:
            // {
            //     |
            // }
            let inner = format!("\n{next_indent}\n{base_indent}");
            self.insert_text(&inner);
            let new_cursor = cursor + 1 + next_indent.chars().count();
            self.set_cursor(new_cursor, false);
            return;
        }

        self.insert_text(&format!("\n{next_indent}"));
    }

    pub fn backspace(&mut self) {
        let range = self.selection.range();
        if !range.is_empty() {
            self.replace_range(range, "");
            return;
        }

        let cursor = self.selection.cursor;
        if cursor == 0 {
            return;
        }
        self.replace_range(cursor - 1..cursor, "");
    }

    pub fn delete_forward(&mut self) {
        let range = self.selection.range();
        if !range.is_empty() {
            self.replace_range(range, "");
            return;
        }

        let cursor = self.selection.cursor;
        if cursor >= self.rope.len_chars() {
            return;
        }
        self.replace_range(cursor..cursor + 1, "");
    }

    pub fn undo(&mut self) -> bool {
        let Some(edit) = self.history.undo.pop() else {
            return false;
        };
        let inserted_len = edit.inserted.chars().count();
        let info = self.apply_raw_edit(edit.start, inserted_len, &edit.deleted);
        self.selection = edit.before;
        self.preferred_column = None;
        self.history.redo.push(edit);
        self.version = self.version.wrapping_add(1);
        self.last_edit = Some(info);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(edit) = self.history.redo.pop() else {
            return false;
        };
        let deleted_len = edit.deleted.chars().count();
        let info = self.apply_raw_edit(edit.start, deleted_len, &edit.inserted);
        self.selection = edit.after;
        self.preferred_column = None;
        self.history.undo.push(edit);
        self.version = self.version.wrapping_add(1);
        self.last_edit = Some(info);
        true
    }

    pub fn move_left(&mut self, extend: bool) {
        if !extend && !self.selection.is_empty() {
            let start = self.selection.range().start;
            self.selection.collapse_to(start);
            self.preferred_column = None;
            return;
        }

        let cursor = self.selection.cursor;
        let next = cursor.saturating_sub(1);
        self.selection.set_cursor(next, extend);
        if !extend {
            self.preferred_column = None;
        }
    }

    pub fn move_right(&mut self, extend: bool) {
        if !extend && !self.selection.is_empty() {
            let end = self.selection.range().end;
            self.selection.collapse_to(end);
            self.preferred_column = None;
            return;
        }

        let cursor = self.selection.cursor;
        let next = (cursor + 1).min(self.rope.len_chars());
        self.selection.set_cursor(next, extend);
        if !extend {
            self.preferred_column = None;
        }
    }

    pub fn move_up(&mut self, extend: bool) {
        self.move_vertical(-1, extend);
    }

    pub fn move_down(&mut self, extend: bool) {
        self.move_vertical(1, extend);
    }

    pub fn move_line_start(&mut self, extend: bool) {
        let (line, _col) = self.cursor_line_col();
        let start = self.rope.line_to_char(line);
        self.selection.set_cursor(start, extend);
        if !extend {
            self.preferred_column = None;
        }
    }

    pub fn move_line_end(&mut self, extend: bool) {
        let (line, _col) = self.cursor_line_col();
        let end = self.rope.line_to_char(line) + self.line_visible_len(line);
        self.selection.set_cursor(end, extend);
        if !extend {
            self.preferred_column = None;
        }
    }

    fn replace_range(&mut self, range: Range<usize>, inserted: &str) {
        let start = range.start.min(self.rope.len_chars());
        let end = range.end.min(self.rope.len_chars());
        if start == end && inserted.is_empty() {
            return;
        }

        let before = self.selection;
        let range = start..end;
        let deleted = if start == end {
            String::new()
        } else {
            self.rope.slice(range.clone()).to_string()
        };

        let deleted_len = end - start;
        let info = self.apply_raw_edit(start, deleted_len, inserted);

        let cursor = start + inserted.chars().count();
        let after = Selection::collapsed(cursor);
        self.selection = after;
        self.preferred_column = None;

        self.history.redo.clear();
        self.history.undo.push(EditRecord {
            start,
            inserted: inserted.to_string(),
            deleted,
            before,
            after,
        });

        self.version = self.version.wrapping_add(1);
        self.last_edit = Some(info);
    }

    fn apply_raw_edit(
        &mut self,
        start: usize,
        remove_len_chars: usize,
        inserted: &str,
    ) -> EditorEdit {
        let start = start.min(self.rope.len_chars());
        let old_end = (start + remove_len_chars).min(self.rope.len_chars());

        let start_byte = self.rope.char_to_byte(start);
        let old_end_byte = self.rope.char_to_byte(old_end);
        let start_point = point_for_char(&self.rope, start);
        let old_end_point = point_for_char(&self.rope, old_end);

        let removed = start < old_end;

        if removed {
            self.rope.remove(start..old_end);
        }
        if !inserted.is_empty() {
            self.rope.insert(start, inserted);
        }

        let new_end = (start + inserted.chars().count()).min(self.rope.len_chars());
        let new_end_byte = self.rope.char_to_byte(new_end);
        let new_end_point = point_for_char(&self.rope, new_end);

        EditorEdit {
            start_byte,
            old_end_byte,
            new_end_byte,
            start_point,
            old_end_point,
            new_end_point,
        }
    }

    fn cursor_line_col(&self) -> (usize, usize) {
        let cursor = self.selection.cursor.min(self.rope.len_chars());
        let line = self.rope.char_to_line(cursor);
        let col = cursor.saturating_sub(self.rope.line_to_char(line));
        (line, col)
    }

    fn line_visible_len(&self, line: usize) -> usize {
        let slice = self.rope.line(line);
        let mut len = slice.len_chars();
        if len == 0 {
            return 0;
        }

        // Ropey lines normally include a trailing '\n' (except maybe the last line).
        if slice.char(len - 1) == '\n' {
            len -= 1;
            if len > 0 && slice.char(len - 1) == '\r' {
                len -= 1;
            }
        }
        len
    }

    fn move_vertical(&mut self, delta_lines: isize, extend: bool) {
        let cursor = self.selection.cursor.min(self.rope.len_chars());
        let (line, col) = self.cursor_line_col();

        let desired = self.preferred_column.unwrap_or(col);
        let target_line = if delta_lines.is_negative() {
            line.saturating_sub(delta_lines.unsigned_abs())
        } else {
            (line + delta_lines as usize).min(self.rope.len_lines().saturating_sub(1))
        };

        let line_start = self.rope.line_to_char(target_line);
        let line_col = desired.min(self.line_visible_len(target_line));
        let next = (line_start + line_col).min(self.rope.len_chars());

        self.selection.set_cursor(next, extend);
        self.preferred_column = Some(desired);

        if !extend && cursor == next {
            self.preferred_column = None;
        }
    }
}

fn leading_indent(rope: &Rope, start: usize, end: usize) -> String {
    let mut out = String::new();
    let mut pos = start.min(end).min(rope.len_chars());
    let end = end.min(rope.len_chars());
    while pos < end {
        let ch = rope.char(pos);
        if ch == ' ' || ch == '\t' {
            out.push(ch);
            pos += 1;
        } else {
            break;
        }
    }
    out
}

fn decrease_indent(indent: &str, indent_unit: &str) -> String {
    if let Some(stripped) = indent.strip_suffix(indent_unit) {
        stripped.to_string()
    } else if let Some(stripped) = indent.strip_suffix('\t') {
        stripped.to_string()
    } else {
        indent.to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorPoint {
    pub row: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorEdit {
    pub start_byte: usize,
    pub old_end_byte: usize,
    pub new_end_byte: usize,
    pub start_point: EditorPoint,
    pub old_end_point: EditorPoint,
    pub new_end_point: EditorPoint,
}

fn point_for_char(rope: &Rope, char_idx: usize) -> EditorPoint {
    let char_idx = char_idx.min(rope.len_chars());
    let row = rope.char_to_line(char_idx);
    let line_start_char = rope.line_to_char(row);
    let line_start_byte = rope.char_to_byte(line_start_char);
    let col = rope.char_to_byte(char_idx).saturating_sub(line_start_byte);
    EditorPoint { row, column: col }
}

#[derive(Debug, Clone)]
struct EditRecord {
    start: usize,
    inserted: String,
    deleted: String,
    before: Selection,
    after: Selection,
}

#[derive(Debug, Clone, Default)]
struct History {
    undo: Vec<EditRecord>,
    redo: Vec<EditRecord>,
}
