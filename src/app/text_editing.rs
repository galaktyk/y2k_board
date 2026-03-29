use cosmic_text::Motion;
use glam::Vec2;

use crate::board::{BoardOperation, ElementPropertyChange, ElementPropertyPatch, TextData};
use crate::text::TextEditSession;

use super::App;

impl App {
    pub(super) fn begin_text_edit(&mut self, id: u64) {
        let Some(element) = self.board.element(id) else {
            return;
        };
        let original_text = element.text.clone();
        self.input.active_text_id = Some(id);
        self.text_edit = Some(TextEditSession::new(id, original_text));
        crate::platform::ime::set_text_input_active(true);
        self.text_dirty = true;
        self.text_system.bump_edit_generation();
    }

    pub(super) fn finish_text_edit(&mut self, commit: bool) {
        let Some(edit) = self.text_edit.take() else {
            crate::platform::ime::set_text_input_active(false);
            self.input.active_text_id = None;
            self.input.text_selecting = false;
            return;
        };

        self.input.active_text_id = None;
        self.input.text_selecting = false;
        crate::platform::ime::set_text_input_active(false);
        self.text_dirty = true;

        if commit {
            let before = edit.original_text_cloned();
            let after = match before.clone() {
                Some(mut text) => {
                    text.content = edit.content().to_string();
                    Some(text)
                }
                None if edit.content().is_empty() => None,
                None => Some(TextData {
                    content: edit.content().to_string(),
                    ..TextData::default()
                }),
            };

            if before != after {
                self.board.apply_operation(BoardOperation::SetProperty {
                    changes: vec![ElementPropertyChange {
                        id: edit.element_id(),
                        patch: ElementPropertyPatch::Text { before, after },
                    }],
                    sync_connected_lines: true,
                });
            }
        }

        self.request_redraw();
    }

    pub(super) fn text_cursor_from_screen(&mut self, id: u64, screen_pos: Vec2) -> Option<usize> {
        let world = self.camera.screen_to_world(screen_pos, self.screen_size);
        let element = self.board.element(id)?;
        let is_active_edit = self
            .text_edit
            .as_ref()
            .map_or(false, |edit| edit.element_id() == id);
        let content = self
            .text_edit
            .as_ref()
            .filter(|edit| edit.element_id() == id)
            .map(TextEditSession::content)
            .or_else(|| element.text.as_ref().map(|text| text.content.as_str()))
            .unwrap_or_default();
        let line_offsets = self
            .text_edit
            .as_ref()
            .filter(|edit| edit.element_id() == id)
            .map(TextEditSession::line_offsets);
        self.text_system
            .hit_test_cursor(element, is_active_edit, content, line_offsets, world)
    }

    pub(super) fn set_text_cursor(&mut self, cursor_byte: usize, extend_selection: bool) {
        if let Some(edit) = self.text_edit.as_mut() {
            edit.set_cursor(cursor_byte, extend_selection);
            edit.clear_preferred_x();
        }
    }

    pub(super) fn move_text_cursor(&mut self, motion: Motion, extend_selection: bool) {
        let Some(edit) = self.text_edit.as_mut() else {
            return;
        };
        let Some(element) = self.board.element(edit.element_id()) else {
            return;
        };
        if let Some((cursor_byte, preferred_x)) = self.text_system.move_cursor(
            element,
            edit.content(),
            Some(edit.line_offsets()),
            edit.cursor_byte(),
            edit.preferred_x(),
            motion,
        ) {
            edit.set_preferred_x(preferred_x);
            edit.set_cursor(cursor_byte, extend_selection);
        }
    }

    pub(super) fn selected_text(&self) -> Option<String> {
        self.text_edit
            .as_ref()
            .and_then(TextEditSession::selected_text)
    }

    pub(super) fn current_text(&self) -> Option<&str> {
        self.text_edit.as_ref().map(TextEditSession::content)
    }

    pub(super) fn select_all_text(&mut self) {
        let Some(edit) = self.text_edit.as_mut() else {
            return;
        };
        edit.select_all();
    }

    pub(super) fn delete_selection_or_all(&mut self) -> bool {
        if self
            .text_edit
            .as_ref()
            .and_then(TextEditSession::selection_range)
            .is_some()
        {
            return self.delete_selection();
        }
        let Some(edit) = self.text_edit.as_mut() else {
            return false;
        };
        edit.clear_all()
    }

    pub(super) fn delete_selection(&mut self) -> bool {
        let Some(edit) = self.text_edit.as_mut() else {
            return false;
        };
        if !edit.delete_selection() {
            return false;
        }
        self.text_dirty = true;
        self.text_system.bump_edit_generation();
        true
    }

    pub(super) fn insert_text(&mut self, inserted: &str) -> bool {
        let _ = self.delete_selection();
        let Some(edit) = self.text_edit.as_mut() else {
            return false;
        };
        if !edit.insert_text(inserted) {
            return false;
        }
        self.text_dirty = true;
        self.text_system.bump_edit_generation();
        true
    }

    pub(super) fn delete_backward(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        let Some(edit) = self.text_edit.as_mut() else {
            return false;
        };
        if !edit.delete_backward() {
            return false;
        }
        self.text_dirty = true;
        self.text_system.bump_edit_generation();
        true
    }

    pub(super) fn delete_forward(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        let Some(edit) = self.text_edit.as_mut() else {
            return false;
        };
        if !edit.delete_forward() {
            return false;
        }
        self.text_dirty = true;
        self.text_system.bump_edit_generation();
        true
    }
}
