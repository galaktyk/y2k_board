use glam::Vec2;
use miniquad::*;

use crate::camera::Camera;

use super::uniforms::{GridUniforms, ShapeUniforms, TextUniforms};
use super::{InstanceData, PreparedImageDraw, Renderer, TextInstanceData};

impl Renderer {
    pub fn camera_mvp(camera: &Camera, screen_size: Vec2) -> glam::Mat4 {
        let w = screen_size.x;
        let h = screen_size.y;
        let z = camera.zoom;
        let px = camera.pan.x;
        let py = camera.pan.y;
        let l = px - w * 0.5 / z;
        let r = px + w * 0.5 / z;
        let b = py + h * 0.5 / z;
        let t = py - h * 0.5 / z;
        glam::Mat4::orthographic_rh_gl(l, r, b, t, -1.0, 1.0)
    }

    pub fn screen_mvp(screen_size: Vec2) -> glam::Mat4 {
        glam::Mat4::orthographic_rh_gl(0.0, screen_size.x, screen_size.y, 0.0, -1.0, 1.0)
    }

    pub fn draw_background_grid(
        &mut self,
        ctx: &mut dyn RenderingBackend,
        camera: &Camera,
        screen_size: Vec2,
    ) {
        let raw = 64.0 / camera.zoom;
        let exp = raw.log2().floor();
        let cell_size = (2.0f32).powf(exp).max(4.0);

        let mvp = Self::camera_mvp(camera, screen_size);
        let inv = mvp.inverse();

        ctx.apply_pipeline(&self.grid_pipeline);
        ctx.apply_bindings(&self.grid_bindings);
        ctx.apply_uniforms(UniformsSource::table(&GridUniforms {
            u_inv_mvp: inv.to_cols_array_2d(),
            u_cell: cell_size,
        }));
        ctx.draw(0, 6, 1);
    }

    pub fn draw_instances(
        &mut self,
        ctx: &mut dyn RenderingBackend,
        instances: &[InstanceData],
        mvp: glam::Mat4,
        screen_size: Vec2,
    ) {
        if instances.is_empty() {
            return;
        }

        let world_per_px = Self::world_per_px(mvp, screen_size);
        ctx.buffer_update(self.instance_buffer, BufferSource::slice(instances));
        ctx.apply_pipeline(&self.shape_pipeline);
        ctx.apply_bindings(&self.shape_bindings);
        ctx.apply_uniforms(UniformsSource::table(&ShapeUniforms {
            u_mvp: mvp.to_cols_array_2d(),
            u_world_per_px: world_per_px,
            u_move_offset: [0.0, 0.0],
            u_rotate_center: [0.0, 0.0],
            u_rotate_angle: 0.0,
        }));
        ctx.draw(0, 6, instances.len() as i32);
    }

    pub fn upload_scene_instances(&mut self, ctx: &mut dyn RenderingBackend, instances: &[InstanceData]) {
        self.scene_shape_count = instances.len();
        if instances.is_empty() {
            return;
        }

        ctx.buffer_update(self.scene_instance_buffer, BufferSource::slice(instances));
    }

    pub fn draw_scene_instances(
        &mut self,
        ctx: &mut dyn RenderingBackend,
        mvp: glam::Mat4,
        screen_size: Vec2,
        move_drag_offset: Option<Vec2>,
        rotate_drag_preview: Option<(f32, Vec2)>,
    ) {
        if self.scene_shape_count == 0 {
            return;
        }

        let world_per_px = Self::world_per_px(mvp, screen_size);
        let mut u_move_offset = [0.0, 0.0];
        let mut u_rotate_center = [0.0, 0.0];
        let mut u_rotate_angle = 0.0;
        
        if let Some(offset) = move_drag_offset {
            u_move_offset = offset.to_array();
        } else if let Some((angle, center)) = rotate_drag_preview {
            u_rotate_center = center.to_array();
            u_rotate_angle = angle;
        }

        ctx.apply_pipeline(&self.shape_pipeline);
        ctx.apply_bindings(&self.scene_shape_bindings);
        ctx.apply_uniforms(UniformsSource::table(&ShapeUniforms {
            u_mvp: mvp.to_cols_array_2d(),
            u_world_per_px: world_per_px,
            u_move_offset,
            u_rotate_center,
            u_rotate_angle,
        }));
        ctx.draw(0, 6, self.scene_shape_count as i32);
    }

