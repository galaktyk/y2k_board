mod construct;
mod draw;
mod shaders;
mod types;
mod uniforms;

use miniquad::*;

pub use types::{
    ImageInstanceData, InstanceData, LineInstanceData, PreparedImageDraw, TextInstanceData,
    MAX_IMAGE_INSTANCES, MAX_LINE_INSTANCES, MAX_SHAPE_INSTANCES, MAX_TEXT_INSTANCES,
};

const TEXT_ATLAS_BYTES: usize = 1024 * 1024;
const EMOJI_ATLAS_BYTES: usize = 1024 * 1024 * 4;

#[derive(Clone, Copy, Debug, Default)]
pub struct RendererMemoryStats {
    pub reserved_gpu_bytes: usize,
    pub active_scene_bytes: usize,
    pub reserved_shape_instance_bytes: usize,
    pub reserved_line_instance_bytes: usize,
    pub reserved_text_instance_bytes: usize,
    pub reserved_image_instance_bytes: usize,
    pub reserved_atlas_bytes: usize,
}

pub struct Renderer {
    shape_pipeline: Pipeline,
    shape_bindings: Bindings,
    instance_buffer: BufferId,
    line_pipeline: Pipeline,
    line_bindings: Bindings,
    line_instance_buffer: BufferId,
    scene_shape_pipeline: Pipeline,
    scene_shape_bindings: Bindings,
    scene_instance_buffer: BufferId,
    scene_shape_count: usize,
    scene_line_pipeline: Pipeline,
    scene_line_bindings: Bindings,
    scene_line_instance_buffer: BufferId,
    scene_line_count: usize,
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

impl Renderer {
    pub fn memory_stats(&self) -> RendererMemoryStats {
        let reserved_shape_instance_bytes =
            MAX_SHAPE_INSTANCES * std::mem::size_of::<InstanceData>() * 2;
        let reserved_line_instance_bytes =
            MAX_LINE_INSTANCES * std::mem::size_of::<LineInstanceData>() * 2;
        let reserved_text_instance_bytes =
            MAX_TEXT_INSTANCES * std::mem::size_of::<TextInstanceData>() * 3;
        let reserved_image_instance_bytes =
            MAX_IMAGE_INSTANCES * std::mem::size_of::<ImageInstanceData>();
        let reserved_atlas_bytes = TEXT_ATLAS_BYTES + EMOJI_ATLAS_BYTES;

        RendererMemoryStats {
            reserved_gpu_bytes: reserved_shape_instance_bytes
                + reserved_line_instance_bytes
                + reserved_text_instance_bytes
                + reserved_image_instance_bytes
                + reserved_atlas_bytes,
            active_scene_bytes: self.scene_shape_count * std::mem::size_of::<InstanceData>()
                + self.scene_line_count * std::mem::size_of::<LineInstanceData>()
                + self.scene_mono_text_count * std::mem::size_of::<TextInstanceData>()
                + self.scene_color_text_count * std::mem::size_of::<TextInstanceData>(),
            reserved_shape_instance_bytes,
            reserved_line_instance_bytes,
            reserved_text_instance_bytes,
            reserved_image_instance_bytes,
            reserved_atlas_bytes,
        }
    }
}
