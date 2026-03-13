use miniquad::*;
use glam::Vec2;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::board::Board;
use crate::camera::Camera;
use crate::input::{self, DragMode, InputState};
use crate::renderer::{InstanceData, Renderer};
use crate::snapshot;
use crate::spatial::SpatialGrid;
use crate::text::TextSystem;
use crate::toolbar::{self, Toolbar, ToolbarAction};
use crate::stats;

const BOARD_VISIBILITY_MARGIN: f32 = 64.0;

#[derive(Clone, Copy)]
struct VisibleRange {
    min: Vec2,
    max: Vec2,
}

#[derive(Default)]
struct BoardRenderCache {
    all_instances: Vec<InstanceData>,
    id_by_index: Vec<u64>,
    index_by_id: HashMap<u64, usize>,
    visible_instances: Vec<InstanceData>,
    visible_board_indices: Vec<usize>,
    visible_index_by_id: HashMap<u64, usize>,
    visible_range: Option<VisibleRange>,
}

impl BoardRenderCache {
    fn rebuild_all(&mut self, board: &Board) {
        self.all_instances.clear();
        self.id_by_index.clear();
        self.index_by_id.clear();
        self.all_instances.reserve(board.elements.len());
        self.id_by_index.reserve(board.elements.len());

        for (index, element) in board.elements.iter().enumerate() {
            self.index_by_id.insert(element.id, index);
            self.id_by_index.push(element.id);
            self.all_instances.push(toolbar::element_instance(element, 1.0));
        }
    }

    fn rebuild_visible(
        &mut self,
        board: &Board,
        spatial: &SpatialGrid,
        camera: &Camera,
        screen_size: Vec2,
    ) {
        let (vis_min, vis_max) = camera.visible_rect(screen_size);
        let min = vis_min - Vec2::splat(BOARD_VISIBILITY_MARGIN);
        let max = vis_max + Vec2::splat(BOARD_VISIBILITY_MARGIN);
        let visible_ids = spatial.query(min, max);

        self.visible_instances.clear();
        self.visible_board_indices.clear();
        self.visible_index_by_id.clear();
        self.visible_range = Some(VisibleRange { min, max });

        for (board_index, element) in board.elements.iter().enumerate() {
            if visible_ids.contains(&element.id) {
                self.push_visible(board_index, element.id);
            }
        }
    }

    fn update_elements(&mut self, board: &Board, dirty_ids: &HashSet<u64>) {
        if dirty_ids.is_empty() {
            return;
        }

        let visible_range = self.visible_range;
        for &id in dirty_ids {
            let Some(&board_index) = self.index_by_id.get(&id) else {
                continue;
            };
            let element = &board.elements[board_index];
            self.all_instances[board_index] = toolbar::element_instance(element, 1.0);

            let should_be_visible = visible_range
                .map(|range| element_in_range(element, range))
                .unwrap_or(false);

            match (self.visible_index_by_id.get(&id).copied(), should_be_visible) {
                (Some(visible_index), true) => {
                    self.visible_instances[visible_index] = self.all_instances[board_index];
                }
                (Some(visible_index), false) => {
                    self.remove_visible(visible_index);
                }
                (None, true) => {
                    self.insert_visible(board_index, id);
                }
                (None, false) => {}
            }
        }
    }

    fn visible_instances(&self) -> &[InstanceData] {
        &self.visible_instances
    }

    fn visible_board_indices(&self) -> &[usize] {
        &self.visible_board_indices
    }

    fn push_visible(&mut self, board_index: usize, id: u64) {
        let visible_index = self.visible_instances.len();
        self.visible_instances.push(self.all_instances[board_index]);
        self.visible_board_indices.push(board_index);
        self.visible_index_by_id.insert(id, visible_index);
    }

    fn insert_visible(&mut self, board_index: usize, id: u64) {
        let insert_at = self
            .visible_board_indices
            .iter()
            .position(|&existing| existing > board_index)
            .unwrap_or(self.visible_board_indices.len());

        self.visible_instances
            .insert(insert_at, self.all_instances[board_index]);
        self.visible_board_indices.insert(insert_at, board_index);
        self.visible_index_by_id.insert(id, insert_at);
        self.reindex_visible_from(insert_at + 1);
    }