    pub fn draw_text_instances(
        &mut self,
        ctx: &mut dyn RenderingBackend,
        instances: &[TextInstanceData],
        mvp: glam::Mat4,
    ) {
        self.draw_text_instances_with_transform(ctx, instances, mvp, None, None);
    }

    pub fn draw_text_instances_with_transform(
        &mut self,
        ctx: &mut dyn RenderingBackend,
        instances: &[TextInstanceData],
        mvp: glam::Mat4,
        move_drag_offset: Option<Vec2>,
        rotate_drag_preview: Option<(f32, Vec2)>,
    ) {
        if instances.is_empty() {
            return;
        }

        let mut u_move_offset = [0.0, 0.0];
        let mut u_rotate_center = [0.0, 0.0];
        let mut u_rotate_angle = 0.0;

        if let Some(offset) = move_drag_offset {
            u_move_offset = offset.to_array();
        } else if let Some((angle, center)) = rotate_drag_preview {
            u_rotate_center = center.to_array();
            u_rotate_angle = angle;
        }

        ctx.buffer_update(self.text_instance_buffer, BufferSource::slice(instances));
        ctx.apply_pipeline(&self.text_pipeline);
        ctx.apply_bindings(&self.text_bindings);
        ctx.apply_uniforms(UniformsSource::table(&TextUniforms {
            u_mvp: mvp.to_cols_array_2d(),
            u_move_offset,
            u_rotate_center,
            u_rotate_angle,
        }));
        ctx.draw(0, 6, instances.len() as i32);
    }

    pub fn upload_scene_text_instances(
        &mut self,
        ctx: &mut dyn RenderingBackend,
        mono_instances: &[TextInstanceData],
        color_instances: &[TextInstanceData],
    ) {
        self.scene_mono_text_count = mono_instances.len();
        self.scene_color_text_count = color_instances.len();

        if !mono_instances.is_empty() {
            ctx.buffer_update(self.scene_mono_text_buffer, BufferSource::slice(mono_instances));
        }
        if !color_instances.is_empty() {
            ctx.buffer_update(self.scene_color_text_buffer, BufferSource::slice(color_instances));
        }
    }

    pub fn draw_scene_text_instances(
        &mut self,
        ctx: &mut dyn RenderingBackend,
        mvp: glam::Mat4,
        move_drag_offset: Option<Vec2>,
        rotate_drag_preview: Option<(f32, Vec2)>,
    ) {
        if self.scene_mono_text_count == 0 {
            return;
        }

        let mut u_move_offset = [0.0, 0.0];
        let mut u_rotate_center = [0.0, 0.0];
        let mut u_rotate_angle = 0.0;
        
        if let Some(offset) = move_drag_offset {
            u_move_offset = offset.to_array();
        } else if let Some((angle, center)) = rotate_drag_preview {
            u_rotate_center = center.to_array();
            u_rotate_angle = angle;
        }

        ctx.apply_pipeline(&self.text_pipeline);
        ctx.apply_bindings(&self.scene_text_bindings);
        ctx.apply_uniforms(UniformsSource::table(&TextUniforms {
            u_mvp: mvp.to_cols_array_2d(),
            u_move_offset,
            u_rotate_center,
            u_rotate_angle,
        }));
        ctx.draw(0, 6, self.scene_mono_text_count as i32);
    }

