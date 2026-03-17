use std::path::Path;

use glam::Vec2;
use miniquad::window;

use crate::board::{
    default_border_width, default_line_stroke_width, default_stroke_color, default_text_box_color,
    BoardOperation, Element, ShapeType, TextData, DEFAULT_TEXT_COLOR,
};
use crate::clipboard::{self, ClipboardPaste};
use crate::images::{ImageImportError, ImportedImage};
use crate::rendering::renderer::PreparedImageDraw;
use crate::rendering::cache::element_in_expanded_view;

use super::App;

impl App {
    pub(super) fn import_image_via_dialog(&mut self) {
        #[cfg(target_arch = "wasm32")]
        {
            eprintln!("Image import is only implemented for native desktop builds");
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(path) = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp", "gif"])
                .pick_file()
            else {
                return;
            };

            match self.import_image_from_path_at(&path, self.camera.pan, true) {
                Ok(()) => {}
                Err(err) => {
                    eprintln!("Failed to import image: {err}");
                    self.request_redraw();
                }
            }
        }
    }

    fn viewport_image_size(&self, display_size: [f32; 2]) -> Vec2 {
        let mut size = Vec2::from_array(display_size);
        let viewport_world = Vec2::new(
            self.screen_size.x / self.camera.zoom.max(0.0001),
            self.screen_size.y / self.camera.zoom.max(0.0001),
        ) * 0.6;
        let scale = (viewport_world.x / size.x)
            .min(viewport_world.y / size.y)
            .min(1.0);
        size *= scale.max(0.01);
        size
    }

    fn paste_anchor_world(&self) -> Vec2 {
        let mouse = self.input.mouse_pos;
        if mouse.x >= 0.0
            && mouse.y >= 0.0
            && mouse.x <= self.screen_size.x
            && mouse.y <= self.screen_size.y
        {
            self.camera.screen_to_world(mouse, self.screen_size)
        } else {
            self.camera.pan
        }
    }

    fn insert_imported_image(&mut self, imported: ImportedImage, anchor: Vec2, select: bool) {
        let new_id = self.board.next_id();
        let size = self.viewport_image_size(imported.display_size);
        let element = Element {
            id: new_id,
            shape: ShapeType::Image,
            pos: anchor - size * 0.5,
            size,
            rotation: 0.0,
            color: [1.0, 1.0, 1.0, 1.0],
            stroke_color: default_stroke_color(),
            border_width: default_border_width(),
            stroke_width: default_line_stroke_width(),
            selected: false,
            text: None,
            image: Some(imported.data),
            text_layout_generation: 0,
        };

        self.board.apply_operation(BoardOperation::AddElement(element));
        if select {
            self.board.deselect_all();
            self.board.select_only(new_id);
        }
        self.mark_board_structure_dirty();
    }

    fn import_image_from_path_at(
        &mut self,
        path: &Path,
        anchor: Vec2,
        select: bool,
    ) -> Result<(), ImageImportError> {
        let element_id = self.board.next_available_id();
        let imported = self.image_manager.import_from_source(element_id, path)?;
        self.insert_imported_image(imported, anchor, select);
        Ok(())
    }

    fn import_image_from_bytes_at(
        &mut self,
        bytes: &[u8],
        anchor: Vec2,
        select: bool,
    ) -> Result<(), ImageImportError> {
        let element_id = self.board.next_available_id();
        let imported = self.image_manager.import_from_bytes(element_id, bytes)?;
        self.insert_imported_image(imported, anchor, select);
        Ok(())
    }

    fn import_image_from_rgba_at(
        &mut self,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
        anchor: Vec2,
        select: bool,
    ) -> Result<(), ImageImportError> {
        let element_id = self.board.next_available_id();
        let imported = self
            .image_manager
            .import_from_rgba(element_id, width, height, rgba)?;
        self.insert_imported_image(imported, anchor, select);
        Ok(())
    }

