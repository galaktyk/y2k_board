mod content;
mod keyboard;
mod rendering;
mod snapshot;
mod style;
mod text_editing;



use miniquad::*;
use glam::Vec2;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use crate::board::{
    Board, Element, ElementPropertyChange, ElementPropertyPatch, ElementStyleSnapshot,
    ShapeType, ToolStyleDefaults,
};
use crate::camera::Camera;
use crate::images::ImageManager;
use crate::input::{self, DragMode, InputState};
use crate::rendering::renderer::Renderer;
use crate::rendering::cache::BoardRenderCache;
use crate::{snapshot as snapshot_io, ui};
use crate::spatial::SpatialGrid;
use crate::text::{PreparedTextDraw, TextEditSession, TextEditSnapshot, TextSystem};

use crate::ui::toolbar::{self, Toolbar, ToolbarAction};
use crate::ui::property_panel::{self, ColorTarget, LineArrowTarget, WidthTarget};

const IMAGE_RAM_FLUSH_INTERVAL: Duration = Duration::from_secs(60);
const IMAGE_RAM_FLUSH_INTERVAL_SECS: f64 = IMAGE_RAM_FLUSH_INTERVAL.as_secs_f64();
#[derive(Clone, Copy)]
enum ImageRamFlushTrigger {
    Auto,
    Manual,
}

impl ImageRamFlushTrigger {
    fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Manual => "manual",
        }
    }

    fn should_request_redraw(self) -> bool {
        matches!(self, Self::Manual)
    }
}

#[derive(Clone)]
enum PropertyPanelSource {
    Tool(ui::tool::Tool),
    Selection(Vec<u64>),
}

#[derive(Clone)]
struct ResolvedPropertyPanel {
    source: PropertyPanelSource,
    view: property_panel::PropertyPanelView,
}

enum WidthDragState {
    Tool {
        tool: ui::tool::Tool,
        target: WidthTarget,
    },
    Selection {
        target: WidthTarget,
        before: Vec<(u64, ElementStyleSnapshot)>,
    },
}

impl WidthDragState {
    fn target(&self) -> WidthTarget {
        match self {
            Self::Tool { target, .. } | Self::Selection { target, .. } => *target,
        }
    }
}

