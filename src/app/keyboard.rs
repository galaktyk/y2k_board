use cosmic_text::Motion;
use miniquad::{window, KeyCode, KeyMods};

use crate::board::{BoardOperation, ElementRotationChange};

#[cfg(not(target_arch = "wasm32"))]
use super::content::normalize_pasted_text;
use super::App;

impl App {
    pub(super) fn handle_key_down(&mut self, keycode: KeyCode, keymods: KeyMods) {
        self.input.shift_held = keymods.shift;
        self.input.ctrl_held = keymods.ctrl;

        if self.input.active_text_id.is_some() {
            if keycode == KeyCode::F9 && keymods.alt {
                self.flush_image_ram_cache(super::ImageRamFlushTrigger::Manual);
                return;
            }
            self.handle_text_edit_key_down(keycode, keymods);
            return;
        }

        if !keymods.ctrl && !keymods.alt {
            match keycode {
                KeyCode::Escape => {
                    self.handle_escape();
                    return;
                }
                KeyCode::Key1 => {
                    self.set_active_tool(crate::ui::tool::Tool::Select);
                    return;
                }
                KeyCode::Key2 => {
                    self.set_active_tool(crate::ui::tool::Tool::Rect);
                    return;
                }
                KeyCode::Key3 => {
                    self.set_active_tool(crate::ui::tool::Tool::Ellipse);
                    return;
                }
                KeyCode::Key4 => {
                    self.set_active_tool(crate::ui::tool::Tool::Line);
                    return;
                }
                KeyCode::Key5 => {
                    self.set_active_tool(crate::ui::tool::Tool::Sticky);
                    return;
                }
                KeyCode::Key6 => {
                    self.set_active_tool(crate::ui::tool::Tool::Text);
                    return;
                }
                KeyCode::PageUp => {
                    let mut order_changed = false;
                    let selected: Vec<_> = self.board.selected_ids().into_iter().collect();
                    for id in selected {
                        order_changed |= self.board.bring_to_front(id);
                    }
                    if order_changed {
                        self.mark_board_order_dirty();
                    }
                    return;
                }
                KeyCode::PageDown => {
                    let mut order_changed = false;
                    let selected: Vec<_> = self.board.selected_ids().into_iter().collect();
                    for id in selected.into_iter().rev() {
                        order_changed |= self.board.send_to_back(id);
                    }
                    if order_changed {
                        self.mark_board_order_dirty();
                    }
                    return;
                }
                KeyCode::Tab => {
                    if self.reset_selected_rotation() {
                        return;
                    }
                }
                _ => {}
            }
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
        if keymods.ctrl && keycode == KeyCode::C {
            self.copy_selected_to_clipboard();
            return;
        }
        if keymods.ctrl && keycode == KeyCode::X {
            self.copy_selected_to_clipboard();
            if self.board.selected_count() > 0 {
                self.board.delete_selected();
                self.mark_board_structure_dirty();
            }
            return;
        }
        #[cfg(target_arch = "wasm32")]
        if keymods.ctrl && keycode == KeyCode::V {
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if keymods.ctrl && keycode == KeyCode::V && self.handle_board_paste() {
            return;
        }
        if keycode == KeyCode::F7 && keymods.alt {
            crate::debug::spawn_debug_shapes(&mut self.board, &self.camera, self.screen_size);
            self.mark_board_structure_dirty();
            return;
        }
        if keycode == KeyCode::G && keymods.alt {
            self.input.touchpad_mode = !self.input.touchpad_mode;
            self.request_redraw();
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
            #[cfg(target_arch = "wasm32")]
            KeyCode::V if keymods.ctrl => {}
            #[cfg(not(target_arch = "wasm32"))]
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

    fn reset_selected_rotation(&mut self) -> bool {
        let ids = self.selected_ids();
        if ids.is_empty() {
            self.request_redraw();
            return false;
        }

        let changes: Vec<ElementRotationChange> = ids
            .iter()
            .filter_map(|id| {
                let element = self.board.element(*id)?;
                (element.rotation != 0.0).then_some(ElementRotationChange {
                    id: *id,
                    before: element.rotation,
                    after: 0.0,
                })
            })
            .collect();

        if changes.is_empty() {
            self.request_redraw();
            return false;
        }

        self.board
            .apply_operation(BoardOperation::SetElementRotations { changes });
        self.input.selection_bounds = None;
        self.mark_elements_dirty(ids);
        true
    }
}
