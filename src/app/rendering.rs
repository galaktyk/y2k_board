use glam::Vec2;
use miniquad::PassAction;

use crate::input::DragMode;
use crate::rendering::renderer::Renderer;
use crate::rendering::transform::{
    offset_instance, rotate_instance,
    rotate_point,
};
use crate::stats;
use crate::text::{PreparedTextDraw, TextEditSession, TextEditSnapshot};
use crate::ui::overlay;

use super::App;


// The pan glide friction coefficient, in world units per second per world unit of velocity.
const PAN_GLIDE_FRICTION_PER_SECOND: f32 = 5.0;

// When the pan velocity (in world units per second) multiplied by the zoom level is below this threshold, 
// we stop panning to prevent imperceptibly slow movement and drifting.
const PAN_GLIDE_STOP_SPEED_SCREEN: f32 = 8.0;

// Maximum delta time to apply pan glide, to prevent large jumps after long frames or when resuming from a paused state.
const PAN_GLIDE_MAX_DT_SECS: f32 = 1.0 / 50.0;

impl App {
    pub(super) fn draw_frame(&mut self) {
        self.update_frame_timing();
        self.apply_pan_glide();
        self.sync_board_render_cache();
        self.upload_scene_shapes_if_needed();

        let move_drag_offset = (self.input.drag_mode == DragMode::MoveSelected)
            .then_some(self.input.move_delta)
            .filter(|delta| delta.length_squared() > 0.0);
        let rotate_drag_preview = (self.input.drag_mode == DragMode::Rotating
            && self.input.move_origin.len() > 1)
            .then_some(self.input.rotate_delta)
            .filter(|angle| angle.abs() > 0.0)
            .zip(self.input.transform_bounds_origin.map(|bounds| bounds.center()));
        let image_draws = self.build_image_draws();

        self.ctx.begin_default_pass(PassAction::clear_color(
            139.0 / 255.0,
            153.0 / 255.0,
            180.0 / 255.0,
            1.0,
        ));

        self.renderer
            .draw_background_grid(&mut *self.ctx, &self.camera, self.screen_size);

        let board_mvp = Renderer::camera_mvp(&self.camera, self.screen_size);
        self.draw_board_layers(board_mvp, move_drag_offset, rotate_drag_preview, &image_draws);
        self.draw_text_layers(board_mvp, move_drag_offset, rotate_drag_preview);
        self.draw_overlay_layers(board_mvp, move_drag_offset, rotate_drag_preview);
        self.draw_screen_ui();

        self.ctx.end_render_pass();
        self.ctx.commit_frame();

        if self.needs_continuous_redraw() {
            self.request_redraw();
        }
    }

    fn update_frame_timing(&mut self) {
        let now = miniquad::date::now();
        let dt_ms = ((now - self.last_frame) * 1000.0) as f32;
        self.last_frame = now;
        self.frame_ms = dt_ms;

        self.fps_accum += dt_ms;
        self.fps_frames += 1;
        if self.fps_accum >= 500.0 {
            self.fps = self.fps_frames as f32 / (self.fps_accum / 1000.0);
            self.fps_accum = 0.0;
            self.fps_frames = 0;
        }
    }

    fn upload_scene_shapes_if_needed(&mut self) {
        if self.board_scene_dirty {
            self.renderer
                .upload_scene_instances(&mut *self.ctx, self.board_render_cache.all_instances());
            self.board_scene_dirty = false;
        }
    }

    fn apply_pan_glide(&mut self) {
        if self.input.panning || !self.input.has_pan_glide() {
            return;
        }

        let dt = (self.frame_ms / 1000.0).clamp(0.0, PAN_GLIDE_MAX_DT_SECS);
        if dt <= 0.0 {
            return;
        }

        self.camera.pan += self.input.pan_velocity * dt;

        let damping = (-PAN_GLIDE_FRICTION_PER_SECOND * dt).exp();
        self.input.pan_velocity *= damping;

        let screen_speed = self.input.pan_velocity.length() * self.camera.zoom;
        if screen_speed < PAN_GLIDE_STOP_SPEED_SCREEN {
            self.input.pan_velocity = Vec2::ZERO;
        }
    }