pub struct App {
    ctx: Box<dyn RenderingBackend>,
    renderer: Renderer,
    board: Board,
    snapshot_path: PathBuf,
    snapshot_path_user_selected: bool,
    camera: Camera,
    toolbar: Toolbar,
    toolbar_icons: toolbar::ToolbarIcons,
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
    image_ram_flush_stop: Arc<(Mutex<bool>, Condvar)>,
    image_ram_flush_thread: Option<JoinHandle<()>>,
    image_ram_flush_deadline: f64,
    tool_style_defaults: ToolStyleDefaults,
    property_panel_target: ColorTarget,
    property_width_drag: Option<WidthDragState>,
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
        let toolbar_icons = toolbar::ToolbarIcons::new(&mut *ctx);
        let snapshot_path = snapshot_io::default_snapshot_path();
        let asset_root = snapshot_io::snapshot_root(&snapshot_path);
        let image_manager = ImageManager::new(&mut *ctx, asset_root);
        let (w, h) = window::screen_size();
        let now = miniquad::date::now();
        let (image_ram_flush_stop, image_ram_flush_thread) = spawn_image_ram_flush_waker();
        let app = Self {
            ctx,
            renderer,
            board: Board::new(),
            snapshot_path,
            snapshot_path_user_selected: false,
            camera: Camera::new(),
            toolbar: Toolbar::new(),
            toolbar_icons,
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
            image_ram_flush_stop,
            image_ram_flush_thread: Some(image_ram_flush_thread),
            image_ram_flush_deadline: now + IMAGE_RAM_FLUSH_INTERVAL_SECS,
            tool_style_defaults: ToolStyleDefaults::default(),
            property_panel_target: ColorTarget::Fill,
            property_width_drag: None,
            text_edit: None,
            cached_text_draw: None,
            text_dirty: true,
            cached_text_edit_snapshot: None,
            last_frame:  now,
            frame_ms:    0.0,
            fps:         0.0,
            fps_accum:   0.0,
            fps_frames:  0,
        };
        app.request_redraw();
        app
    }

    /// Rebuilds the spatial grid for hit testing.
    /// This is O(N) where N is the number of elements on the board.
    /// Called when board structure changes or after drag-and-drop.
    fn rebuild_spatial(&mut self) {


        // [HOT] Rebuilding spatial grid
        self.spatial.clear();
        for e in &self.board.elements {
            let (min, max) = e.aabb();
            self.spatial.insert(e.id, min, max);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn pick_snapshot_save_path(&self) -> Option<PathBuf> {
        snapshot::pick_save_path(&self.snapshot_path)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn pick_snapshot_load_path(&self) -> Option<PathBuf> {
        snapshot::pick_load_path(&self.snapshot_path)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn copy_snapshot_assets(&self, source_root: &Path, target_root: &Path) -> std::io::Result<()> {
        snapshot::copy_assets(&self.board.elements, source_root, target_root)
    }

    fn request_redraw(&self) {
        window::schedule_update();
    }

    fn flush_image_ram_cache(&mut self, trigger: ImageRamFlushTrigger) {
        let before_ram = self.image_manager.ram_used_bytes();
        let gpu_bytes = self.image_manager.gpu_used_bytes();
        let stats = self.image_manager.clear_ram_cache();
        let after_ram = self.image_manager.ram_used_bytes();
        println!(
            "[image] RAM clear source={} entries={} freed={:.2} MiB ram={:.2} MiB gpu={:.2} MiB",
            trigger.label(),
            stats.entries_cleared,
            mib(stats.bytes_freed),
            mib(after_ram),
            mib(gpu_bytes),
        );
        if before_ram == 0 && stats.entries_cleared == 0 {
            println!("[image] RAM clear source={} cache already empty", trigger.label());
        }
        self.image_ram_flush_deadline = miniquad::date::now() + IMAGE_RAM_FLUSH_INTERVAL_SECS;
        if trigger.should_request_redraw() {
            self.request_redraw();
        }
    }

    fn update_image_ram_maintenance(&mut self) {
        if miniquad::date::now() >= self.image_ram_flush_deadline {
            self.flush_image_ram_cache(ImageRamFlushTrigger::Auto);
        }
    }

    fn needs_continuous_redraw(&self) -> bool {
        self.input.panning
            || self.input.has_pan_glide()
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

    fn mark_board_order_dirty(&mut self) {
        self.board_cache_dirty = true;
        self.board_scene_dirty = true;
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

    fn set_active_tool(&mut self, tool: ui::tool::Tool) {
        if self.toolbar.active_tool != tool {
            if self.input.active_text_id.is_some() {
                self.finish_text_edit(true);
            }
            let selected_before = self.board.selected_ids();
            self.board.deselect_all();
            if !selected_before.is_empty() {
                self.mark_elements_dirty(selected_before);
            }
            self.toolbar.active_tool = tool;
            self.request_redraw();
        }
    }

    fn resolve_property_panel(&self) -> Option<ResolvedPropertyPanel> {
        if self.board.selected_count() > 0 {
            self.resolve_selection_property_panel()
        } else {
            self.resolve_tool_property_panel()
        }
    }

    fn resolve_selection_property_panel(&self) -> Option<ResolvedPropertyPanel> {
        let selected: Vec<&Element> = self.board.elements.iter().filter(|element| element.selected).collect();
        if selected.is_empty() {
            return None;
        }

        let ids: Vec<u64> = selected.iter().map(|element| element.id).collect();
        let can_fill = selected
            .iter()
            .all(|element| matches!(element.shape, ShapeType::Rect | ShapeType::Ellipse));
        let can_text = selected.iter().all(|element| element.can_host_text());
        let can_stroke = selected.iter().all(|element| {
            matches!(element.shape, ShapeType::Rect | ShapeType::Ellipse | ShapeType::Line)
        });

        if !can_fill && !can_text && !can_stroke {
            return None;
        }

        let tabs = style::tabs(can_text, can_fill, can_stroke);
        let active_target = self.resolve_panel_target(tabs)?;
        let title = style::title_for_selection(&selected);
        let active_color = style::color_for_selection(&selected, active_target);
        let border_width = selected
            .iter()
            .find(|element| element.uses_border_width())
            .map(|element| element.border_width.min(16));
        let stroke_width = selected
            .iter()
            .find(|element| element.uses_stroke_width())
            .map(|element| element.stroke_width.clamp(1, 16));
        let show_line_arrows = selected.iter().all(|element| element.shape == ShapeType::Line);
        let line_arrow_start = show_line_arrows.then(|| selected[0].line_arrow_start);
        let line_arrow_end = show_line_arrows.then(|| selected[0].line_arrow_end);

        Some(ResolvedPropertyPanel {
            source: PropertyPanelSource::Selection(ids),
            view: property_panel::PropertyPanelView {
                title,
                tabs,
                active_target,
                active_color,
                border_width,
                stroke_width,
                line_arrow_start,
                line_arrow_end,
            },
        })
    }

    fn resolve_tool_property_panel(&self) -> Option<ResolvedPropertyPanel> {
        let (title, tabs, active_color, border_width, stroke_width, line_arrow_start, line_arrow_end) = match self.toolbar.active_tool {
            ui::tool::Tool::Rect => {
                let tabs = style::tabs(true, true, true);
                let active = self.resolve_panel_target(tabs)?;
                (
                    "RECT",
                    tabs,
                    style::color_for_box_defaults(self.tool_style_defaults.rect, active),
                    Some(self.tool_style_defaults.rect.border_width.min(16)),
                    None,
                    None,
                    None,
                )
            }
            ui::tool::Tool::Ellipse => {
                let tabs = style::tabs(true, true, true);
                let active = self.resolve_panel_target(tabs)?;
                (
                    "ELPS",
                    tabs,
                    style::color_for_box_defaults(self.tool_style_defaults.ellipse, active),
                    Some(self.tool_style_defaults.ellipse.border_width.min(16)),
                    None,
                    None,
                    None,
                )
            }
            ui::tool::Tool::Text => {
                let tabs = style::tabs(true, true, true);
                let active = self.resolve_panel_target(tabs)?;
                (
                    "TEXT",
                    tabs,
                    style::color_for_box_defaults(self.tool_style_defaults.text, active),
                    Some(self.tool_style_defaults.text.border_width.min(16)),
                    None,
                    None,
                    None,
                )
            }
            ui::tool::Tool::Line => {
                let tabs = style::tabs(false, false, true);
                (
                    "LINE",
                    tabs,
                    self.tool_style_defaults.line.color,
                    None,
                    Some(self.tool_style_defaults.line.stroke_width.clamp(1, 16)),
                    Some(self.tool_style_defaults.line.arrow_start),
                    Some(self.tool_style_defaults.line.arrow_end),
                )
            }
            ui::tool::Tool::Select => return None,
        };

        Some(ResolvedPropertyPanel {
            source: PropertyPanelSource::Tool(self.toolbar.active_tool),
            view: property_panel::PropertyPanelView {
                title,
                tabs,
                active_target: self.resolve_panel_target(tabs)?,
                active_color,
                border_width,
                stroke_width,
                line_arrow_start,
                line_arrow_end,
            },
        })
    }

    fn resolve_panel_target(&self, tabs: [Option<ColorTarget>; 3]) -> Option<ColorTarget> {
        tabs.into_iter()
            .flatten()
            .find(|target| *target == self.property_panel_target)
            .or_else(|| property_panel::first_available_target(tabs))
    }

    fn apply_property_panel_hit(&mut self, hit: property_panel::PropertyPanelHit) {
        match hit {
            property_panel::PropertyPanelHit::Tab(target) => {
                self.property_panel_target = target;
            }
            property_panel::PropertyPanelHit::Swatch(index) => {
                let target = self
                    .resolve_property_panel()
                    .map(|panel| panel.view.active_target)
                    .unwrap_or(self.property_panel_target);
                self.apply_property_panel_color(target, crate::palette::PALETTE[index]);
            }
            property_panel::PropertyPanelHit::Width(target, width) => {
                self.begin_property_width_drag(target, width);
            }
            property_panel::PropertyPanelHit::Arrow(target) => {
                self.apply_property_panel_arrow(target);
            }
        }
    }

    fn apply_property_panel_arrow(&mut self, target: LineArrowTarget) {
        let Some(panel) = self.resolve_property_panel() else {
            return;
        };

        let enabled = match target {
            LineArrowTarget::Start => !panel.view.line_arrow_start.unwrap_or(false),
            LineArrowTarget::End => !panel.view.line_arrow_end.unwrap_or(false),
        };

        match panel.source {
            PropertyPanelSource::Tool(tool) => {
                self.apply_tool_panel_arrow(tool, target, enabled);
                self.request_redraw();
            }
            PropertyPanelSource::Selection(ids) => {
                let changes: Vec<ElementPropertyChange> = ids
                    .iter()
                    .filter_map(|id| {
                        let element = self.board.element(*id)?;
                        let before = element.style_snapshot();
                        let after = style::updated_style_with_arrow(element, target, enabled)?;
                        (before != after).then_some(ElementPropertyChange {
                            id: *id,
                            patch: ElementPropertyPatch::Style { before, after },
                        })
                    })
                    .collect();

                if changes.is_empty() {
                    self.request_redraw();
                    return;
                }

                self.board.apply_operation(crate::board::BoardOperation::SetProperty {
                    changes,
                    sync_connected_lines: false,
                });
                self.mark_elements_dirty(ids);
            }
        }
    }

    fn apply_property_panel_color(&mut self, target: ColorTarget, color: [f32; 4]) {
        let Some(panel) = self.resolve_property_panel() else {
            return;
        };

        match panel.source {
            PropertyPanelSource::Tool(tool) => {
                self.apply_tool_panel_color(tool, target, color);
                self.request_redraw();
            }
            PropertyPanelSource::Selection(ids) => {
                let changes: Vec<ElementPropertyChange> = ids
                    .iter()
                    .filter_map(|id| {
                        let element = self.board.element(*id)?;
                        let before = element.style_snapshot();
                        let after = style::updated_style_with_color(element, target, color)?;
                        (before != after).then_some(ElementPropertyChange {
                            id: *id,
                            patch: ElementPropertyPatch::Style { before, after },
                        })
                    })
                    .collect();

                if changes.is_empty() {
                    self.request_redraw();
                    return;
                }

                self.board.apply_operation(crate::board::BoardOperation::SetProperty {
                    changes,
                    sync_connected_lines: true,
                });
                self.mark_elements_dirty(ids);
            }
        }
    }

    fn begin_property_width_drag(&mut self, target: WidthTarget, width: u8) {
        let Some(panel) = self.resolve_property_panel() else {
            return;
        };

        self.property_width_drag = Some(match panel.source.clone() {
            PropertyPanelSource::Tool(tool) => WidthDragState::Tool { tool, target },
            PropertyPanelSource::Selection(ids) => WidthDragState::Selection {
                target,
                before: ids
                    .iter()
                    .filter_map(|id| self.board.element(*id).map(|element| (*id, element.style_snapshot())))
                    .collect(),
            },
        });
        self.preview_property_width(target, width);
    }

    fn preview_property_width(&mut self, target: WidthTarget, width: u8) {
        let width = width.clamp(target.min_width(), 16);
        let Some(state) = self.property_width_drag.as_ref() else {
            return;
        };

        match state {
            WidthDragState::Tool { tool, .. } => {
                self.apply_tool_panel_width(*tool, target, width);
                self.request_redraw();
            }
            WidthDragState::Selection { before, .. } => {
                let ids: Vec<u64> = before.iter().map(|(id, _)| *id).collect();
                for (id, _) in before {
                    if let Some(element) = self.board.element_mut(*id) {
                        if let Some(after) = style::updated_style_with_width(element, target, width) {
                            element.apply_style_snapshot(after);
                        }
                    }
                }
                self.mark_elements_dirty(ids);
            }
        }
    }

    fn finish_property_width_drag(&mut self) {
        let Some(state) = self.property_width_drag.take() else {
            return;
        };

        match state {
            WidthDragState::Tool { .. } => {
                self.request_redraw();
            }
            WidthDragState::Selection { before, .. } => {
                let ids: Vec<u64> = before.iter().map(|(id, _)| *id).collect();
                let changes: Vec<ElementPropertyChange> = before
                    .into_iter()
                    .filter_map(|(id, before)| {
                        let after = self.board.element(id)?.style_snapshot();
                        (before != after).then_some(ElementPropertyChange {
                            id,
                            patch: ElementPropertyPatch::Style { before, after },
                        })
                    })
                    .collect();

                if !changes.is_empty() {
                    self.board.apply_operation(crate::board::BoardOperation::SetProperty {
                        changes,
                        sync_connected_lines: true,
                    });
                    self.mark_elements_dirty(ids);
                } else {
                    self.request_redraw();
                }
            }
        }
    }

    fn apply_tool_panel_color(&mut self, tool: ui::tool::Tool, target: ColorTarget, color: [f32; 4]) {
        match tool {
            ui::tool::Tool::Rect => style::apply_box_color(&mut self.tool_style_defaults.rect, target, color),
            ui::tool::Tool::Ellipse => style::apply_box_color(&mut self.tool_style_defaults.ellipse, target, color),
            ui::tool::Tool::Text => style::apply_box_color(&mut self.tool_style_defaults.text, target, color),
            ui::tool::Tool::Line => {
                if target == ColorTarget::Stroke {
                    self.tool_style_defaults.line.color = color;
                }
            }
            ui::tool::Tool::Select => {}
        }
    }

    fn apply_tool_panel_width(&mut self, tool: ui::tool::Tool, target: WidthTarget, width: u8) {
        match tool {
            ui::tool::Tool::Rect if target == WidthTarget::Border => self.tool_style_defaults.rect.border_width = width,
            ui::tool::Tool::Ellipse if target == WidthTarget::Border => self.tool_style_defaults.ellipse.border_width = width,
            ui::tool::Tool::Text if target == WidthTarget::Border => self.tool_style_defaults.text.border_width = width,
            ui::tool::Tool::Line if target == WidthTarget::Stroke => self.tool_style_defaults.line.stroke_width = width,
            ui::tool::Tool::Select => {}
            _ => {}
        }
    }

    fn apply_tool_panel_arrow(&mut self, tool: ui::tool::Tool, target: LineArrowTarget, enabled: bool) {
        if tool != ui::tool::Tool::Line {
            return;
        }

        match target {
            LineArrowTarget::Start => self.tool_style_defaults.line.arrow_start = enabled,
            LineArrowTarget::End => self.tool_style_defaults.line.arrow_end = enabled,
        }
    }

    fn handle_escape(&mut self) {
        if self.input.active_text_id.is_some() {
            self.finish_text_edit(true);
        }

        self.board.deselect_all();
        self.input.dragging_tool = false;
        self.input.drag_mode = DragMode::None;
        self.input.pending_drag_mode = DragMode::None;
        self.input.preview = None;
        self.input.move_origin.clear();
        self.input.move_delta = Vec2::ZERO;
        self.input.rotate_delta = 0.0;
        self.input.marquee_bounds = None;
        self.input.selection_bounds = None;
        self.input.drag_selection_bounds = None;
        self.input.transform_bounds_origin = None;
        self.input.active_text_id = None;
        self.input.text_selecting = false;
        self.property_width_drag = None;
        self.set_active_tool(ui::tool::Tool::Select);
    }

    /// Synchronizes the board's CPU state with the GPU render cache.
    /// This is a high-cost operation if structure or visibility is dirty.
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
        #[cfg(not(target_arch = "wasm32"))]
        let target_path = if self.snapshot_path_user_selected {
            self.snapshot_path.clone()
        } else {
            let Some(path) = self.pick_snapshot_save_path() else {
                return;
            };
            path
        };

        #[cfg(target_arch = "wasm32")]
        let target_path = self.snapshot_path.clone();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let current_root = snapshot_io::snapshot_root(&self.snapshot_path);
            let target_root = snapshot_io::snapshot_root(&target_path);
            if let Err(err) = self.copy_snapshot_assets(&current_root, &target_root) {
                eprintln!("Failed to prepare snapshot assets: {err}");
                return;
            }
        }

        match snapshot_io::save_to_path(&self.board, &target_path) {
            Ok(path) => {
                self.snapshot_path = path.clone();
                self.snapshot_path_user_selected = true;
                let asset_root = snapshot_io::snapshot_root(&self.snapshot_path);
                self.image_manager.set_asset_root(&mut *self.ctx, asset_root);
                println!("Saved snapshot to {}", path.display());
            }
            Err(err) => eprintln!("Failed to save snapshot: {err}"),
        }
    }

    fn load_snapshot(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        let Some(path) = self.pick_snapshot_load_path() else {
            return;
        };

        #[cfg(target_arch = "wasm32")]
        let path = self.snapshot_path.clone();

        match snapshot_io::load_from_path(&path) {
            Ok(loaded) => {
                self.snapshot_path = loaded.path.clone();
                self.snapshot_path_user_selected = true;
                let asset_root = snapshot_io::snapshot_root(&self.snapshot_path);
                self.image_manager.set_asset_root(&mut *self.ctx, asset_root);
                self.board
                    .restore_snapshot(loaded.data);
                self.camera = Camera::new();
                self.input = InputState::new();
                self.toolbar = Toolbar::new();
                self.tool_style_defaults = ToolStyleDefaults::default();
                self.property_panel_target = ColorTarget::Fill;
                self.property_width_drag = None;
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
                self.set_active_tool(tool);
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
    fn update(&mut self) {
        self.update_image_ram_maintenance();
    }

    fn draw(&mut self) {
        self.draw_frame();
    }

    fn mouse_button_down_event(&mut self, button: MouseButton, x: f32, y: f32) {
        if button == MouseButton::Left {
            if let Some(panel) = self.resolve_property_panel() {
                if property_panel::contains_point(self.screen_size, &panel.view, x, y) {
                    if self.text_edit.is_some() {
                        self.finish_text_edit(true);
                    }
                    if let Some(hit) = property_panel::hit_test(self.screen_size, &panel.view, x, y) {
                        self.apply_property_panel_hit(hit);
                    }
                    self.request_redraw();
                    return;
                }
            }
        }

        if button == MouseButton::Left && self.toolbar.contains_point(self.screen_size, x, y) {
            if self.text_edit.is_some() {
                self.finish_text_edit(true);
            }
            if let Some(action) = self.toolbar.hit_test(self.screen_size, x, y) {
                self.handle_toolbar_action(action);
            }
            self.request_redraw();
            return;
        }

        let previous_active = self.input.active_text_id;
        let selected_before: std::collections::HashSet<u64> = self.board.selected_ids().into_iter().collect();

        let order_changed = input::on_mouse_down(
            &mut self.input,
            &mut self.board,
            &self.camera,
            self.toolbar.active_tool,
            self.screen_size,
            x,
            y,
            button,
        );

        let selected_after: std::collections::HashSet<u64> = self.board.selected_ids().into_iter().collect();
        let changed_ids: Vec<u64> = selected_before.symmetric_difference(&selected_after).copied().collect();
        if !changed_ids.is_empty() {
            self.mark_elements_dirty(changed_ids);
        }

        if order_changed {
            self.mark_board_order_dirty();
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
        if button == MouseButton::Left && self.property_width_drag.is_some() {
            self.input.mouse_pos = Vec2::new(x, y);
            self.finish_property_width_drag();
            return;
        }

        let drag_mode_before_up = self.input.drag_mode;
        let had_drag = drag_mode_before_up != DragMode::None;
        let had_preview = self.input.preview.is_some();
        let active_before_up = self.input.active_text_id;
        let selected_before: std::collections::HashSet<u64> = self.board.selected_ids().into_iter().collect();

        if let Some(tool) = input::on_mouse_up(
            &mut self.input,
            &mut self.board,
            &self.camera,
            &self.tool_style_defaults,
            self.toolbar.active_tool,
            self.screen_size,
            x,
            y,
            button,
        ) {
            self.toolbar.active_tool = tool;
        }

        let selected_after: std::collections::HashSet<u64> = self.board.selected_ids().into_iter().collect();
        let changed_ids: Vec<u64> = selected_before.symmetric_difference(&selected_after).copied().collect();
        if !changed_ids.is_empty() {
            self.mark_elements_dirty(changed_ids);
        }

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
        if had_preview || self.board.elements.len() != self.board_render_cache.element_count() {
            self.mark_board_structure_dirty();
            return;
        }
        self.request_redraw();
    }

    fn mouse_motion_event(&mut self, x: f32, y: f32) {
        let previous_hover = self
            .toolbar
            .hovered_action(self.screen_size, self.input.mouse_pos.x, self.input.mouse_pos.y);
        let previous_panel_hover = self
            .resolve_property_panel()
            .and_then(|panel| property_panel::hit_test(self.screen_size, &panel.view, self.input.mouse_pos.x, self.input.mouse_pos.y));
        let mouse_pos = Vec2::new(x, y);

        if self.property_width_drag.is_some() {
            self.input.mouse_pos = mouse_pos;
            let target = self.property_width_drag.as_ref().map(WidthDragState::target);
            if let Some(panel) = self.resolve_property_panel() {
                if let Some(target) = target {
                    if let Some(width) = property_panel::width_at_x(self.screen_size, &panel.view, target, x) {
                        self.preview_property_width(target, width);
                    }
                }
            }
            return;
        }

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
            &mut self.input,
            &mut self.board,
            &mut self.camera,
            &self.tool_style_defaults,
            self.toolbar.active_tool,
            self.screen_size,
            x,
            y,
        );

        let current_hover = self
            .toolbar
            .hovered_action(self.screen_size, self.input.mouse_pos.x, self.input.mouse_pos.y);
        let current_panel_hover = self
            .resolve_property_panel()
            .and_then(|panel| property_panel::hit_test(self.screen_size, &panel.view, self.input.mouse_pos.x, self.input.mouse_pos.y));
        if previous_hover != current_hover || previous_panel_hover != current_panel_hover {
            self.request_redraw();
            return;
        }

        if self.input.panning || was_panning {
            self.request_redraw();
            return;
        }

        if self.input.drag_mode == DragMode::MoveSelected {
            self.request_redraw();
            return;
        }

        if self.input.drag_mode == DragMode::MarqueeSelect
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
        self.handle_key_down(keycode, keymods);
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
        self.handle_char_input(character, repeat);
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

impl Drop for App {
    fn drop(&mut self) {
        stop_image_ram_flush_waker(&self.image_ram_flush_stop, self.image_ram_flush_thread.take());
        self.toolbar_icons.destroy(&mut *self.ctx);
    }
}

fn spawn_image_ram_flush_waker() -> (Arc<(Mutex<bool>, Condvar)>, JoinHandle<()>) {
    let stop = Arc::new((Mutex::new(false), Condvar::new()));
    let stop_clone = Arc::clone(&stop);
    let thread = thread::Builder::new()
        .name("image-ram-flush-waker".to_string())
        .spawn(move || loop {
            let (lock, condvar) = &*stop_clone;
            let stopped = lock.lock().unwrap();
            let (stopped, timeout) = condvar
                .wait_timeout(stopped, IMAGE_RAM_FLUSH_INTERVAL)
                .unwrap();
            if *stopped {
                break;
            }
            drop(stopped);
            if timeout.timed_out() {
                window::schedule_update();
            }
        })
        .expect("image RAM flush waker thread should start");
    (stop, thread)
}

fn stop_image_ram_flush_waker(
    stop: &Arc<(Mutex<bool>, Condvar)>,
    thread: Option<JoinHandle<()>>,
) {
    let (lock, condvar) = &**stop;
    if let Ok(mut stopped) = lock.lock() {
        *stopped = true;
        condvar.notify_all();
    }
    if let Some(thread) = thread {
        let _ = thread.join();
    }
}

fn mib(bytes: usize) -> f32 {
    bytes as f32 / (1024.0 * 1024.0)
}

