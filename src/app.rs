use miniquad::*;
use glam::Vec2;
use std::time::Instant;

use crate::board::{Board, Element, ShapeType};
use crate::camera::Camera;
use crate::input::{self, InputState};
use crate::renderer::{InstanceData, Renderer};
use crate::spatial::SpatialGrid;
use crate::toolbar::{self, Toolbar};
use crate::stats;

pub struct App {
    ctx: Box<dyn RenderingBackend>,
    renderer: Renderer,
    board: Board,
    camera: Camera,
    toolbar: Toolbar,
    input: InputState,
    spatial: SpatialGrid,
    screen_size: Vec2,
    dirty: bool,
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
        Self {
            ctx,
            renderer,
            board: Board::new(),
            camera: Camera::new(),
            toolbar: Toolbar::new(),
            input: InputState::new(),
            spatial: SpatialGrid::new(),
            screen_size: Vec2::new(w, h),
            dirty: true,
            last_frame:  Instant::now(),
            frame_ms:    0.0,
            fps:         0.0,
            fps_accum:   0.0,
            fps_frames:  0,
        }
    }

    fn rebuild_spatial(&mut self) {
        self.spatial.clear();
        for e in &self.board.elements {
            let (min, max) = e.aabb();
            self.spatial.insert(e.id, min, max);
        }
    }

    fn board_instances(&self) -> Vec<InstanceData> {
        let (vis_min, vis_max) = self.camera.visible_rect(self.screen_size);
        let margin = 64.0f32;
        let vis_ids = self.spatial.query(
            vis_min - Vec2::splat(margin),
            vis_max + Vec2::splat(margin),
        );

        let mut out = Vec::new();
        for e in &self.board.elements {
            if vis_ids.contains(&e.id) {
                out.extend(toolbar::element_to_instances(e, 1.0));
            }
        }
        if let Some(ref prev) = self.input.preview {
            out.extend(toolbar::element_to_instances(prev, 0.5));
        }
        out
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

        if self.dirty {
            self.rebuild_spatial();
            self.dirty = false;
        }

        self.ctx.begin_default_pass(PassAction::clear_color(0.09, 0.10, 0.13, 1.0));

        self.renderer.draw_background_grid(&mut *self.ctx, &self.camera, self.screen_size);

        let board_inst = self.board_instances();
        let board_mvp = Renderer::camera_mvp(&self.camera, self.screen_size);
        self.renderer.draw_instances(&mut *self.ctx, &board_inst, board_mvp);

        let tb_inst = self.toolbar.build_instances(
            self.screen_size.x,
            self.board.can_undo(),
            self.board.can_redo(),
        );
        let screen_mvp = Renderer::screen_mvp(self.screen_size);
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
        input::on_mouse_down(
            &mut self.input, &mut self.board, &self.camera,
            &mut self.toolbar, self.screen_size, x, y, button,
        );
        self.dirty = true;
    }

    fn mouse_button_up_event(&mut self, button: MouseButton, x: f32, y: f32) {
        input::on_mouse_up(
            &mut self.input, &mut self.board, &self.camera,
            &self.toolbar, self.screen_size, x, y, button,
        );
        self.dirty = true;
    }

    fn mouse_motion_event(&mut self, x: f32, y: f32) {
        input::on_mouse_move(
            &mut self.input, &mut self.board, &mut self.camera,
            &self.toolbar, self.screen_size, x, y,
        );
        self.dirty = true;
    }

    fn mouse_wheel_event(&mut self, dx: f32, dy: f32) {
        input::on_scroll(&mut self.input, &mut self.camera, self.screen_size, dx, dy);
        self.dirty = true;
    }

    fn key_down_event(&mut self, keycode: KeyCode, keymods: KeyMods, _repeat: bool) {
        if keycode == KeyCode::Space {
            self.input.space_held = true;
        }
        if keycode == KeyCode::B && keymods.alt {
            let (vis_min, vis_max) = self.camera.visible_rect(self.screen_size);
            let vis_size = vis_max - vis_min;
            let mut seed: u64 = (self.board.elements.len() as u64)
                .wrapping_mul(0x9e3779b97f4a7c15)
                ^ 0xdeadbeefcafe1234;
            let rng = |s: &mut u64| -> f32 {
                *s ^= *s << 13;
                *s ^= *s >> 7;
                *s ^= *s << 17;
                *s as u32 as f32 / u32::MAX as f32
            };
            let shapes = [ShapeType::Rect, ShapeType::Ellipse, ShapeType::Line];
            for _ in 0..500 {
                let rx  = rng(&mut seed);
                let ry  = rng(&mut seed);
                let rw  = rng(&mut seed);
                let rh  = rng(&mut seed);
                let rc0 = rng(&mut seed);
                let rc1 = rng(&mut seed);
                let rc2 = rng(&mut seed);
                let shape = shapes[(seed % 3) as usize];
                let pos   = vis_min + Vec2::new(rx * vis_size.x, ry * vis_size.y);
                let size  = Vec2::new(100.0 + rw * 300.0, 100.0 + rh * 300.0);
                let color = [rc0 * 0.7 + 0.3, rc1 * 0.7 + 0.3, rc2 * 0.7 + 0.3, 0.85];
                let id    = self.board.next_id();
                self.board.elements.push(Element { id, shape, pos, size, color, selected: false });
            }
            println!("Alt+B: spawned 500 shapes | total elements: {}", self.board.elements.len());
            self.dirty = true;
            return;
        }
        input::on_key_down(&mut self.input, &mut self.board, keycode, keymods);
        self.dirty = true;
    }

    fn key_up_event(&mut self, keycode: KeyCode, _keymods: KeyMods) {
        if keycode == KeyCode::Space {
            self.input.space_held = false;
        }
    }

    fn resize_event(&mut self, width: f32, height: f32) {
        self.screen_size = Vec2::new(width, height);
        self.dirty = true;
    }
}

