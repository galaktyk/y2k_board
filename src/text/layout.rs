use cosmic_text::{Buffer, Cursor};

#[derive(Clone, Default, PartialEq, Eq)]
pub struct LineOffsets {
    /// Byte offset of the first character on each line.
    starts: Vec<usize>,
    /// Byte offset one past the last character on each line (excluding `\n`).
    ends: Vec<usize>,
}

impl LineOffsets {
    pub fn build(text: &str) -> Self {
        let mut starts = Vec::new();
        let mut ends = Vec::new();
        let mut offset = 0usize;
        for segment in text.split('\n') {
            starts.push(offset);
            ends.push(offset + segment.len());
            offset += segment.len() + 1;
        }
        if starts.is_empty() {
            starts.push(0);
            ends.push(0);
        }
        Self { starts, ends }
    }

    pub fn byte_to_cursor(&self, text: &str, global_byte: usize) -> Cursor {
        let target = global_byte.min(text.len());
        let line = self
            .starts
            .partition_point(|&s| s <= target)
            .saturating_sub(1);
        Cursor::new(line, target - self.starts[line])
    }

    pub fn cursor_to_byte(&self, text: &str, cursor: Cursor) -> usize {
        match self.starts.get(cursor.line) {
            Some(&line_start) => {
                let segment_len = self.ends[cursor.line] - line_start;
                (line_start + cursor.index.min(segment_len)).min(text.len())
            }
            None => text.len(),
        }
    }
}

pub fn global_byte_to_cursor(text: &str, global_byte: usize) -> Cursor {
    LineOffsets::build(text).byte_to_cursor(text, global_byte)
}

pub fn cursor_to_global_byte(text: &str, cursor: Cursor) -> usize {
    LineOffsets::build(text).cursor_to_byte(text, cursor)
}

pub fn selection_range(cursor_byte: usize, anchor_byte: Option<usize>) -> Option<(usize, usize)> {
    let anchor_byte = anchor_byte?;
    if anchor_byte == cursor_byte {
        return None;
    }
    Some((anchor_byte.min(cursor_byte), anchor_byte.max(cursor_byte)))
}

pub fn caret_geometry(buffer: &Buffer, cursor: Cursor) -> Option<(f32, f32, f32)> {
    let mut last_matching_run = None;

    for run in buffer.layout_runs() {
        if run.line_i != cursor.line {
            continue;
        }
        last_matching_run = Some((
            run.line_w,
            run.line_top,
            run.line_height,
            run.glyphs.is_empty(),
        ));
        // Handle cursor at the very beginning (leftmost edge)
        if cursor.index == 0 {
            return Some((0.0, run.line_top, run.line_height));
        }
        if let Some((x, _)) = run.highlight(cursor, cursor) {
            return Some((x, run.line_top, run.line_height));
        }
        if run.glyphs.is_empty() {
            return Some((0.0, run.line_top, run.line_height));
        }
    }

    last_matching_run.map(|(line_w, line_top, line_height, glyphs_empty)| {
        let x = if glyphs_empty { 0.0 } else { line_w };
        (x, line_top, line_height)
    })
}