    fn draw_board_layers(
        &mut self,
        board_mvp: glam::Mat4,
        move_drag_offset: Option<Vec2>,
        rotate_drag_preview: Option<(f32, Vec2)>,
        image_draws: &[crate::rendering::renderer::PreparedImageDraw],
    ) {
        self.renderer
            .draw_image_draws(&mut *self.ctx, image_draws, board_mvp, move_drag_offset, rotate_drag_preview);

        self.renderer.draw_scene_instances(
            &mut *self.ctx,
            board_mvp,
            self.screen_size,
            move_drag_offset,
            rotate_drag_preview,
        );
    }

    fn draw_text_layers(
        &mut self,
        board_mvp: glam::Mat4,
        move_drag_offset: Option<Vec2>,
        rotate_drag_preview: Option<(f32, Vec2)>,
    ) {
        self.refresh_text_cache_if_needed();
        let text_draw = self.cached_text_draw.as_ref().unwrap();

        self.renderer.draw_scene_text_instances(
            &mut *self.ctx,
            board_mvp,
            move_drag_offset,
            rotate_drag_preview,
        );
        self.renderer.draw_scene_color_text_instances(
            &mut *self.ctx,
            board_mvp,
            move_drag_offset,
            rotate_drag_preview,
        );

        let moved_caret_pos = self.transformed_caret_position(text_draw, move_drag_offset, rotate_drag_preview);
        if let Some(world_caret) = moved_caret_pos {
            let screen_caret = self.camera.world_to_screen(world_caret, self.screen_size);
            crate::platform::ime::set_ime_candidate_pos(screen_caret.x as i32, screen_caret.y as i32);
        }
    }

    fn refresh_text_cache_if_needed(&mut self) {
        let current_edit_snapshot = self.text_edit.as_ref().map(TextEditSession::snapshot);
        let text_cache_valid = !self.text_dirty
            && self.cached_text_draw.is_some()
            && self.cached_text_edit_snapshot == current_edit_snapshot;

        if text_cache_valid {
            return;
        }

        let active_text_edit = current_edit_snapshot
            .as_ref()
            .map(TextEditSnapshot::as_active_edit);

        let prepared = self.text_system.build_text_instances(
            &mut *self.ctx,
            self.renderer.text_atlas(),
            self.renderer.emoji_atlas(),
            &self.board,
            active_text_edit,
            self.cached_text_draw.as_ref(),
        );

        self.renderer.upload_scene_text_instances(
            &mut *self.ctx,
            &prepared.mono_instances,
            &prepared.color_instances,
        );
        self.cached_text_draw = Some(prepared);
        self.cached_text_edit_snapshot = current_edit_snapshot;
        self.text_dirty = false;
    }

    fn transformed_caret_position(
        &self,
        text_draw: &PreparedTextDraw,
        move_drag_offset: Option<Vec2>,
        rotate_drag_preview: Option<(f32, Vec2)>,
    ) -> Option<Vec2> {
        if let Some((angle, center)) = rotate_drag_preview {
            return text_draw.caret_pos.map(|pos| {
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
            });
        }

        move_drag_offset
            .map(|offset| {
                text_draw.caret_pos.map(|pos| {
                    if self
                        .input
                        .active_text_id
                        .map(|id| self.board.is_selected(id))
                        .unwrap_or(false)
                    {
                        pos + offset
                    } else {
                        pos
                    }
                })
            })
            .unwrap_or(text_draw.caret_pos)
    }

    fn draw_overlay_layers(
        &mut self,
        board_mvp: glam::Mat4,
        move_drag_offset: Option<Vec2>,
        rotate_drag_preview: Option<(f32, Vec2)>,
    ) {
        self.draw_preview_overlay(board_mvp);
        self.draw_selection_overlay(board_mvp, move_drag_offset, rotate_drag_preview);
        self.draw_handle_overlay(board_mvp, move_drag_offset);
    }

    fn draw_preview_overlay(&mut self, board_mvp: glam::Mat4) {
        if let Some(ref preview) = self.input.preview {
            let preview_inst = overlay::preview_instances(preview, self.camera.zoom, 0.5);
            self.renderer
                .draw_instances(&mut *self.ctx, &preview_inst, board_mvp, self.screen_size);
        }
    }