    fn remove_visible(&mut self, visible_index: usize) {
        let board_index = self.visible_board_indices.remove(visible_index);
        self.visible_instances.remove(visible_index);
        let id = self.id_by_index[board_index];
        self.visible_index_by_id.remove(&id);
        self.reindex_visible_from(visible_index);
    }

    fn reindex_visible_from(&mut self, start: usize) {
        for visible_index in start..self.visible_board_indices.len() {
            let board_index = self.visible_board_indices[visible_index];
            let id = self.id_by_index[board_index];
            self.visible_index_by_id.insert(id, visible_index);
        }
    }
}

fn element_in_range(element: &crate::board::Element, range: VisibleRange) -> bool {
    let (min, max) = element.aabb();
    min.x <= range.max.x
        && max.x >= range.min.x
        && min.y <= range.max.y
        && max.y >= range.min.y
}

pub struct App {
    ctx: Box<dyn RenderingBackend>,
    renderer: Renderer,
    board: Board,
    camera: Camera,
    toolbar: Toolbar,
    input: InputState,
    spatial: SpatialGrid,
    board_render_cache: BoardRenderCache,
    screen_size: Vec2,
    board_cache_dirty: bool,
    spatial_dirty: bool,
    visibility_dirty: bool,
    dirty_element_ids: HashSet<u64>,
    text_system: TextSystem,
    // ── stats ─────────────────────────────────────────────────────────────
    last_frame:   Instant,
    frame_ms:     f32,
    fps:          f32,
    fps_accum:    f32,
    fps_frames:   u32,
}

impl App {
    pub fn new() -> Self {
        let mut ctx = window::new_rendering_backend();
        let renderer = Renderer::new(&mut *ctx);
        let (w, h) = window::screen_size();
        let app = Self {
            ctx,
            renderer,
            board: Board::new(),
            camera: Camera::new(),
            toolbar: Toolbar::new(),
            input: InputState::new(),
            spatial: SpatialGrid::new(),
            board_render_cache: BoardRenderCache::default(),
            screen_size: Vec2::new(w, h),
            board_cache_dirty: true,
            spatial_dirty: true,
            visibility_dirty: true,
            dirty_element_ids: HashSet::new(),
            text_system: TextSystem::new(),
            last_frame:  Instant::now(),
            frame_ms:    0.0,
            fps:         0.0,
            fps_accum:   0.0,
            fps_frames:  0,
        };
        app.request_redraw();
        app
    }

    fn rebuild_spatial(&mut self) {
        self.spatial.clear();
        for e in &self.board.elements {
            let (min, max) = e.aabb();
            self.spatial.insert(e.id, min, max);
        }
    }

    fn request_redraw(&self) {
        window::schedule_update();
    }

    fn rebuild_board_cache(&mut self) {
        self.board_render_cache.rebuild_all(&self.board);
        self.board_cache_dirty = false;
        self.visibility_dirty = true;
    }

    fn mark_board_structure_dirty(&mut self) {
        self.board_cache_dirty = true;
        self.spatial_dirty = true;
        self.visibility_dirty = true;
        self.request_redraw();
    }

    fn mark_visibility_dirty(&mut self) {
        self.visibility_dirty = true;
        self.request_redraw();
    }

    fn mark_elements_dirty<I>(&mut self, ids: I)
    where
        I: IntoIterator<Item = u64>,
    {
        self.dirty_element_ids.extend(ids);
        self.request_redraw();
    }

    fn selected_ids(&self) -> Vec<u64> {
        self.board
            .elements
            .iter()
            .filter(|element| element.selected)
            .map(|element| element.id)
            .collect()
    }

    fn sync_board_render_cache(&mut self) {
        if self.board_cache_dirty {
            self.rebuild_board_cache();
        }

        if !self.dirty_element_ids.is_empty() {
            self.board_render_cache
                .update_elements(&self.board, &self.dirty_element_ids);
            self.dirty_element_ids.clear();
        }

        if self.spatial_dirty {
            self.rebuild_spatial();
            self.spatial_dirty = false;
        }

        if self.visibility_dirty {
            self.board_render_cache.rebuild_visible(
                &self.board,
                &self.spatial,
                &self.camera,
                self.screen_size,
            );
            self.visibility_dirty = false;
        }
    }