    pub fn draw_color_text_instances(
        &mut self,
        ctx: &mut dyn RenderingBackend,
        instances: &[TextInstanceData],
        mvp: glam::Mat4,
    ) {
        if instances.is_empty() {
            return;
        }

        ctx.buffer_update(self.text_instance_buffer, BufferSource::slice(instances));
        ctx.apply_pipeline(&self.color_text_pipeline);
        ctx.apply_bindings(&self.color_text_bindings);
        ctx.apply_uniforms(UniformsSource::table(&TextUniforms {
            u_mvp: mvp.to_cols_array_2d(),
            u_move_offset: [0.0, 0.0],
            u_rotate_center: [0.0, 0.0],
            u_rotate_angle: 0.0,
        }));
        ctx.draw(0, 6, instances.len() as i32);
    }

    pub fn draw_scene_color_text_instances(
        &mut self,
        ctx: &mut dyn RenderingBackend,
        mvp: glam::Mat4,
        move_drag_offset: Option<Vec2>,
        rotate_drag_preview: Option<(f32, Vec2)>,
    ) {
        if self.scene_color_text_count == 0 {
            return;
        }

        let mut u_move_offset = [0.0, 0.0];
        let mut u_rotate_center = [0.0, 0.0];
        let mut u_rotate_angle = 0.0;
        
        if let Some(offset) = move_drag_offset {
            u_move_offset = offset.to_array();
        } else if let Some((angle, center)) = rotate_drag_preview {
            u_rotate_center = center.to_array();
            u_rotate_angle = angle;
        }

        ctx.apply_pipeline(&self.color_text_pipeline);
        ctx.apply_bindings(&self.scene_color_text_bindings);
        ctx.apply_uniforms(UniformsSource::table(&TextUniforms {
            u_mvp: mvp.to_cols_array_2d(),
            u_move_offset,
            u_rotate_center,
            u_rotate_angle,
        }));
        ctx.draw(0, 6, self.scene_color_text_count as i32);
    }

    pub fn draw_image_draws(
        &mut self,
        ctx: &mut dyn RenderingBackend,
        draws: &[PreparedImageDraw],
        mvp: glam::Mat4,
        move_drag_offset: Option<Vec2>,
        rotate_drag_preview: Option<(f32, Vec2)>,
    ) {
        if draws.is_empty() {
            return;
        }

        let mut u_move_offset = [0.0, 0.0];
        let mut u_rotate_center = [0.0, 0.0];
        let mut u_rotate_angle = 0.0;

        if let Some(offset) = move_drag_offset {
            u_move_offset = offset.to_array();
        } else if let Some((angle, center)) = rotate_drag_preview {
            u_rotate_center = center.to_array();
            u_rotate_angle = angle;
        }

        let mut start = 0usize;
        let mut batch = Vec::new();

        while start < draws.len() {
            let texture = draws[start].texture;
            let mut end = start;
            batch.clear();
            while end < draws.len() && draws[end].texture == texture {
                batch.push(draws[end].instance);
                end += 1;
            }

            ctx.buffer_update(self.image_instance_buffer, BufferSource::slice(&batch));
            self.image_bindings.images[0] = texture;
            ctx.apply_pipeline(&self.image_pipeline);
            ctx.apply_bindings(&self.image_bindings);
            ctx.apply_uniforms(UniformsSource::table(&TextUniforms {
                u_mvp: mvp.to_cols_array_2d(),
                u_move_offset,
                u_rotate_center,
                u_rotate_angle,
            }));
            ctx.draw(0, 6, batch.len() as i32);

            start = end;
        }
    }

    pub fn text_atlas(&self) -> TextureId {
        self.text_atlas
    }

    pub fn emoji_atlas(&self) -> TextureId {
        self.emoji_atlas
    }

    fn world_per_px(mvp: glam::Mat4, screen_size: Vec2) -> f32 {
        let pixels_per_world_x = (mvp.x_axis.x * screen_size.x * 0.5).abs();
        let pixels_per_world_y = (mvp.y_axis.y * screen_size.y * 0.5).abs();
        let pixels_per_world = pixels_per_world_x.min(pixels_per_world_y).max(0.0001);
        1.0 / pixels_per_world
    }
}