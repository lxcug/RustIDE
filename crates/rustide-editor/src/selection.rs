use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub anchor: usize,
    pub cursor: usize,
}

impl Selection {
    pub fn collapsed(pos: usize) -> Self {
        Self {
            anchor: pos,
            cursor: pos,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.cursor
    }

    pub fn range(&self) -> Range<usize> {
        if self.anchor <= self.cursor {
            self.anchor..self.cursor
        } else {
            self.cursor..self.anchor
        }
    }

    pub fn set_cursor(&mut self, cursor: usize, extend: bool) {
        if !extend {
            self.anchor = cursor;
        }
        self.cursor = cursor;
    }

    pub fn collapse_to(&mut self, pos: usize) {
        self.anchor = pos;
        self.cursor = pos;
    }
}