    fn save_snapshot(&self) {
        match snapshot::save_to_default_path(&self.board) {
            Ok(path) => println!("Saved snapshot to {}", path.display()),
            Err(err) => eprintln!("Failed to save snapshot: {err}"),
        }
    }

    fn load_snapshot(&mut self) {
        match snapshot::load_from_default_path() {
            Ok(snapshot_data) => {
                self.board
                    .restore_snapshot(snapshot_data.elements, snapshot_data.next_id);
                self.camera = Camera::new();
                self.input = InputState::new();
                self.toolbar = Toolbar::new();
                self.board_cache_dirty = true;
                self.spatial_dirty = true;
                self.visibility_dirty = true;
                self.dirty_element_ids.clear();
                self.request_redraw();
                println!("Loaded snapshot from snapshot.bin");
            }
            Err(err) => eprintln!("Failed to load snapshot: {err}"),
        }
    }

    fn handle_toolbar_action(&mut self, action: ToolbarAction) {
        match action {
            ToolbarAction::SetTool(tool) => {
                self.toolbar.active_tool = tool;
                self.request_redraw();
            }
            ToolbarAction::Save => self.save_snapshot(),
            ToolbarAction::Load => self.load_snapshot(),
            ToolbarAction::Undo => {
                self.board.undo();
                self.mark_board_structure_dirty();
            }
            ToolbarAction::Redo => {
                self.board.redo();
                self.mark_board_structure_dirty();
            }
        }
    }
}

impl EventHandler for App {
    fn update(&mut self) {}

    fn draw(&mut self) {
        // ── Frame timing ──────────────────────────────────────────────────
        let now   = Instant::now();
        let dt_ms = now.duration_since(self.last_frame).as_secs_f32() * 1000.0;
        self.last_frame = now;
        self.frame_ms   = dt_ms;

        self.fps_accum  += dt_ms;
        self.fps_frames += 1;
        if self.fps_accum >= 500.0 {
            self.fps       = self.fps_frames as f32 / (self.fps_accum / 1000.0);
            self.fps_accum  = 0.0;
            self.fps_frames = 0;
        }

        self.sync_board_render_cache();

        self.ctx.begin_default_pass(PassAction::clear_color(0.09, 0.10, 0.13, 1.0));

        self.renderer.draw_background_grid(&mut *self.ctx, &self.camera, self.screen_size);

        // Draw board elements and toolbar in separate passes since they use different MVP matrices.
        let board_mvp = Renderer::camera_mvp(&self.camera, self.screen_size);



        // Selection outlines (semi-transparent)    
        let mut selection_inst = Vec::new();
        for element in &self.board.elements {
            if let Some(instance) = toolbar::selection_instance(element, 1.0) {
                selection_inst.push(instance);
            }
        }

        
        if !selection_inst.is_empty() {
            self.renderer
                .draw_instances(&mut *self.ctx, &selection_inst, board_mvp);
        }






        // Board elements
        self.renderer.draw_instances(
            &mut *self.ctx,
            self.board_render_cache.visible_instances(),
            board_mvp,
        );

        let text_instances = self.text_system.build_visible_text_instances(
            &mut *self.ctx,
            self.renderer.text_atlas(),
            &self.board,
            self.board_render_cache.visible_board_indices(),
            &self.camera,
        );
        self.renderer
            .draw_text_instances(&mut *self.ctx, &text_instances, board_mvp);


        
        if let Some(ref preview) = self.input.preview {
            let preview_inst = toolbar::element_to_instances(preview, 0.5);
            self.renderer
                .draw_instances(&mut *self.ctx, &preview_inst, board_mvp);
        }

        // Selection handles
        let mut handle_inst = Vec::new();
        for e in &self.board.elements {
            if e.selected {
                handle_inst.extend(crate::input::handles_to_instances(e));
            }
        }
        if !handle_inst.is_empty() {
            self.renderer.draw_instances(&mut *self.ctx, &handle_inst, board_mvp);
        }

        let tb_inst = self.toolbar.build_instances(
            self.screen_size.x,
            self.board.can_undo(),
            self.board.can_redo(),
        );
        let screen_mvp = Renderer::screen_mvp(self.screen_size);

        // Toolbar (full opacity, screen-space)
        self.renderer.draw_instances(&mut *self.ctx, &tb_inst, screen_mvp);

        // ── Stats overlay ─────────────────────────────────────────────────
        let stats_inst = stats::build_stats_instances(
            self.camera.zoom,
            self.board.elements.len(),
            self.fps,
            self.frame_ms,
            self.screen_size,
        );
        self.renderer.draw_instances(&mut *self.ctx, &stats_inst, screen_mvp);

        self.ctx.end_render_pass();
        self.ctx.commit_frame();
    }

