mod content;
mod keyboard;
mod rendering;
mod text_editing;

use miniquad::*;
use glam::Vec2;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use crate::board::{
    Board,
};
use crate::camera::Camera;
use crate::images::ImageManager;
use crate::input::{self, DragMode, InputState};
use crate::renderer::Renderer;
use crate::rendering::cache::BoardRenderCache;
use crate::snapshot;
use crate::spatial::SpatialGrid;
use crate::text::{PreparedTextDraw, TextEditSession, TextEditSnapshot, TextSystem};
use crate::tool::Tool;
use crate::toolbar::{self, Toolbar, ToolbarAction};

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
        let snapshot_path = snapshot::default_snapshot_path();
        let asset_root = snapshot::snapshot_root(&snapshot_path);
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

    fn rebuild_spatial(&mut self) {
        self.spatial.clear();
        for e in &self.board.elements {
            let (min, max) = e.aabb();
            self.spatial.insert(e.id, min, max);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn pick_snapshot_save_path(&self) -> Option<PathBuf> {
        let default_name = self
            .snapshot_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("snapshot.bin");

        rfd::FileDialog::new()
            .add_filter("Quadboard Snapshots", &["bin"])
            .set_directory(snapshot::snapshot_root(&self.snapshot_path))
            .set_file_name(default_name)
            .save_file()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn pick_snapshot_load_path(&self) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter("Quadboard Snapshots", &["bin"])
            .set_directory(snapshot::snapshot_root(&self.snapshot_path))
            .pick_file()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn copy_snapshot_assets(&self, source_root: &Path, target_root: &Path) -> std::io::Result<()> {
        if source_root == target_root {
            return Ok(());
        }

        let mut copied_paths = HashSet::new();
        for element in &self.board.elements {
            let Some(image) = element.image.as_ref() else {
                continue;
            };

            for relative_path in std::iter::once(image.asset_path.as_str())
                .chain(image.hires_asset_path.iter().map(String::as_str))
            {
                if !copied_paths.insert(relative_path.to_string()) {
                    continue;
                }

                let source_path = source_root.join(relative_path);
                let target_path = target_root.join(relative_path);
                if source_path == target_path {
                    continue;
                }

                if let Some(parent) = target_path.parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent)?;
                    }
                }

                std::fs::copy(&source_path, &target_path)?;
            }
        }

        Ok(())
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

    fn set_active_tool(&mut self, tool: Tool) {
        self.toolbar.active_tool = tool;
        self.request_redraw();
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
        self.set_active_tool(Tool::Select);
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
            let current_root = snapshot::snapshot_root(&self.snapshot_path);
            let target_root = snapshot::snapshot_root(&target_path);
            if let Err(err) = self.copy_snapshot_assets(&current_root, &target_root) {
                eprintln!("Failed to prepare snapshot assets: {err}");
                return;
            }
        }

        match snapshot::save_to_path(&self.board, &target_path) {
            Ok(path) => {
                self.snapshot_path = path.clone();
                self.snapshot_path_user_selected = true;
                let asset_root = snapshot::snapshot_root(&self.snapshot_path);
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

        match snapshot::load_from_path(&path) {
            Ok(loaded) => {
                self.snapshot_path = loaded.path.clone();
                self.snapshot_path_user_selected = true;
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
    fn update(&mut self) {
        self.update_image_ram_maintenance();
    }

    fn draw(&mut self) {
        self.draw_frame();
    }

    fn mouse_button_down_event(&mut self, button: MouseButton, x: f32, y: f32) {
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
        let drag_mode_before_up = self.input.drag_mode;
        let had_drag = drag_mode_before_up != DragMode::None;
        let had_preview = self.input.preview.is_some();
        let active_before_up = self.input.active_text_id;
        if let Some(tool) = input::on_mouse_up(
            &mut self.input,
            &mut self.board,
            &self.camera,
            self.toolbar.active_tool,
            self.screen_size,
            x,
            y,
            button,
        ) {
            self.toolbar.active_tool = tool;
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
        if had_preview || self.board.elements.len() != self.board_render_cache.all_instances().len() {
            self.mark_board_structure_dirty();
            return;
        }
        self.request_redraw();
    }

    fn mouse_motion_event(&mut self, x: f32, y: f32) {
        let previous_hover = self
            .toolbar
            .hovered_action(self.screen_size, self.input.mouse_pos.x, self.input.mouse_pos.y);
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
            &mut self.input,
            &mut self.board,
            &mut self.camera,
            self.toolbar.active_tool,
            self.screen_size,
            x,
            y,
        );

        let current_hover = self
            .toolbar
            .hovered_action(self.screen_size, self.input.mouse_pos.x, self.input.mouse_pos.y);
        if previous_hover != current_hover {
            self.request_redraw();
            return;
        }

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

