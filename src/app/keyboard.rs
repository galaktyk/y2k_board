use cosmic_text::Motion;
use miniquad::{window, KeyCode, KeyMods};

use super::content::normalize_pasted_text;
use super::App;

impl App {
    pub(super) fn handle_key_down(&mut self, keycode: KeyCode, keymods: KeyMods) {
        self.input.shift_held = keymods.shift;
        self.input.ctrl_held = keymods.ctrl;

        if self.input.active_text_id.is_some() {
            self.handle_text_edit_key_down(keycode, keymods);
            return;
        }

        if keycode == KeyCode::Space {
            self.input.space_held = true;
        }
        if keymods.ctrl && keycode == KeyCode::S {
            self.save_snapshot();
            return;
        }
        if keymods.ctrl && keycode == KeyCode::O {
            self.load_snapshot();
            return;
        }
        if keymods.ctrl && keycode == KeyCode::V && self.handle_board_paste() {
            return;
        }
        if keycode == KeyCode::F7 && keymods.alt {
            crate::debug::spawn_debug_shapes(&mut self.board, &self.camera, self.screen_size);
            self.mark_board_structure_dirty();
            return;
        }
        if keycode == KeyCode::F8 && keymods.alt {
            match crate::debug::spawn_debug_images(
                &mut self.board,
                &self.camera,
                self.screen_size,
                &mut self.image_manager,
            ) {
                Ok(_) => self.mark_board_structure_dirty(),
                Err(err) => {
                    eprintln!("Failed to spawn debug images: {err}");
                    self.request_redraw();
                }
            }
            return;
        }

        let mut board_changed = false;
        if matches!(keycode, KeyCode::Delete | KeyCode::Backspace) {
            board_changed = self.board.elements.iter().any(|element| element.selected);
        }
        if keymods.ctrl && matches!(keycode, KeyCode::Z | KeyCode::Y) {
            board_changed = true;
        }

        crate::input::on_key_down(&mut self.input, &mut self.board, keycode, keymods);
        if board_changed {
            self.mark_board_structure_dirty();
        } else {
            self.request_redraw();
        }
    }

    pub(super) fn handle_char_input(&mut self, character: char, repeat: bool) {
        if repeat || character.is_control() || self.input.active_text_id.is_none() {
            return;
        }

        let text = character.to_string();
        if self.insert_text(&text) {
            self.request_redraw();
        }
    }

    fn handle_text_edit_key_down(&mut self, keycode: KeyCode, keymods: KeyMods) {
        match keycode {
            KeyCode::Escape => {
                self.finish_text_edit(true);
            }
            KeyCode::Backspace => {
                if self.delete_backward() {
                    self.request_redraw();
                }
            }
            KeyCode::Delete => {
                if self.delete_forward() {
                    self.request_redraw();
                }
            }
            KeyCode::Left => self.move_text_cursor_and_redraw(Motion::Left, keymods.shift),
            KeyCode::Right => self.move_text_cursor_and_redraw(Motion::Right, keymods.shift),
            KeyCode::Up => self.move_text_cursor_and_redraw(Motion::Up, keymods.shift),
            KeyCode::Down => self.move_text_cursor_and_redraw(Motion::Down, keymods.shift),
            KeyCode::Home => self.move_text_cursor_and_redraw(Motion::Home, keymods.shift),
            KeyCode::End => self.move_text_cursor_and_redraw(Motion::End, keymods.shift),
            KeyCode::Enter => {
                if self.insert_text("\n") {
                    self.request_redraw();
                }
            }
            KeyCode::A if keymods.ctrl => {
                self.select_all_text();
                self.request_redraw();
            }
            KeyCode::C if keymods.ctrl => {
                if let Some(text) = self.selected_or_current_text() {
                    window::clipboard_set(&text);
                }
            }
            KeyCode::X if keymods.ctrl => {
                if let Some(text) = self.selected_or_current_text() {
                    window::clipboard_set(&text);
                }
                if self.delete_selection_or_all() {
                    self.request_redraw();
                }
            }
            KeyCode::V if keymods.ctrl => {
                if let Some(clipboard) = window::clipboard_get() {
                    let clipboard = normalize_pasted_text(&clipboard);
                    if self.insert_text(&clipboard) {
                        self.request_redraw();
                    }
                }
            }
            _ => {}
        }
    }

    fn move_text_cursor_and_redraw(&mut self, motion: Motion, extend_selection: bool) {
        self.move_text_cursor(motion, extend_selection);
        self.request_redraw();
    }

    fn selected_or_current_text(&self) -> Option<String> {
        self.selected_text()
            .or_else(|| self.current_text().map(str::to_string))
    }
}