    fn mouse_button_down_event(&mut self, button: MouseButton, x: f32, y: f32) {
        if let Some(action) = input::on_mouse_down(
            &mut self.input, &mut self.board, &self.camera,
            &mut self.toolbar, self.screen_size, x, y, button,
        ) {
            self.handle_toolbar_action(action);
            return;
        }

        self.request_redraw();
    }

    fn mouse_button_up_event(&mut self, button: MouseButton, x: f32, y: f32) {
        let had_drag = self.input.drag_mode != DragMode::None;
        let had_preview = self.input.preview.is_some();
        input::on_mouse_up(
            &mut self.input, &mut self.board, &self.camera,
            &mut self.toolbar, self.screen_size, x, y, button,
        );

        if had_drag || had_preview {
            self.spatial_dirty = true;
        }
        if had_preview || self.board.elements.len() != self.board_render_cache.all_instances.len() {
            self.mark_board_structure_dirty();
            return;
        }
        self.request_redraw();
    }

    fn mouse_motion_event(&mut self, x: f32, y: f32) {
        let was_panning = self.input.panning;
        let was_dragging_tool = self.input.dragging_tool;
        input::on_mouse_move(
            &mut self.input, &mut self.board, &mut self.camera,
            &self.toolbar, self.screen_size, x, y,
        );

        if self.input.panning || was_panning {
            self.mark_visibility_dirty();
            return;
        }

        if self.input.drag_mode != DragMode::None {
            self.mark_elements_dirty(self.selected_ids());
            return;
        }

        if self.input.dragging_tool || was_dragging_tool {
            self.request_redraw();
        }
    }

    fn mouse_wheel_event(&mut self, dx: f32, dy: f32) {
        input::on_scroll(&mut self.input, &mut self.camera, self.screen_size, dx, dy);
        self.mark_visibility_dirty();
    }

