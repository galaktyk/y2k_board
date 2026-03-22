use glam::Vec2;
use miniquad::PassAction;

use crate::board::ElementTransform;
use crate::input::{DragMode, COMPUTE_TEXT_LAYOUT_DEBOUNCE};
use crate::rendering::renderer::Renderer;
use crate::rendering::transform::{
    offset_instance, rotate_instance,
    rotate_point,
};
use crate::stats;
use crate::text::{PreparedTextDraw, TextEditSession};
use crate::ui::overlay;

use super::App;


// The pan glide friction coefficient, in world units per second per world unit of velocity.
const PAN_GLIDE_FRICTION_PER_SECOND: f32 = 5.0;

// When the pan velocity (in world units per second) multiplied by the zoom level is below this threshold, 
// we stop panning to prevent imperceptibly slow movement and drifting.
const PAN_GLIDE_STOP_SPEED_SCREEN: f32 = 8.0;

// Maximum delta time to apply pan glide, to prevent large jumps after long frames or when resuming from a paused state.
const PAN_GLIDE_MAX_DT_SECS: f32 = 1.0 / 50.0;
const RESIZE_TEXT_RECOMPUTE_BATCH: usize = 8;

impl App {
    pub(super) fn draw_frame(&mut self) {
        self.update_frame_timing();
        self.apply_pan_glide();
        self.sync_board_render_cache();
        self.preview_connected_lines_for_drag();
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
        self.draw_screen_ui(move_drag_offset, rotate_drag_preview);

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

    /// Called every render frame (not on mouse-move events) to keep connected-line
    /// positions visually correct while a transform preview is in progress.
    /// `sync_board_render_cache` runs first and may rebuild instances from the true
    /// board state; this then patches the in-memory buffer with preview positions,
    /// and `upload_scene_shapes_if_needed` does **one** GPU upload per frame.
    fn preview_connected_lines_for_drag(&mut self) {
        let Some((selected_ids, preview_transforms)) = self.connected_line_preview_state() else {
            return;
        };
        let patches = self
            .board
            .compute_drag_line_previews(&selected_ids, &preview_transforms);
        if !patches.is_empty() {
            self.board_render_cache.patch_element_positions(&patches);
            // Force a GPU re-upload this frame so the patched positions reach the shader.
            self.board_scene_dirty = true;
        }
    }

    fn connected_line_preview_state(
        &self,
    ) -> Option<(
        std::collections::HashSet<u64>,
        std::collections::HashMap<u64, ElementTransform>,
    )> {
        let selected_ids: std::collections::HashSet<u64> =
            self.input.move_origin.iter().map(|&(id, _, _, _)| id).collect();
        if selected_ids.is_empty() {
            return None;
        }

        match self.input.drag_mode {
            DragMode::MoveSelected => {
                let delta = self.input.move_delta;
                if delta.length_squared() == 0.0 {
                    return None;
                }

                let preview_transforms = self
                    .input
                    .move_origin
                    .iter()
                    .map(|&(id, pos, size, rotation)| {
                        (
                            id,
                            ElementTransform::new(pos + delta, size, rotation),
                        )
                    })
                    .collect();
                Some((selected_ids, preview_transforms))
            }
            DragMode::Rotating if self.input.move_origin.len() > 1 => {
                let angle = self.input.rotate_delta;
                let center = self.input.transform_bounds_origin.map(|bounds| bounds.center())?;
                if angle.abs() == 0.0 {
                    return None;
                }

                let preview_transforms = self
                    .input
                    .move_origin
                    .iter()
                    .map(|&(id, pos, size, rotation)| {
                        let rotated_center = rotate_point(pos + size * 0.5, center, angle);
                        (
                            id,
                            ElementTransform::new(rotated_center - size * 0.5, size, rotation + angle),
                        )
                    })
                    .collect();
                Some((selected_ids, preview_transforms))
            }
            DragMode::Rotating | DragMode::ResizingHandle(_) => {
                let mut changed = false;
                let preview_transforms: std::collections::HashMap<u64, ElementTransform> = self
                    .input
                    .move_origin
                    .iter()
                    .filter_map(|&(id, orig_pos, orig_size, orig_rotation)| {
                        let element = self.board.element(id)?;
                        let before = ElementTransform::new(orig_pos, orig_size, orig_rotation);
                        let after = ElementTransform::new(element.pos, element.size, element.rotation);
                        changed |= before != after;
                        Some((id, after))
                    })
                    .collect();

                changed.then_some((selected_ids, preview_transforms))
            }
            DragMode::MarqueeSelect | DragMode::CreatingConnection | DragMode::None => None,
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
        let Some(text_draw) = self.cached_text_draw.as_ref() else {
            return;
        };

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
        self.promote_resize_text_recompute_if_due();

        let text_cache_valid = !self.text_dirty
            && self.cached_text_draw.is_some()
            && match (&self.cached_text_edit_snapshot, self.text_edit.as_ref()) {
                (None, None) => true,
                (Some(snapshot), Some(edit)) => snapshot.matches_session(edit),
                _ => false,
            };

        if text_cache_valid {
            return;
        }

        let active_text_edit = self.text_edit.as_ref().map(TextEditSession::as_active_edit);
        let current_edit_snapshot = self.text_edit.as_ref().map(TextEditSession::snapshot);

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

    fn promote_resize_text_recompute_if_due(&mut self) {
        if !self.input.has_pending_resize_text_recompute() {
            return;
        }

        let now = miniquad::date::now();
        if now - self.input.last_resize_text_bump < COMPUTE_TEXT_LAYOUT_DEBOUNCE {
            return;
        }

        let mut promoted = 0usize;
        while promoted < RESIZE_TEXT_RECOMPUTE_BATCH {
            let Some(id) = self.input.pop_resize_text_recompute() else {
                break;
            };
            let Some(element) = self.board.element_mut(id) else {
                continue;
            };
            if element.text.is_none() {
                continue;
            }

            element.bump_text_generation();
            promoted += 1;
        }

        if promoted > 0 {
            self.input.last_resize_text_bump = now;
            self.text_dirty = true;
        }
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

        if let Some(connection_drag) = self.input.connection_drag {
            let preview_line = overlay::connection_preview_instance(
                connection_drag.start_world,
                connection_drag.end_world,
                crate::board::DEFAULT_LINE_COLOR,
                crate::board::DEFAULT_LINE_STROKE_WIDTH,
                0.85,
            );
            self.renderer
                .draw_instances(&mut *self.ctx, &[preview_line], board_mvp, self.screen_size);
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
                    let mut helper_instances = crate::input::connection_helpers_to_instances(
                        e,
                        self.camera.zoom,
                    );
                    if let Some(offset) = move_drag_offset {
                        for instance in &mut instances {
                            *instance = offset_instance(*instance, offset);
                        }
                        for instance in &mut helper_instances {
                            *instance = offset_instance(*instance, offset);
                        }
                    }
                    handle_inst.extend(instances);
                    handle_inst.extend(helper_instances);
                }
            }
        }
        if !handle_inst.is_empty() {
            self.renderer
                .draw_instances(&mut *self.ctx, &handle_inst, board_mvp, self.screen_size);
        }
    }

    fn draw_screen_ui(
        &mut self,
        move_drag_offset: Option<Vec2>,
        rotate_drag_preview: Option<(f32, Vec2)>,
    ) {
        let mut ui_bg_instances = self.toolbar.build_instances(
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

        if self.board.selected_count() > 1 {
            if let Some(bounds) = self
                .input
                .drag_selection_bounds
                .or(self.input.selection_bounds)
                .or_else(|| self.board.selected_bounds())
            {
                let handles = crate::input::get_selection_bounds_handles(bounds, self.camera.zoom);
                if handles.len() > 4 {
                    let pos = handles[4];
                    let screen_pos = self.camera.world_to_screen(pos, self.screen_size);
                    ui_text_specs.push(crate::text::UiTextSpec::top_center("↻", screen_pos - glam::Vec2::new(0.0, 12.0), 24.0, [1.0, 1.0, 1.0, 1.0]));
                }
            }
        } else {
            for e in &self.board.elements {
                if e.selected {
                    if let Some(handles) = crate::input::get_element_handles(e, self.camera.zoom) {
                        if handles.len() > 4 {
                            let mut pos = handles[4];
                            if let Some(offset) = move_drag_offset {
                                pos += offset;
                            }
                            if let Some((angle, center)) = rotate_drag_preview {
                                let rel = pos - center;
                                let c = angle.cos();
                                let s = angle.sin();
                                pos = center + glam::Vec2::new(rel.x * c - rel.y * s, rel.x * s + rel.y * c);
                            }
                            let screen_pos = self.camera.world_to_screen(pos, self.screen_size);
                            ui_text_specs.push(crate::text::UiTextSpec::top_center("↻", screen_pos - glam::Vec2::new(0.0, 12.0), 24.0, [1.0, 1.0, 1.0, 1.0]));
                        }
                    }
                }
            }
        }

        if let Some(panel) = self.resolve_property_panel() {
            let mut panel_inst = crate::ui::property_panel::build_instances(
                self.screen_size,
                &panel.view,
                self.input.mouse_pos,
            );
            ui_bg_instances.append(&mut panel_inst);
            ui_text_specs.extend(crate::ui::property_panel::build_text_specs(
                self.screen_size,
                &panel.view,
            ));
        }

        let Some(text_draw) = self.cached_text_draw.as_ref() else {
            return;
        };
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
        let mut stats_bg = stats::build_stats_background_instances(&stats_layout);
        ui_bg_instances.append(&mut stats_bg);

        self.renderer
            .draw_instances(&mut *self.ctx, &ui_bg_instances, screen_mvp, self.screen_size);
        self.renderer
            .draw_image_draws(&mut *self.ctx, &tb_icon_draws, screen_mvp, None, None);

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
