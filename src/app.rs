use miniquad::*;
use glam::Vec2;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use cosmic_text::Motion;

use crate::board::{Board, BoardOperation, Element, ElementPropertyChange, ElementPropertyPatch, ShapeType, TextData};
use crate::camera::Camera;
use crate::clipboard::{self, ClipboardPaste};
use crate::images::{ImageImportError, ImageManager, ImportedImage};
use crate::input::{self, DragMode, InputState};
use crate::renderer::{InstanceData, PreparedImageDraw, Renderer};
use crate::snapshot;
use crate::spatial::SpatialGrid;
use crate::text::{ActiveTextEdit, PreparedTextDraw, TextSystem};
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
    ) -> bool {
        let (vis_min, vis_max) = camera.visible_rect(screen_size);
        let min = vis_min - Vec2::splat(BOARD_VISIBILITY_MARGIN);
        let max = vis_max + Vec2::splat(BOARD_VISIBILITY_MARGIN);
        let visible_ids = spatial.query(min, max);

        let previous_indices = self.visible_board_indices.clone();

        self.visible_instances.clear();
        self.visible_board_indices.clear();
        self.visible_index_by_id.clear();
        self.visible_range = Some(VisibleRange { min, max });

        for (board_index, element) in board.elements.iter().enumerate() {
            if visible_ids.contains(&element.id) {
                self.push_visible(board_index, element.id);
            }
        }

        self.visible_board_indices != previous_indices
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

    fn all_instances(&self) -> &[InstanceData] {
        &self.all_instances
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

fn offset_instance(mut instance: InstanceData, delta: Vec2) -> InstanceData {
    instance.pos[0] += delta.x;
    instance.pos[1] += delta.y;
    instance
}

fn rotate_point(point: Vec2, center: Vec2, angle: f32) -> Vec2 {
    let offset = point - center;
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    center + Vec2::new(
        offset.x * cos_a - offset.y * sin_a,
        offset.x * sin_a + offset.y * cos_a,
    )
}

fn rotate_instance(mut instance: InstanceData, center: Vec2, angle: f32) -> InstanceData {
    if instance.shape_type == 2 {
        let start = Vec2::new(instance.pos[0], instance.pos[1]);
        let end = start + Vec2::new(instance.size[0], instance.size[1]);
        let rotated_start = rotate_point(start, center, angle);
        let rotated_end = rotate_point(end, center, angle);
        instance.pos = rotated_start.to_array();
        instance.size = (rotated_end - rotated_start).to_array();
        return instance;
    }

    let original_center = Vec2::new(instance.pos[0], instance.pos[1])
        + Vec2::new(instance.size[0], instance.size[1]) * 0.5;
    let rotated_center = rotate_point(original_center, center, angle);
    let size = Vec2::new(instance.size[0], instance.size[1]);
    instance.pos = (rotated_center - size * 0.5).to_array();
    instance.rotation += angle;
    instance
}

fn offset_text_instance(mut instance: crate::renderer::TextInstanceData, delta: Vec2) -> crate::renderer::TextInstanceData {
    instance.pos[0] += delta.x;
    instance.pos[1] += delta.y;
    instance.origin[0] = instance.origin[0].saturating_add(delta.x.round() as i16);
    instance.origin[1] = instance.origin[1].saturating_add(delta.y.round() as i16);
    instance
}

fn rotate_text_instance(
    mut instance: crate::renderer::TextInstanceData,
    center: Vec2,
    angle: f32,
) -> crate::renderer::TextInstanceData {
    let original_origin = Vec2::new(instance.origin[0] as f32, instance.origin[1] as f32);
    let origin = rotate_point(
        original_origin,
        center,
        angle,
    );
    let pos = Vec2::new(instance.pos[0], instance.pos[1]) + (origin - original_origin);

    instance.pos = pos.to_array();
    instance.origin = [
        origin.x.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16,
        origin.y.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16,
    ];
    instance.rotation += angle;
    instance
}

fn preview_rect_transform(
    element: &crate::board::Element,
    move_drag_offset: Option<Vec2>,
    rotate_drag_preview: Option<(f32, Vec2)>,
) -> (Vec2, Vec2, f32) {
    if let Some((angle, center)) = rotate_drag_preview.filter(|_| element.selected) {
        let original_center = element.pos + element.size * 0.5;
        let rotated_center = rotate_point(original_center, center, angle);
        (
            rotated_center - element.size * 0.5,
            element.size,
            element.rotation + angle,
        )
    } else if let Some(offset) = move_drag_offset.filter(|_| element.selected) {
        (element.pos + offset, element.size, element.rotation)
    } else {
        (element.pos, element.size, element.rotation)
    }
}

struct TextEditSession {
    element_id: u64,
    original_text: Option<TextData>,
    buffer: String,
    cursor_byte: usize,
    selection_anchor_byte: Option<usize>,
    preferred_x: Option<i32>,
}

impl TextEditSession {
    fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.selection_anchor_byte?;
        if anchor == self.cursor_byte {
            return None;
        }
        Some((anchor.min(self.cursor_byte), anchor.max(self.cursor_byte)))
    }

    fn clear_selection(&mut self) {
        self.selection_anchor_byte = None;
    }

    fn set_cursor(&mut self, cursor_byte: usize, extend_selection: bool) {
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
}

/// Lightweight snapshot of the active text edit state for cache comparison.
#[derive(Clone, PartialEq)]
struct TextEditSnapshot {
    element_id: u64,
    content: String,
    cursor_byte: usize,
    selection_anchor_byte: Option<usize>,
}

pub struct App {
    ctx: Box<dyn RenderingBackend>,
    renderer: Renderer,
    board: Board,
    snapshot_path: PathBuf,
    camera: Camera,
    toolbar: Toolbar,
    input: InputState,
    spatial: SpatialGrid,
    board_render_cache: BoardRenderCache,
    screen_size: Vec2,
    board_cache_dirty: bool,
    board_scene_dirty: bool,
    spatial_dirty: bool,
    visibility_dirty: bool,
    dirty_element_ids: HashSet<u64>,
    text_system: TextSystem,
    image_manager: ImageManager,
    text_edit: Option<TextEditSession>,
    // ── text cache ────────────────────────────────────────────────────────
    cached_text_draw: Option<PreparedTextDraw>,
    text_dirty: bool,
    cached_text_edit_snapshot: Option<TextEditSnapshot>,
    // ── stats ─────────────────────────────────────────────────────────────
    last_frame:   f64,
    frame_ms:     f32,
    fps:          f32,
    fps_accum:    f32,
    fps_frames:   u32,
}

impl App {
    pub fn new() -> Self {
        let mut ctx = window::new_rendering_backend();
        let renderer = Renderer::new(&mut *ctx);
        let snapshot_path = snapshot::default_snapshot_path();
        let asset_root = snapshot::snapshot_root(&snapshot_path);
        let image_manager = ImageManager::new(&mut *ctx, asset_root);
        let (w, h) = window::screen_size();
        let app = Self {
            ctx,
            renderer,
            board: Board::new(),
            snapshot_path,
            camera: Camera::new(),
            toolbar: Toolbar::new(),
            input: InputState::new(),
            spatial: SpatialGrid::new(),
            board_render_cache: BoardRenderCache::default(),
            screen_size: Vec2::new(w, h),
            board_cache_dirty: true,
            board_scene_dirty: true,
            spatial_dirty: true,
            visibility_dirty: true,
            dirty_element_ids: HashSet::new(),
            text_system: TextSystem::new(),
            image_manager,
            text_edit: None,
            cached_text_draw: None,
            text_dirty: true,
            cached_text_edit_snapshot: None,
            last_frame:  miniquad::date::now(),
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

    fn needs_continuous_redraw(&self) -> bool {
        self.input.panning
            || self.input.drag_mode != DragMode::None
            || self.input.dragging_tool
    }

    fn rebuild_board_cache(&mut self) {
        self.board_render_cache.rebuild_all(&self.board);
        self.board_cache_dirty = false;
        self.board_scene_dirty = true;
        self.visibility_dirty = true;
        // Evict stale layout cache entries for deleted elements
        let live_ids: HashSet<u64> = self.board.elements.iter().map(|e| e.id).collect();
        self.text_system.evict_stale_layouts(&live_ids);
    }

    fn mark_board_structure_dirty(&mut self) {
        self.board_cache_dirty = true;
        self.board_scene_dirty = true;
        self.spatial_dirty = true;
        self.visibility_dirty = true;
        self.text_dirty = true;
        self.request_redraw();
    }

    fn mark_elements_dirty<I>(&mut self, ids: I)
    where
        I: IntoIterator<Item = u64>,
    {
        self.dirty_element_ids.extend(ids);
        self.board_scene_dirty = true;
        self.text_dirty = true;
        self.request_redraw();
    }

    fn selected_ids(&self) -> Vec<u64> {
        self.board.selected_ids()
    }

    fn sync_board_render_cache(&mut self) {

        if self.board_cache_dirty {
            self.rebuild_board_cache();
        }

        if !self.dirty_element_ids.is_empty() {
            self.board_render_cache
                .update_elements(&self.board, &self.dirty_element_ids);
            self.dirty_element_ids.clear();
            self.board_scene_dirty = true;
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

    fn save_snapshot(&mut self) {
        let asset_root = snapshot::snapshot_root(&self.snapshot_path);
        self.image_manager.set_asset_root(&mut *self.ctx, asset_root);
        match snapshot::save_to_path(&self.board, &self.snapshot_path) {
            Ok(path) => println!("Saved snapshot to {}", path.display()),
            Err(err) => eprintln!("Failed to save snapshot: {err}"),
        }
    }

    fn load_snapshot(&mut self) {
        match snapshot::load_from_path(&self.snapshot_path) {
            Ok(loaded) => {
                self.snapshot_path = loaded.path.clone();
                let asset_root = snapshot::snapshot_root(&self.snapshot_path);
                self.image_manager.set_asset_root(&mut *self.ctx, asset_root);
                self.board
                    .restore_snapshot(loaded.data.elements, loaded.data.next_id);
                self.camera = Camera::new();
                self.input = InputState::new();
                self.toolbar = Toolbar::new();
                self.board_cache_dirty = true;
                self.spatial_dirty = true;
                self.visibility_dirty = true;
                self.dirty_element_ids.clear();
                self.text_edit = None;
                self.text_dirty = true;
                self.cached_text_draw = None;
                self.cached_text_edit_snapshot = None;
                self.request_redraw();
                println!("Loaded snapshot from {}", self.snapshot_path.display());
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
            ToolbarAction::ImportImage => self.import_image_via_dialog(),
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
        let now   = miniquad::date::now();
        let dt_ms = ((now - self.last_frame) * 1000.0) as f32;
        self.last_frame = now;
        self.frame_ms   = dt_ms;

        self.fps_accum  += dt_ms;
        self.fps_frames += 1;
        if self.fps_accum >= 500.0 {
            self.fps       = self.fps_frames as f32 / (self.fps_accum / 1000.0);
            self.fps_accum  = 0.0;
            self.fps_frames = 0;
        }

        // Capture visibility state BEFORE sync clears it
        self.sync_board_render_cache();

        if self.board_scene_dirty {
            self.renderer
                .upload_scene_instances(&mut *self.ctx, self.board_render_cache.all_instances());
            self.board_scene_dirty = false;
        }

        let move_drag_offset = (self.input.drag_mode == DragMode::MoveSelected)
            .then_some(self.input.move_delta)
            .filter(|delta| delta.length_squared() > 0.0);
        let rotate_drag_preview = (self.input.drag_mode == DragMode::Rotating
            && self.input.move_origin.len() > 1)
            .then_some(self.input.rotate_delta)
            .filter(|angle| angle.abs() > 0.0)
            .zip(self.input.transform_bounds_origin.map(|bounds| bounds.center()));
        let image_draws = self.build_image_draws(move_drag_offset, rotate_drag_preview);

        self.ctx.begin_default_pass(PassAction::clear_color(0.09, 0.10, 0.13, 1.0));

        self.renderer.draw_background_grid(&mut *self.ctx, &self.camera, self.screen_size);

        // Draw board elements and toolbar in separate passes since they use different MVP matrices.
        let board_mvp = Renderer::camera_mvp(&self.camera, self.screen_size);

        // Board elements
        let rotated_shape_instances;
        let moved_shape_instances;
        let shape_instances = if let Some((angle, center)) = rotate_drag_preview {
            rotated_shape_instances = self
                .board_render_cache
                .all_instances()
                .iter()
                .enumerate()
                .map(|(board_index, &instance)| {
                    if self.board.elements[board_index].selected {
                        rotate_instance(instance, center, angle)
                    } else {
                        instance
                    }
                })
                .collect::<Vec<_>>();
            rotated_shape_instances.as_slice()
        } else if let Some(offset) = move_drag_offset {
            moved_shape_instances = self
                .board_render_cache
                .all_instances()
                .iter()
                .enumerate()
                .map(|(board_index, &instance)| {
                    if self.board.elements[board_index].selected {
                        offset_instance(instance, offset)
                    } else {
                        instance
                    }
                })
                .collect::<Vec<_>>();
            moved_shape_instances.as_slice()
        } else {
            &[]
        };
        if rotate_drag_preview.is_some() || move_drag_offset.is_some() {
            self.renderer
                .draw_instances(&mut *self.ctx, shape_instances, board_mvp, self.screen_size);
        } else {
            self.renderer
                .draw_scene_instances(&mut *self.ctx, board_mvp, self.screen_size);
        }
        self.renderer
            .draw_image_draws(&mut *self.ctx, &image_draws, board_mvp);

        // Build current edit snapshot for cache comparison
        let current_edit_snapshot = self.text_edit.as_ref().map(|edit| TextEditSnapshot {
            element_id: edit.element_id,
            content: edit.buffer.clone(),
            cursor_byte: edit.cursor_byte,
            selection_anchor_byte: edit.selection_anchor_byte,
        });

        // Check if we can reuse cached text draw
        let text_cache_valid = !self.text_dirty
            && self.cached_text_draw.is_some()
            && self.cached_text_edit_snapshot == current_edit_snapshot;

        if text_cache_valid {
            // FAST PATH: reuse cached PreparedTextDraw
        } else {
            // SLOW PATH: rebuild
            let active_text_edit = current_edit_snapshot.as_ref().map(|snap| ActiveTextEdit {
                element_id: snap.element_id,
                content: &snap.content,
                cursor_byte: snap.cursor_byte,
                selection_anchor_byte: snap.selection_anchor_byte,
            });

            let prepared = self.text_system.build_text_instances(
                &mut *self.ctx,
                self.renderer.text_atlas(),
                self.renderer.emoji_atlas(),
                &self.board,
                active_text_edit,
            );

            self.renderer.upload_scene_text_instances(
                &mut *self.ctx,
                &prepared.mono_instances,
                &prepared.color_instances,
            );
            self.cached_text_draw = Some(prepared);
            self.cached_text_edit_snapshot = current_edit_snapshot.clone();
            self.text_dirty = false;
        }

        let text_instances = self.cached_text_draw.as_ref().unwrap();

        let moved_mono_instances;
        let moved_color_instances;
        let rotated_mono_instances;
        let rotated_color_instances;
        let moved_caret_pos;
        let mono_instances = if let Some((angle, center)) = rotate_drag_preview {
            rotated_mono_instances = {
                let mut instances = text_instances.mono_instances.clone();
                for range in &text_instances.element_ranges {
                    if self.board.is_selected(range.element_id) {
                        for instance in &mut instances[range.mono_start..range.mono_end] {
                            *instance = rotate_text_instance(*instance, center, angle);
                        }
                    }
                }
                instances
            };
            rotated_mono_instances.as_slice()
        } else if let Some(offset) = move_drag_offset {
            moved_mono_instances = {
                let mut instances = text_instances.mono_instances.clone();
                for range in &text_instances.element_ranges {
                    if self.board.is_selected(range.element_id) {
                        for instance in &mut instances[range.mono_start..range.mono_end] {
                            *instance = offset_text_instance(*instance, offset);
                        }
                    }
                }
                instances
            };
            moved_mono_instances.as_slice()
        } else {
            text_instances.mono_instances.as_slice()
        };
        let color_instances = if let Some((angle, center)) = rotate_drag_preview {
            rotated_color_instances = {
                let mut instances = text_instances.color_instances.clone();
                for range in &text_instances.element_ranges {
                    if self.board.is_selected(range.element_id) {
                        for instance in &mut instances[range.color_start..range.color_end] {
                            *instance = rotate_text_instance(*instance, center, angle);
                        }
                    }
                }
                instances
            };
            rotated_color_instances.as_slice()
        } else if let Some(offset) = move_drag_offset {
            moved_color_instances = {
                let mut instances = text_instances.color_instances.clone();
                for range in &text_instances.element_ranges {
                    if self.board.is_selected(range.element_id) {
                        for instance in &mut instances[range.color_start..range.color_end] {
                            *instance = offset_text_instance(*instance, offset);
                        }
                    }
                }
                instances
            };
            moved_color_instances.as_slice()
        } else {
            text_instances.color_instances.as_slice()
        };
        if rotate_drag_preview.is_some() || move_drag_offset.is_some() {
            self.renderer
                .draw_text_instances(&mut *self.ctx, mono_instances, board_mvp);
            self.renderer
                .draw_color_text_instances(&mut *self.ctx, color_instances, board_mvp);
        } else {
            self.renderer
                .draw_scene_text_instances(&mut *self.ctx, board_mvp);
            self.renderer
                .draw_scene_color_text_instances(&mut *self.ctx, board_mvp);
        }

        moved_caret_pos = if let Some((angle, center)) = rotate_drag_preview {
            text_instances.caret_pos.map(|pos| {
                if self
                    .input
                    .active_text_id
                    .map(|id| self.board.is_selected(id))
                    .unwrap_or(false)
                {
                    rotate_point(pos, center, angle)
                } else {
                    pos
                }
            })
        } else {
            move_drag_offset
                .map(|offset| text_instances.caret_pos.map(|pos| pos + offset))
                .unwrap_or(text_instances.caret_pos)
        };
        if let Some(world_caret) = moved_caret_pos {
            let screen_caret = self.camera.world_to_screen(world_caret, self.screen_size);
            set_ime_candidate_pos(screen_caret.x as i32, screen_caret.y as i32);
        }


        
        if let Some(ref preview) = self.input.preview {
            let preview_inst = toolbar::preview_instances(preview, 0.5);
            self.renderer
                .draw_instances(&mut *self.ctx, &preview_inst, board_mvp, self.screen_size);
        }

        let mut selection_inst = Vec::new();
        for element in &self.board.elements {
            if let Some(instance) = toolbar::selection_instance(element, 1.0) {
                selection_inst.push(
                    if let Some((angle, center)) = rotate_drag_preview.filter(|_| element.selected) {
                        rotate_instance(instance, center, angle)
                    } else if let Some(offset) = move_drag_offset.filter(|_| element.selected) {
                        offset_instance(instance, offset)
                    } else {
                        instance
                    },
                );
            }
        }
        if self.board.selected_count() > 1 {
            if let Some(bounds) = self
                .input
                .drag_selection_bounds
                .or(self.input.selection_bounds)
                .or_else(|| self.board.selected_bounds())
            {
                selection_inst.push(toolbar::selection_bounds_instance(bounds, 1.0));
            }
        }
        if let Some(bounds) = self.input.marquee_bounds {
            selection_inst.push(toolbar::marquee_instance(bounds, 1.0));
        }
        if !selection_inst.is_empty() {
            self.renderer
                .draw_instances(&mut *self.ctx, &selection_inst, board_mvp, self.screen_size);
        }

        // Selection handles
        let mut handle_inst = Vec::new();
        if self.board.selected_count() > 1 {
            if let Some(bounds) = self
                .input
                .drag_selection_bounds
                .or(self.input.selection_bounds)
                .or_else(|| self.board.selected_bounds())
            {
                handle_inst.extend(crate::input::selection_bounds_handles_to_instances(bounds, self.camera.zoom));
            }
        } else {
            for e in &self.board.elements {
                if e.selected {
                    let mut instances = crate::input::handles_to_instances(e, self.camera.zoom);
                    if let Some(offset) = move_drag_offset {
                        for instance in &mut instances {
                            *instance = offset_instance(*instance, offset);
                        }
                    }
                    handle_inst.extend(instances);
                }
            }
        }
        if !handle_inst.is_empty() {
            self.renderer.draw_instances(&mut *self.ctx, &handle_inst, board_mvp, self.screen_size);
        }

        let tb_inst = self.toolbar.build_instances(
            self.screen_size.x,
            self.board.can_undo(),
            self.board.can_redo(),
        );
        let screen_mvp = Renderer::screen_mvp(self.screen_size);

        // Toolbar (full opacity, screen-space)
        self.renderer.draw_instances(&mut *self.ctx, &tb_inst, screen_mvp, self.screen_size);

        // ── Stats overlay ─────────────────────────────────────────────────
        let char_count = mono_instances.len() + color_instances.len();

        let stats_inst = stats::build_stats_instances(
            self.camera.zoom,
            self.board_render_cache.all_instances().len(),
            char_count,
            self.image_manager.atlas_count(),
            self.image_manager.atlas_capacity(),
            self.image_manager.ram_used_bytes(),
            self.image_manager.ram_capacity_bytes(),
            self.image_manager.gpu_used_bytes(),
            self.image_manager.gpu_capacity_bytes(),
            self.fps,
            self.frame_ms,
            self.screen_size,
        );
        self.renderer.draw_instances(&mut *self.ctx, &stats_inst, screen_mvp, self.screen_size);

        self.ctx.end_render_pass();
        self.ctx.commit_frame();

        if self.needs_continuous_redraw() {
            self.request_redraw();
        }
    }

    fn mouse_button_down_event(&mut self, button: MouseButton, x: f32, y: f32) {
        if button == MouseButton::Left && y < toolbar::TOOLBAR_HEIGHT && self.text_edit.is_some() {
            self.finish_text_edit(true);
        }

        let previous_active = self.input.active_text_id;
        if let Some(action) = input::on_mouse_down(
            &mut self.input, &mut self.board, &self.camera,
            &mut self.toolbar, self.screen_size, x, y, button,
        ) {
            self.handle_toolbar_action(action);
            return;
        }

        let new_active = self.input.active_text_id;
        if previous_active != new_active {
            if previous_active.is_some() {
                self.finish_text_edit(true);
            }
            if let Some(id) = new_active {
                self.begin_text_edit(id);
            }
        }

        if button == MouseButton::Left {
            if let Some(id) = self.input.active_text_id {
                if let Some(cursor_byte) = self.text_cursor_from_screen(id, Vec2::new(x, y)) {
                    self.set_text_cursor(cursor_byte, false);
                    self.input.text_selecting = true;
                }
            }
        }

        self.request_redraw();
    }

    fn mouse_button_up_event(&mut self, button: MouseButton, x: f32, y: f32) {
        let drag_mode_before_up = self.input.drag_mode;
        let had_drag = drag_mode_before_up != DragMode::None;
        let had_preview = self.input.preview.is_some();
        let active_before_up = self.input.active_text_id;
        input::on_mouse_up(
            &mut self.input, &mut self.board, &self.camera,
            &mut self.toolbar, self.screen_size, x, y, button,
        );
        self.input.text_selecting = false;
        let active_after_up = self.input.active_text_id;
        if active_before_up != active_after_up {
            if active_before_up.is_some() {
                self.finish_text_edit(true);
            }
            if let Some(id) = active_after_up {
                self.begin_text_edit(id);
            }
        }

        if had_drag || had_preview {
            self.spatial_dirty = true;
        }
        if matches!(drag_mode_before_up, DragMode::MoveSelected | DragMode::ResizingHandle(_) | DragMode::Rotating) {
            self.mark_board_structure_dirty();
            return;
        }
        if had_preview || self.board.elements.len() != self.board_render_cache.all_instances.len() {
            self.mark_board_structure_dirty();
            return;
        }
        self.request_redraw();
    }

    fn mouse_motion_event(&mut self, x: f32, y: f32) {
        let mouse_pos = Vec2::new(x, y);
        if self.input.text_selecting {
            if let Some(id) = self.input.active_text_id {
                self.input.mouse_pos = mouse_pos;
                if let Some(cursor_byte) = self.text_cursor_from_screen(id, mouse_pos) {
                    self.set_text_cursor(cursor_byte, true);
                    self.request_redraw();
                    return;
                }
            }
        }

        let was_panning = self.input.panning;
        let was_dragging_tool = self.input.dragging_tool;

        input::on_mouse_move(
            &mut self.input, &mut self.board, &mut self.camera,
            &self.toolbar, self.screen_size, x, y,
        );

        if self.input.panning || was_panning {
            self.request_redraw();
            return;
        }

        if matches!(self.input.drag_mode, DragMode::MarqueeSelect | DragMode::MoveSelected)
            || (self.input.drag_mode == DragMode::Rotating && self.input.move_origin.len() > 1)
        {
            self.request_redraw();
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
        self.request_redraw();
    }

    fn key_down_event(&mut self, keycode: KeyCode, keymods: KeyMods, _repeat: bool) {
        self.input.shift_held = keymods.shift;
        self.input.ctrl_held = keymods.ctrl;

        if self.input.active_text_id.is_some() {
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
                KeyCode::Left => {
                    self.move_text_cursor(Motion::Left, keymods.shift);
                    self.request_redraw();
                }
                KeyCode::Right => {
                    self.move_text_cursor(Motion::Right, keymods.shift);
                    self.request_redraw();
                }
                KeyCode::Up => {
                    self.move_text_cursor(Motion::Up, keymods.shift);
                    self.request_redraw();
                }
                KeyCode::Down => {
                    self.move_text_cursor(Motion::Down, keymods.shift);
                    self.request_redraw();
                }
                KeyCode::Home => {
                    self.move_text_cursor(Motion::Home, keymods.shift);
                    self.request_redraw();
                }
                KeyCode::End => {
                    self.move_text_cursor(Motion::End, keymods.shift);
                    self.request_redraw();
                }
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
                    if let Some(text) = self.selected_text().or_else(|| self.current_text().map(str::to_string)) {
                        window::clipboard_set(&text);
                    }
                }
                KeyCode::X if keymods.ctrl => {
                    if let Some(text) = self.selected_text().or_else(|| self.current_text().map(str::to_string)) {
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
        if keymods.ctrl && keycode == KeyCode::V {
            if self.handle_board_paste() {
                return;
            }
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
        match keycode {
            KeyCode::LeftShift | KeyCode::RightShift => self.input.shift_held = false,
            KeyCode::LeftControl | KeyCode::RightControl => self.input.ctrl_held = false,
            _ => {}
        }
    }

    fn char_event(&mut self, character: char, _keymods: KeyMods, repeat: bool) {
        if repeat || character.is_control() {
            return;
        }
        if self.input.active_text_id.is_none() {
            return;
        }

        let text = character.to_string();
        if self.insert_text(&text) {
            self.request_redraw();
        }
    }

    fn resize_event(&mut self, width: f32, height: f32) {
        self.screen_size = Vec2::new(width, height);
        self.visibility_dirty = true;
        self.request_redraw();
    }

    fn files_dropped_event(&mut self) {
        self.import_dropped_files();
    }
}

impl App {
    fn import_image_via_dialog(&mut self) {
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
            color: [1.0, 1.0, 1.0, 1.0],
        };
        let max_width = (self.screen_size.x / self.camera.zoom.max(0.0001) * 0.5).max(180.0);
        let size = self
            .text_system
            .measure_text_box(&text_data.content, &text_data, max_width);
        let anchor = self.paste_anchor_world();
        let new_id = self.board.next_id();

        self.board.apply_operation(BoardOperation::AddElement(Element {
            id: new_id,
            shape: ShapeType::Text,
            pos: anchor - size * 0.5,
            size,
            rotation: 0.0,
            color: [0.0, 0.0, 0.0, 0.0],
            selected: false,
            text: Some(text_data),
            image: None,
            text_layout_generation: 0,
        }));
        self.mark_board_structure_dirty();
        true
    }

    fn handle_board_paste(&mut self) -> bool {
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

    fn import_dropped_files(&mut self) {
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

    fn build_image_draws(
        &mut self,
        move_drag_offset: Option<Vec2>,
        rotate_drag_preview: Option<(f32, Vec2)>,
    ) -> Vec<PreparedImageDraw> {
        let (vis_min, vis_max) = self.camera.visible_rect(self.screen_size);
        let range = VisibleRange {
            min: vis_min - Vec2::splat(BOARD_VISIBILITY_MARGIN),
            max: vis_max + Vec2::splat(BOARD_VISIBILITY_MARGIN),
        };

        let pending: Vec<(crate::board::ImageData, Vec2, Vec2, f32)> = self
            .board
            .elements
            .iter()
            .filter_map(|element| {
                if element.shape != ShapeType::Image {
                    return None;
                }

                let image = element.image.clone()?;
                let (pos, size, rotation) = preview_rect_transform(element, move_drag_offset, rotate_drag_preview);
                let mut preview_element = element.clone();
                preview_element.pos = pos;
                preview_element.size = size;
                preview_element.rotation = rotation;
                element_in_range(&preview_element, range).then_some((image, pos, size, rotation))
            })
            .collect();

        for (image, _, _, _) in &pending {
            self.image_manager
                .preload_thumb(&mut *self.ctx, &image.asset_path);
        }

        pending
            .into_iter()
            .map(|(image, pos, size, rotation)| {
                self.image_manager.prepare_draw(
                    &mut *self.ctx,
                    &image,
                    pos.to_array(),
                    size.to_array(),
                    rotation,
                    self.camera.zoom,
                    [size.x * self.camera.zoom, size.y * self.camera.zoom],
                    self.screen_size.to_array(),
                )
            })
            .collect()
    }

    fn begin_text_edit(&mut self, id: u64) {
        let Some(element) = self.board.element(id) else {
            return;
        };
        let original_text = element.text.clone();
        let cursor_byte = original_text
            .as_ref()
            .map(|text| text.content.len())
            .unwrap_or(0);
        self.input.active_text_id = Some(id);
        self.text_edit = Some(TextEditSession {
            element_id: id,
            buffer: original_text
                .as_ref()
                .map(|text| text.content.clone())
                .unwrap_or_default(),
            original_text,
            cursor_byte,
            selection_anchor_byte: None,
            preferred_x: None,
        });
        self.text_dirty = true;
        self.text_system.bump_edit_generation();
    }

    fn finish_text_edit(&mut self, commit: bool) {
        let Some(edit) = self.text_edit.take() else {
            self.input.active_text_id = None;
            self.input.text_selecting = false;
            return;
        };

        self.input.active_text_id = None;
        self.input.text_selecting = false;
        self.text_dirty = true;

        if commit {
            let before = edit.original_text.clone();
            let after = match before.clone() {
                Some(mut text) => {
                    text.content = edit.buffer.clone();
                    Some(text)
                }
                None if edit.buffer.is_empty() => None,
                None => Some(TextData {
                    content: edit.buffer.clone(),
                    ..TextData::default()
                }),
            };

            if before != after {
                self.board.apply_operation(BoardOperation::SetProperty {
                    changes: vec![ElementPropertyChange {
                        id: edit.element_id,
                        patch: ElementPropertyPatch::Text { before, after },
                    }],
                });
            }
        }

        self.request_redraw();
    }

    fn text_cursor_from_screen(&mut self, id: u64, screen_pos: Vec2) -> Option<usize> {
        let world = self.camera.screen_to_world(screen_pos, self.screen_size);
        let element = self.board.element(id)?;
        let content = self
            .text_edit
            .as_ref()
            .filter(|edit| edit.element_id == id)
            .map(|edit| edit.buffer.as_str())
            .or_else(|| element.text.as_ref().map(|text| text.content.as_str()))
            .unwrap_or_default();
        self.text_system.hit_test_cursor(element, content, world)
    }

    fn set_text_cursor(&mut self, cursor_byte: usize, extend_selection: bool) {
        if let Some(edit) = self.text_edit.as_mut() {
            edit.set_cursor(cursor_byte, extend_selection);
            edit.preferred_x = None;
        }
    }

    fn move_text_cursor(&mut self, motion: Motion, extend_selection: bool) {
        let Some(edit) = self.text_edit.as_mut() else {
            return;
        };
        let Some(element) = self.board.element(edit.element_id) else {
            return;
        };
        if let Some((cursor_byte, preferred_x)) = self.text_system.move_cursor(
            element,
            &edit.buffer,
            edit.cursor_byte,
            edit.preferred_x,
            motion,
        ) {
            edit.preferred_x = preferred_x;
            edit.set_cursor(cursor_byte, extend_selection);
        }
    }

    fn selected_text(&self) -> Option<String> {
        let edit = self.text_edit.as_ref()?;
        let (start, end) = edit.selection_range()?;
        Some(edit.buffer[start..end].to_string())
    }

    fn current_text(&self) -> Option<&str> {
        self.text_edit.as_ref().map(|edit| edit.buffer.as_str())
    }

    fn select_all_text(&mut self) {
        let Some(edit) = self.text_edit.as_mut() else {
            return;
        };
        edit.selection_anchor_byte = Some(0);
        edit.cursor_byte = edit.buffer.len();
        edit.preferred_x = None;
    }

    fn delete_selection_or_all(&mut self) -> bool {
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
        if edit.buffer.is_empty() {
            return false;
        }
        edit.buffer.clear();
        edit.cursor_byte = 0;
        edit.clear_selection();
        edit.preferred_x = None;
        true
    }

    fn delete_selection(&mut self) -> bool {
        let Some(edit) = self.text_edit.as_mut() else {
            return false;
        };
        let Some((start, end)) = edit.selection_range() else {
            return false;
        };
        edit.buffer.replace_range(start..end, "");
        edit.cursor_byte = start;
        edit.clear_selection();
        edit.preferred_x = None;
        self.text_dirty = true;
        self.text_system.bump_edit_generation();
        true
    }

    fn insert_text(&mut self, inserted: &str) -> bool {
        let _ = self.delete_selection();
        let Some(edit) = self.text_edit.as_mut() else {
            return false;
        };
        let cursor = edit.cursor_byte.min(edit.buffer.len());
        edit.buffer.insert_str(cursor, inserted);
        edit.cursor_byte = cursor + inserted.len();
        edit.clear_selection();
        edit.preferred_x = None;
        self.text_dirty = true;
        self.text_system.bump_edit_generation();
        true
    }

    fn delete_backward(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        let Some(edit) = self.text_edit.as_mut() else {
            return false;
        };
        if edit.cursor_byte == 0 {
            return false;
        }
        let previous = previous_char_boundary(&edit.buffer, edit.cursor_byte);
        edit.buffer.replace_range(previous..edit.cursor_byte, "");
        edit.cursor_byte = previous;
        edit.preferred_x = None;
        self.text_dirty = true;
        self.text_system.bump_edit_generation();
        true
    }

    fn delete_forward(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        let Some(edit) = self.text_edit.as_mut() else {
            return false;
        };
        if edit.cursor_byte >= edit.buffer.len() {
            return false;
        }
        let next = next_char_boundary(&edit.buffer, edit.cursor_byte);
        edit.buffer.replace_range(edit.cursor_byte..next, "");
        edit.preferred_x = None;
        self.text_dirty = true;
        self.text_system.bump_edit_generation();
        true
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

fn normalize_pasted_text(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Move the OS IME / emoji candidate window to the given screen-space pixel coordinate.
/// On non-Windows platforms this is a no-op.
fn set_ime_candidate_pos(x: i32, y: i32) {
    #[cfg(target_os = "windows")]
    unsafe {
        use winapi::um::winuser::GetForegroundWindow;
        use winapi::um::imm::{ImmGetContext, ImmReleaseContext, ImmSetCompositionWindow,
                               COMPOSITIONFORM, CFS_POINT};
        use winapi::shared::windef::POINT;

        let hwnd = GetForegroundWindow();
        if hwnd.is_null() { return; }
        let himc = ImmGetContext(hwnd);
        if himc.is_null() { return; }
        let mut cf = COMPOSITIONFORM {
            dwStyle: CFS_POINT,
            ptCurrentPos: POINT { x, y },
            rcArea: std::mem::zeroed(),
        };
        ImmSetCompositionWindow(himc, &mut cf);
        ImmReleaseContext(hwnd, himc);
    }
    // suppress unused-variable warnings on non-Windows
    #[cfg(not(target_os = "windows"))]
    let _ = (x, y);
}

