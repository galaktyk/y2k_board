mod construct;
mod draw;
mod shaders;
mod types;
mod uniforms;

use miniquad::*;

pub use types::{
    ImageInstanceData, InstanceData, PreparedImageDraw, TextInstanceData,
    MAX_IMAGE_INSTANCES, MAX_SHAPE_INSTANCES, MAX_TEXT_INSTANCES,
};

pub struct Renderer {
    shape_pipeline: Pipeline,
    shape_bindings: Bindings,
    instance_buffer: BufferId,
    scene_shape_bindings: Bindings,
    scene_instance_buffer: BufferId,
    scene_shape_count: usize,
    text_pipeline: Pipeline,
    text_bindings: Bindings,
    color_text_pipeline: Pipeline,
    color_text_bindings: Bindings,
    text_instance_buffer: BufferId,
    image_pipeline: Pipeline,
    image_bindings: Bindings,
    image_instance_buffer: BufferId,
    scene_text_bindings: Bindings,
    scene_color_text_bindings: Bindings,
    scene_mono_text_buffer: BufferId,
    scene_color_text_buffer: BufferId,
    scene_mono_text_count: usize,
    scene_color_text_count: usize,
    text_atlas: TextureId,
    emoji_atlas: TextureId,
    grid_pipeline: Pipeline,
    grid_bindings: Bindings,
}
