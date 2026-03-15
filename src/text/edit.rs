use crate::board::TextData;

use super::ActiveTextEdit;

pub struct TextEditSession {
    element_id: u64,
    original_text: Option<TextData>,
    buffer: String,
    cursor_byte: usize,
    selection_anchor_byte: Option<usize>,
    preferred_x: Option<i32>,
}

impl TextEditSession {
    pub fn new(element_id: u64, original_text: Option<TextData>) -> Self {
        let cursor_byte = original_text
            .as_ref()
            .map(|text| text.content.len())
            .unwrap_or(0);
        let buffer = original_text
            .as_ref()
            .map(|text| text.content.clone())
            .unwrap_or_default();

        Self {
            element_id,
            original_text,
            buffer,
            cursor_byte,
            selection_anchor_byte: None,
            preferred_x: None,
        }
    }

    pub fn element_id(&self) -> u64 {
        self.element_id
    }

    pub fn original_text_cloned(&self) -> Option<TextData> {
        self.original_text.clone()
    }

    pub fn content(&self) -> &str {
        &self.buffer
    }

    pub fn cursor_byte(&self) -> usize {
        self.cursor_byte
    }

    pub fn preferred_x(&self) -> Option<i32> {
        self.preferred_x
    }

    pub fn set_preferred_x(&mut self, preferred_x: Option<i32>) {
        self.preferred_x = preferred_x;
    }

    pub fn clear_preferred_x(&mut self) {
        self.preferred_x = None;
    }

    pub fn snapshot(&self) -> TextEditSnapshot {
        TextEditSnapshot {
            element_id: self.element_id,
            content: self.buffer.clone(),
            cursor_byte: self.cursor_byte,
            selection_anchor_byte: self.selection_anchor_byte,
        }
    }

    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.selection_anchor_byte?;
        if anchor == self.cursor_byte {
            return None;
        }
        Some((anchor.min(self.cursor_byte), anchor.max(self.cursor_byte)))
    }

    pub fn clear_selection(&mut self) {
        self.selection_anchor_byte = None;
    }

    pub fn set_cursor(&mut self, cursor_byte: usize, extend_selection: bool) {
        if extend_selection {
            if self.selection_anchor_byte.is_none() {
                self.selection_anchor_byte = Some(self.cursor_byte);
            }
        } else {
            self.selection_anchor_byte = None;
        }
        self.cursor_byte = cursor_byte.min(self.buffer.len());
        if self.selection_anchor_byte == Some(self.cursor_byte) {
            self.selection_anchor_byte = None;
        }
    }

    pub fn selected_text(&self) -> Option<String> {
        let (start, end) = self.selection_range()?;
        Some(self.buffer[start..end].to_string())
    }

    pub fn select_all(&mut self) {
        self.selection_anchor_byte = Some(0);
        self.cursor_byte = self.buffer.len();
        self.preferred_x = None;
    }

    pub fn clear_all(&mut self) -> bool {
        if self.buffer.is_empty() {
            return false;
        }
        self.buffer.clear();
        self.cursor_byte = 0;
        self.clear_selection();
        self.preferred_x = None;
        true
    }

    pub fn delete_selection(&mut self) -> bool {
        let Some((start, end)) = self.selection_range() else {
            return false;
        };
        self.buffer.replace_range(start..end, "");
        self.cursor_byte = start;
        self.clear_selection();
        self.preferred_x = None;
        true
    }

    pub fn insert_text(&mut self, inserted: &str) -> bool {
        let cursor = self.cursor_byte.min(self.buffer.len());
        self.buffer.insert_str(cursor, inserted);
        self.cursor_byte = cursor + inserted.len();
        self.clear_selection();
        self.preferred_x = None;
        true
    }

    pub fn delete_backward(&mut self) -> bool {
        if self.cursor_byte == 0 {
            return false;
        }
        let previous = previous_char_boundary(&self.buffer, self.cursor_byte);
        self.buffer.replace_range(previous..self.cursor_byte, "");
        self.cursor_byte = previous;
        self.preferred_x = None;
        true
    }

    pub fn delete_forward(&mut self) -> bool {
        if self.cursor_byte >= self.buffer.len() {
            return false;
        }
        let next = next_char_boundary(&self.buffer, self.cursor_byte);
        self.buffer.replace_range(self.cursor_byte..next, "");
        self.preferred_x = None;
        true
    }
}

#[derive(Clone, PartialEq)]
pub struct TextEditSnapshot {
    pub element_id: u64,
    pub content: String,
    pub cursor_byte: usize,
    pub selection_anchor_byte: Option<usize>,
}

impl TextEditSnapshot {
    pub fn as_active_edit(&self) -> ActiveTextEdit<'_> {
        ActiveTextEdit {
            element_id: self.element_id,
            content: &self.content,
            cursor_byte: self.cursor_byte,
            selection_anchor_byte: self.selection_anchor_byte,
        }
    }
}

fn previous_char_boundary(text: &str, index: usize) -> usize {
    text[..index.min(text.len())]
        .char_indices()
        .last()
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

fn next_char_boundary(text: &str, index: usize) -> usize {
    let clamped = index.min(text.len());
    if clamped >= text.len() {
        return text.len();
    }
    let mut chars = text[clamped..].char_indices();
    let _ = chars.next();
    chars
        .next()
        .map(|(offset, _)| clamped + offset)
        .unwrap_or(text.len())
}