    fn key_down_event(&mut self, keycode: KeyCode, keymods: KeyMods, _repeat: bool) {
        if let Some(id) = self.input.active_text_id {
            match keycode {
                KeyCode::Escape => {
                    self.input.active_text_id = None;
                    self.request_redraw();
                }
                KeyCode::Backspace => {
                    if self.edit_active_text(id, EditAction::Backspace) {
                        self.request_redraw();
                    }
                }
                KeyCode::Delete => {
                    if self.edit_active_text(id, EditAction::Delete) {
                        self.request_redraw();
                    }
                }
                KeyCode::Left => {
                    self.move_text_cursor(id, CursorMove::Left);
                    self.request_redraw();
                }
                KeyCode::Right => {
                    self.move_text_cursor(id, CursorMove::Right);
                    self.request_redraw();
                }
                KeyCode::Home => {
                    self.move_text_cursor(id, CursorMove::Home);
                    self.request_redraw();
                }
                KeyCode::End => {
                    self.move_text_cursor(id, CursorMove::End);
                    self.request_redraw();
                }
                KeyCode::Enter => {
                    if self.insert_text(id, "\n") {
                        self.request_redraw();
                    }
                }
                KeyCode::C if keymods.ctrl => {
                    if let Some(text) = self.board.element(id).and_then(|element| element.text.as_ref()) {
                        window::clipboard_set(&text.content);
                    }
                }
                KeyCode::X if keymods.ctrl => {
                    if let Some(text) = self.board.element(id).and_then(|element| element.text.as_ref()) {
                        window::clipboard_set(&text.content);
                    }
                    if self.board.update_text(id, |text| text.content.clear()) {
                        self.input.text_cursor = 0;
                        self.request_redraw();
                    }
                }
                KeyCode::V if keymods.ctrl => {
                    if let Some(clipboard) = window::clipboard_get() {
                        if self.insert_text(id, &clipboard) {
                            self.request_redraw();
                        }
                    }
                }
                _ => {}
            }
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
        if keycode == KeyCode::B && keymods.alt && keymods.ctrl {
            crate::debug::spawn_debug_shapes(&mut self.board, &self.camera, self.screen_size);
            self.mark_board_structure_dirty();
            return;
        }

        let mut board_changed = false;
        if matches!(keycode, KeyCode::Delete | KeyCode::Backspace) {
            board_changed = self.board.elements.iter().any(|element| element.selected);
        }
        if keymods.ctrl && matches!(keycode, KeyCode::Z | KeyCode::Y) {
            board_changed = true;
        }

        input::on_key_down(&mut self.input, &mut self.board, keycode, keymods);
        if board_changed {
            self.mark_board_structure_dirty();
        } else {
            self.request_redraw();
        }
    }

    fn key_up_event(&mut self, keycode: KeyCode, _keymods: KeyMods) {
        if keycode == KeyCode::Space {
            self.input.space_held = false;
        }
    }

    fn char_event(&mut self, character: char, _keymods: KeyMods, repeat: bool) {
        if repeat || character.is_control() {
            return;
        }
        let Some(id) = self.input.active_text_id else {
            return;
        };

        let text = character.to_string();
        if self.insert_text(id, &text) {
            self.request_redraw();
        }
    }

    fn resize_event(&mut self, width: f32, height: f32) {
        self.screen_size = Vec2::new(width, height);
        self.visibility_dirty = true;
        self.request_redraw();
    }
}

enum EditAction {
    Backspace,
    Delete,
}

enum CursorMove {
    Left,
    Right,
    Home,
    End,
}

impl App {
    fn active_text_chars(&self, id: u64) -> usize {
        self.board
            .element(id)
            .and_then(|element| element.text.as_ref())
            .map(|text| text.content.chars().count())
            .unwrap_or(0)
    }

    fn insert_text(&mut self, id: u64, inserted: &str) -> bool {
        let cursor = self.input.text_cursor;
        let updated = self.board.update_text(id, |text| {
            let byte_index = char_to_byte_index(&text.content, cursor);
            text.content.insert_str(byte_index, inserted);
        });
        if updated {
            self.input.text_cursor += inserted.chars().count();
        }
        updated
    }

    fn edit_active_text(&mut self, id: u64, action: EditAction) -> bool {
        let cursor = self.input.text_cursor;
        let updated = self.board.update_text(id, |text| match action {
            EditAction::Backspace => {
                if cursor == 0 {
                    return;
                }
                let start = char_to_byte_index(&text.content, cursor - 1);
                let end = char_to_byte_index(&text.content, cursor);
                text.content.replace_range(start..end, "");
            }
            EditAction::Delete => {
                let total = text.content.chars().count();
                if cursor >= total {
                    return;
                }
                let start = char_to_byte_index(&text.content, cursor);
                let end = char_to_byte_index(&text.content, cursor + 1);
                text.content.replace_range(start..end, "");
            }
        });

        if updated {
            if matches!(action, EditAction::Backspace) && self.input.text_cursor > 0 {
                self.input.text_cursor -= 1;
            }
        }
        updated
    }

    fn move_text_cursor(&mut self, id: u64, movement: CursorMove) {
        let len = self.active_text_chars(id);
        self.input.text_cursor = match movement {
            CursorMove::Left => self.input.text_cursor.saturating_sub(1),
            CursorMove::Right => (self.input.text_cursor + 1).min(len),
            CursorMove::Home => 0,
            CursorMove::End => len,
        };
    }
}

fn char_to_byte_index(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(text.len())
}

