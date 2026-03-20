use std::collections::HashMap;
use std::path::Path;

use glam::Vec2;
use miniquad::window;

use crate::board::{
    default_border_width, default_line_stroke_width, default_stroke_color, default_text_box_color,
    BoardOperation, Element, LineAnchor, LineConnectionChange, LineEndpoints, ShapeType, TextData,
    DEFAULT_TEXT_COLOR,
};
use crate::clipboard::{self, BoardClipboardData, ClipboardPaste};
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
            line_arrow_start: false,
            line_arrow_end: false,
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
        let imported = self.image_manager.import_from_source(path)?;
        self.insert_imported_image(imported, anchor, select);
        Ok(())
    }

    fn import_image_from_bytes_at(
        &mut self,
        bytes: &[u8],
        anchor: Vec2,
        select: bool,
    ) -> Result<(), ImageImportError> {
        let imported = self.image_manager.import_from_bytes(bytes)?;
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
        let imported = self
            .image_manager
            .import_from_rgba(width, height, rgba)?;
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
            line_arrow_start: false,
            line_arrow_end: false,
            selected: false,
            text: Some(text_data),
            image: None,
            text_layout_generation: 0,
        }));
        self.mark_board_structure_dirty();
        true
    }

    pub(super) fn copy_selected_to_clipboard(&mut self) {
        use crate::snapshot::snapshot_root;

        let selected_ids: std::collections::HashSet<u64> = self
            .board
            .elements
            .iter()
            .filter(|e| e.selected)
            .map(|e| e.id)
            .collect();

        if selected_ids.is_empty() {
            return;
        }

        // Collect elements, clearing selection flag.
        let elements: Vec<Element> = self
            .board
            .elements
            .iter()
            .filter(|e| selected_ids.contains(&e.id))
            .cloned()
            .map(|mut e| { e.selected = false; e })
            .collect();

        // Compute bounding-box centroid.
        let mut bb_min = Vec2::splat(f32::MAX);
        let mut bb_max = Vec2::splat(f32::MIN);
        for e in &elements {
            let (mn, mx) = e.aabb();
            bb_min = bb_min.min(mn);
            bb_max = bb_max.max(mx);
        }
        let centroid = (bb_min + bb_max) * 0.5;

        // Build line_connections: only keep anchors where target_id is also selected.
        let mut line_connections: HashMap<String, LineEndpoints> = HashMap::new();
        for (&line_id, endpoints) in &self.board.line_attachments {
            if !selected_ids.contains(&line_id) {
                continue;
            }
            let filtered_start = endpoints.start.as_ref().and_then(|a| {
                selected_ids.contains(&a.target_id).then(|| a.clone())
            });
            let filtered_end = endpoints.end.as_ref().and_then(|a| {
                selected_ids.contains(&a.target_id).then(|| a.clone())
            });
            if filtered_start.is_some() || filtered_end.is_some() {
                line_connections.insert(
                    line_id.to_string(),
                    LineEndpoints { start: filtered_start, end: filtered_end },
                );
            }
        }

        // Embed image bytes as base64.
        let asset_root = snapshot_root(&self.snapshot_path);
        let mut images: HashMap<String, String> = HashMap::new();
        for element in &elements {
            if let Some(img) = &element.image {
                for path_str in [Some(&img.asset_path), img.hires_asset_path.as_ref()]
                    .into_iter()
                    .flatten()
                {
                    if images.contains_key(path_str.as_str()) {
                        continue;
                    }
                    let full_path = asset_root.join(path_str);
                    match std::fs::read(&full_path) {
                        Ok(bytes) => {
                            use base64::Engine as _;
                            let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
                            images.insert(path_str.clone(), encoded);
                        }
                        Err(err) => {
                            eprintln!("clipboard copy: failed to read image {}: {err}", full_path.display());
                        }
                    }
                }
            }
        }

        let data = BoardClipboardData::new(centroid.to_array(), elements, line_connections, images);
        if let Err(err) = clipboard::set_board_clipboard(&data) {
            eprintln!("clipboard copy: failed to write: {err}");
        }
    }

    pub(super) fn handle_board_paste(&mut self) -> bool {
        // Check for a board-object clipboard payload first (works on all platforms).
        if let Some(text) = clipboard::get_clipboard_text() {
            if let Some(data) = clipboard::detect_board_clipboard(&text) {
                return self.paste_board_clipboard(data);
            }
        }

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

    fn paste_board_clipboard(&mut self, data: BoardClipboardData) -> bool {
        use crate::snapshot::snapshot_root;

        if data.elements.is_empty() {
            return false;
        }

        let anchor = self.paste_anchor_world();
        let centroid = Vec2::from_array(data.centroid);
        let delta = anchor - centroid;

        // Build old-ID → new-ID remap.
        let mut id_remap: HashMap<u64, u64> = HashMap::new();
        for e in &data.elements {
            id_remap.insert(e.id, self.board.next_id());
        }

        // Save embedded images to the asset root; build path remap.
        let asset_root = snapshot_root(&self.snapshot_path);
        let mut path_remap: HashMap<String, String> = HashMap::new();
        for (old_path, b64) in &data.images {
            if path_remap.contains_key(old_path) {
                continue;
            }
            use base64::Engine as _;
            let bytes = match base64::engine::general_purpose::STANDARD.decode(b64) {
                Ok(b) => b,
                Err(err) => {
                    eprintln!("clipboard paste: base64 decode failed for {old_path}: {err}");
                    continue;
                }
            };
            // Image files are content-addressed by hash; reuse the same path.
            let dest = asset_root.join(old_path);
            if dest.exists() {
                println!("[image] paste hash hit: {old_path}");
            } else {
                if let Some(parent) = dest.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Err(err) = std::fs::write(&dest, &bytes) {
                    eprintln!("clipboard paste: failed to write image {}: {err}", dest.display());
                    continue;
                }
                println!("[image] paste write: {old_path}");
            }
            path_remap.insert(old_path.clone(), old_path.clone());
        }

        // Remap and insert elements.
        self.board.deselect_all();
        let mut conn_changes: Vec<LineConnectionChange> = Vec::new();

        for mut element in data.elements {
            let new_id = match id_remap.get(&element.id) {
                Some(&id) => id,
                None => continue,
            };
            element.id = new_id;
            element.selected = true;
            element.pos += delta;

            // Update image asset paths.
            if let Some(img) = element.image.as_mut() {
                if let Some(new_path) = path_remap.get(&img.asset_path) {
                    img.asset_path = new_path.clone();
                }
                if let Some(hires) = img.hires_asset_path.as_mut() {
                    if let Some(new_path) = path_remap.get(hires.as_str()) {
                        *hires = new_path.clone();
                    }
                }
            }

            self.board.apply_operation(BoardOperation::AddElement(element));
        }

        // Build line connection changes with remapped IDs.
        for (id_str, endpoints) in &data.line_connections {
            let old_line_id: u64 = match id_str.parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let new_line_id = match id_remap.get(&old_line_id) {
                Some(&id) => id,
                None => continue,
            };
            let remap_anchor = |anchor: &LineAnchor| -> Option<LineAnchor> {
                id_remap.get(&anchor.target_id).map(|&new_target| LineAnchor {
                    target_id: new_target,
                    norm_pos: anchor.norm_pos,
                })
            };
            let new_endpoints = LineEndpoints {
                start: endpoints.start.as_ref().and_then(remap_anchor),
                end: endpoints.end.as_ref().and_then(remap_anchor),
            };
            if new_endpoints.start.is_some() || new_endpoints.end.is_some() {
                conn_changes.push(LineConnectionChange {
                    id: new_line_id,
                    before: LineEndpoints::default(),
                    after: new_endpoints,
                });
            }
        }

        if !conn_changes.is_empty() {
            self.board.apply_operation(BoardOperation::SetLineConnections { changes: conn_changes });
        }

        self.mark_board_structure_dirty();
        true
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