    fn insert_pasted_text_box(&mut self, text: &str) -> bool {
        let content = normalize_pasted_text(text);
        if content.is_empty() {
            return false;
        }

        let text_data = TextData {
            content,
            font_size: 24.0,
            color: DEFAULT_TEXT_COLOR,
        };
        let max_width = (self.screen_size.x / self.camera.zoom.max(0.0001) * 0.5).max(180.0);
        let size = self
            .text_system
            .measure_text_box(&text_data.content, &text_data, max_width);
        let anchor = self.paste_anchor_world();
        let new_id = self.board.next_id();

        self.board.apply_operation(BoardOperation::AddElement(Element {
            id: new_id,
            shape: ShapeType::Rect,
            pos: anchor - size * 0.5,
            size,
            rotation: 0.0,
            color: default_text_box_color(),
            stroke_color: default_stroke_color(),
            border_width: default_border_width(),
            stroke_width: default_line_stroke_width(),
            selected: false,
            text: Some(text_data),
            image: None,
            text_layout_generation: 0,
        }));
        self.mark_board_structure_dirty();
        true
    }

    pub(super) fn handle_board_paste(&mut self) -> bool {
        #[cfg(all(target_os = "windows", not(target_arch = "wasm32")))]
        {
            let anchor = self.paste_anchor_world();
            match clipboard::preferred_paste_contents() {
                Ok(Some(ClipboardPaste::Image(image))) => {
                    match self.import_image_from_rgba_at(
                        image.width,
                        image.height,
                        image.rgba,
                        anchor,
                        false,
                    ) {
                        Ok(()) => return true,
                        Err(err) => {
                            eprintln!("Failed to paste image: {err}");
                            return true;
                        }
                    }
                }
                Ok(Some(ClipboardPaste::Text(text))) => {
                    return self.insert_pasted_text_box(&text);
                }
                Ok(None) => {}
                Err(err) => {
                    eprintln!("Failed to read clipboard: {err}");
                    return true;
                }
            }
        }

        if let Some(clipboard) = window::clipboard_get() {
            return self.insert_pasted_text_box(&clipboard);
        }

        false
    }

    pub(super) fn import_dropped_files(&mut self) {
        let count = window::dropped_file_count();
        if count == 0 {
            return;
        }

        let base_anchor = self.paste_anchor_world();
        let mut imported_any = false;
        for index in 0..count {
            let offset = Vec2::splat(index as f32 * 24.0);
            let anchor = base_anchor + offset;

            let result = if let Some(bytes) = window::dropped_file_bytes(index) {
                self.import_image_from_bytes_at(&bytes, anchor, false)
            } else if let Some(path) = window::dropped_file_path(index) {
                self.import_image_from_path_at(&path, anchor, false)
            } else {
                continue;
            };

            match result {
                Ok(()) => imported_any = true,
                Err(err) => eprintln!("Failed to import dropped image: {err}"),
            }
        }

        if !imported_any {
            self.request_redraw();
        }
    }

    pub(super) fn build_image_draws(
        &mut self,
    ) -> Vec<PreparedImageDraw> {
        let pending: Vec<(crate::board::ImageData, Vec2, Vec2, f32, bool)> = self
            .board
            .elements
            .iter()
            .filter_map(|element| {
                if element.shape != ShapeType::Image {
                    return None;
                }

                let image = element.image.clone()?;
                let pos = element.pos;
                let size = element.size;
                let rotation = element.rotation;
                let selected = element.selected;

                element_in_expanded_view(&self.camera, self.screen_size, element)
                    .then_some((image, pos, size, rotation, selected))
            })
            .collect();

        for (image, _, _, _, _) in &pending {
            self.image_manager
                .preload_thumb(&mut *self.ctx, &image.asset_path);
        }

        pending
            .into_iter()
            .map(|(image, pos, size, rotation, selected)| {
                self.image_manager.prepare_draw(
                    &mut *self.ctx,
                    &image,
                    pos.to_array(),
                    size.to_array(),
                    rotation,
                    self.camera.zoom,
                    [size.x * self.camera.zoom, size.y * self.camera.zoom],
                    self.screen_size.to_array(),
                    selected,
                )
            })
            .collect()
    }
}

pub(super) fn normalize_pasted_text(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}