    fn draw_selection_overlay(
        &mut self,
        board_mvp: glam::Mat4,
        move_drag_offset: Option<Vec2>,
        rotate_drag_preview: Option<(f32, Vec2)>,
    ) {
        let mut selection_inst = Vec::new();
        for element in &self.board.elements {
            if let Some(instance) = overlay::selection_instance(element, self.camera.zoom, 1.0) {
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
                selection_inst.push(overlay::selection_bounds_instance(bounds, self.camera.zoom, 1.0));
            }
        }
        if let Some(bounds) = self.input.marquee_bounds {
            selection_inst.push(overlay::marquee_instance(bounds, self.camera.zoom, 1.0));
        }
        if !selection_inst.is_empty() {
            self.renderer
                .draw_instances(&mut *self.ctx, &selection_inst, board_mvp, self.screen_size);
        }
    }

    fn draw_handle_overlay(&mut self, board_mvp: glam::Mat4, move_drag_offset: Option<Vec2>) {
        let mut handle_inst = Vec::new();
        if self.board.selected_count() > 1 {
            if let Some(bounds) = self
                .input
                .drag_selection_bounds
                .or(self.input.selection_bounds)
                .or_else(|| self.board.selected_bounds())
            {
                handle_inst.extend(crate::input::selection_bounds_handles_to_instances(
                    bounds,
                    self.camera.zoom,
                ));
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
            self.renderer
                .draw_instances(&mut *self.ctx, &handle_inst, board_mvp, self.screen_size);
        }
    }

    fn draw_screen_ui(&mut self) {
        let tb_inst = self.toolbar.build_instances(
            self.screen_size,
            self.input.mouse_pos,
            self.board.can_undo(),
            self.board.can_redo(),
        );
        let tb_icon_draws = self.toolbar.build_icon_draws(
            self.screen_size,
            self.input.mouse_pos,
            self.board.can_undo(),
            self.board.can_redo(),
            &self.toolbar_icons,
        );
        let screen_mvp = Renderer::screen_mvp(self.screen_size);
        let mut ui_text_specs = self.toolbar.build_text_specs(
            self.screen_size,
            self.input.mouse_pos,
            self.board.can_undo(),
            self.board.can_redo(),
        );

        self.renderer
            .draw_instances(&mut *self.ctx, &tb_inst, screen_mvp, self.screen_size);
        self.renderer
            .draw_image_draws(&mut *self.ctx, &tb_icon_draws, screen_mvp, None, None);

        if let Some(panel) = self.resolve_property_panel() {
            let panel_inst = crate::ui::property_panel::build_instances(
                self.screen_size,
                &panel.view,
                self.input.mouse_pos,
            );
            ui_text_specs.extend(crate::ui::property_panel::build_text_specs(
                self.screen_size,
                &panel.view,
            ));
            self.renderer
                .draw_instances(&mut *self.ctx, &panel_inst, screen_mvp, self.screen_size);
        }

        let text_draw = self.cached_text_draw.as_ref().unwrap();
        let char_count = text_draw.mono_instances.len() + text_draw.color_instances.len();
        let mut stats_text_specs = stats::build_stats_text_specs(
            self.camera.zoom,
            self.board.elements.len(),
            char_count,
            self.image_manager.atlas_count(),
            self.image_manager.atlas_capacity(),
            self.image_manager.ram_used_bytes(),
            self.image_manager.ram_capacity_bytes(),
            self.image_manager.gpu_used_bytes(),
            self.image_manager.gpu_capacity_bytes(),
            self.fps,
            self.frame_ms,
        );
        let mut stats_text_size = Vec2::ZERO;
        for spec in &stats_text_specs {
            let measured = self.text_system.measure_ui_text(spec);
            stats_text_size.x = stats_text_size.x.max(spec.pos.x + measured.x);
            stats_text_size.y = stats_text_size.y.max(spec.pos.y + measured.y);
        }
        let stats_layout = stats::build_stats_layout(stats_text_size, self.screen_size);
        for spec in &mut stats_text_specs {
            spec.pos += stats_layout.text_origin;
        }
        ui_text_specs.extend(stats_text_specs);
        self.renderer
            .draw_instances(
                &mut *self.ctx,
                &stats::build_stats_background_instances(&stats_layout),
                screen_mvp,
                self.screen_size,
            );

        let ui_text_draw = self.text_system.build_ui_text_instances(
            &mut *self.ctx,
            self.renderer.text_atlas(),
            self.renderer.emoji_atlas(),
            &ui_text_specs,
        );
        self.renderer
            .draw_text_instances(&mut *self.ctx, &ui_text_draw.mono_instances, screen_mvp);
        self.renderer
            .draw_color_text_instances(&mut *self.ctx, &ui_text_draw.color_instances, screen_mvp);
    }
}
