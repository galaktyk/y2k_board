use miniquad::*;

use super::shaders::{
    COLOR_TEXT_FRAGMENT_SRC, FRAGMENT_SRC, GRID_FRAGMENT_SRC, GRID_VERTEX_SRC,
    IMAGE_FRAGMENT_SRC, TEXT_FRAGMENT_SRC, TEXT_VERTEX_SRC, VERTEX_SRC,
};
use super::{
    ImageInstanceData, InstanceData, Renderer, TextInstanceData, MAX_IMAGE_INSTANCES,
    MAX_SHAPE_INSTANCES, MAX_TEXT_INSTANCES,
};

impl Renderer {
    pub fn new(ctx: &mut dyn RenderingBackend) -> Self {
        #[rustfmt::skip]
        let quad_verts: [f32; 8] = [
            0.0, 0.0,
            1.0, 0.0,
            1.0, 1.0,
            0.0, 1.0,
        ];
        let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];

        let vertex_buf = ctx.new_buffer(
            BufferType::VertexBuffer,
            BufferUsage::Immutable,
            BufferSource::slice(&quad_verts),
        );
        eprintln!(
            "[Renderer] Quad vertex buffer created: {} bytes",
            quad_verts.len() * std::mem::size_of::<f32>()
        );
        let index_buf = ctx.new_buffer(
            BufferType::IndexBuffer,
            BufferUsage::Immutable,
            BufferSource::slice(&indices),
        );
        eprintln!(
            "[Renderer] Index buffer created: {} bytes",
            indices.len() * std::mem::size_of::<u16>()
        );

        let instance_buffer = ctx.new_buffer(
            BufferType::VertexBuffer,
            BufferUsage::Stream,
            BufferSource::empty::<InstanceData>(MAX_SHAPE_INSTANCES),
        );
        let scene_instance_buffer = ctx.new_buffer(
            BufferType::VertexBuffer,
            BufferUsage::Stream,
            BufferSource::empty::<InstanceData>(MAX_SHAPE_INSTANCES),
        );
        let text_instance_buffer = ctx.new_buffer(
            BufferType::VertexBuffer,
            BufferUsage::Stream,
            BufferSource::empty::<TextInstanceData>(MAX_TEXT_INSTANCES),
        );
        let image_instance_buffer = ctx.new_buffer(
            BufferType::VertexBuffer,
            BufferUsage::Stream,
            BufferSource::empty::<ImageInstanceData>(MAX_IMAGE_INSTANCES),
        );
        let scene_mono_text_buffer = ctx.new_buffer(
            BufferType::VertexBuffer,
            BufferUsage::Stream,
            BufferSource::empty::<TextInstanceData>(MAX_TEXT_INSTANCES),
        );
        let scene_color_text_buffer = ctx.new_buffer(
            BufferType::VertexBuffer,
            BufferUsage::Stream,
            BufferSource::empty::<TextInstanceData>(MAX_TEXT_INSTANCES),
        );
        eprintln!(
            "[Renderer] Instance buffers created: {} MB (max {} shape instances, max {} text instances)",
            ((MAX_SHAPE_INSTANCES * std::mem::size_of::<InstanceData>())
                + (MAX_TEXT_INSTANCES * std::mem::size_of::<TextInstanceData>()))
                / (1024 * 1024),
            MAX_SHAPE_INSTANCES,
            MAX_TEXT_INSTANCES
        );

        let text_atlas = ctx.new_texture(
            TextureAccess::Static,
            TextureSource::Bytes(&vec![0u8; 1024 * 1024]),
            TextureParams {
                width: 1024,
                height: 1024,
                format: TextureFormat::Alpha,
                wrap: TextureWrap::Clamp,
                min_filter: FilterMode::Nearest,
                mag_filter: FilterMode::Nearest,
                ..Default::default()
            },
        );
        let emoji_atlas = ctx.new_texture(
            TextureAccess::Static,
            TextureSource::Bytes(&vec![0u8; 1024 * 1024 * 4]),
            TextureParams {
                width: 1024,
                height: 1024,
                format: TextureFormat::RGBA8,
                wrap: TextureWrap::Clamp,
                min_filter: FilterMode::Nearest,
                mag_filter: FilterMode::Nearest,
                ..Default::default()
            },
        );

        let shape_shader = ctx
            .new_shader(
                ShaderSource::Glsl {
                    vertex: VERTEX_SRC,
                    fragment: FRAGMENT_SRC,
                },
                ShaderMeta {
                    uniforms: UniformBlockLayout {
                        uniforms: vec![
                            UniformDesc::new("u_mvp", UniformType::Mat4),
                            UniformDesc::new("u_world_per_px", UniformType::Float1),
                            UniformDesc::new("u_move_offset", UniformType::Float2),
                            UniformDesc::new("u_rotate_center", UniformType::Float2),
                            UniformDesc::new("u_rotate_angle", UniformType::Float1),
                        ],
                    },
                    images: vec![],
                },
            )
            .expect("shape shader compile failed");

        let shape_pipeline = ctx.new_pipeline(
            &[
                BufferLayout::default(),
                BufferLayout {
                    step_func: VertexStep::PerInstance,
                    ..Default::default()
                },
            ],
            &[
                VertexAttribute::with_buffer("a_pos", VertexFormat::Float2, 0),
                VertexAttribute::with_buffer("i_pos", VertexFormat::Float2, 1),
                VertexAttribute::with_buffer("i_size", VertexFormat::Float2, 1),
                VertexAttribute::with_buffer("i_color", VertexFormat::Byte4, 1),
                VertexAttribute::with_buffer("i_rotation", VertexFormat::Float1, 1),
                VertexAttribute::with_buffer("i_pack", VertexFormat::Byte4, 1),
            ],
            shape_shader,
            PipelineParams {
                color_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Value(BlendValue::SourceAlpha),
                    BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                )),
                alpha_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::One,
                    BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                )),
                ..Default::default()
            },
        );

        let shape_bindings = Bindings {
            vertex_buffers: vec![vertex_buf, instance_buffer],
            index_buffer: index_buf,
            images: vec![],
        };
        let scene_shape_bindings = Bindings {
            vertex_buffers: vec![vertex_buf, scene_instance_buffer],
            index_buffer: index_buf,
            images: vec![],
        };

        let text_shader = ctx
            .new_shader(
                ShaderSource::Glsl {
                    vertex: TEXT_VERTEX_SRC,
                    fragment: TEXT_FRAGMENT_SRC,
                },
                ShaderMeta {
                    uniforms: UniformBlockLayout {
                        uniforms: vec![
                            UniformDesc::new("u_mvp", UniformType::Mat4),
                            UniformDesc::new("u_move_offset", UniformType::Float2),
                            UniformDesc::new("u_rotate_center", UniformType::Float2),
                            UniformDesc::new("u_rotate_angle", UniformType::Float1),
                        ],
                    },
                    images: vec!["u_text_atlas".to_string()],
                },
            )
            .expect("text shader compile failed");

        let text_pipeline = Self::new_text_pipeline(ctx, text_shader);
        let text_bindings = Bindings {
            vertex_buffers: vec![vertex_buf, text_instance_buffer],
            index_buffer: index_buf,
            images: vec![text_atlas],
        };
        let scene_text_bindings = Bindings {
            vertex_buffers: vec![vertex_buf, scene_mono_text_buffer],
            index_buffer: index_buf,
            images: vec![text_atlas],
        };

        let color_text_shader = ctx
            .new_shader(
                ShaderSource::Glsl {
                    vertex: TEXT_VERTEX_SRC,
                    fragment: COLOR_TEXT_FRAGMENT_SRC,
                },
                ShaderMeta {
                    uniforms: UniformBlockLayout {
                        uniforms: vec![
                            UniformDesc::new("u_mvp", UniformType::Mat4),
                            UniformDesc::new("u_move_offset", UniformType::Float2),
                            UniformDesc::new("u_rotate_center", UniformType::Float2),
                            UniformDesc::new("u_rotate_angle", UniformType::Float1),
                        ],
                    },
                    images: vec!["u_color_atlas".to_string()],
                },
            )
            .expect("color text shader compile failed");

        let color_text_pipeline = Self::new_text_pipeline(ctx, color_text_shader);
        let color_text_bindings = Bindings {
            vertex_buffers: vec![vertex_buf, text_instance_buffer],
            index_buffer: index_buf,
            images: vec![emoji_atlas],
        };
        let scene_color_text_bindings = Bindings {
            vertex_buffers: vec![vertex_buf, scene_color_text_buffer],
            index_buffer: index_buf,
            images: vec![emoji_atlas],
        };

        let image_shader = ctx
            .new_shader(
                ShaderSource::Glsl {
                    vertex: TEXT_VERTEX_SRC,
                    fragment: IMAGE_FRAGMENT_SRC,
                },
                ShaderMeta {
                    uniforms: UniformBlockLayout {
                        uniforms: vec![
                            UniformDesc::new("u_mvp", UniformType::Mat4),
                            UniformDesc::new("u_move_offset", UniformType::Float2),
                            UniformDesc::new("u_rotate_center", UniformType::Float2),
                            UniformDesc::new("u_rotate_angle", UniformType::Float1),
                        ],
                    },
                    images: vec!["u_image_texture".to_string()],
                },
            )
            .expect("image shader compile failed");

        let image_pipeline = Self::new_text_pipeline(ctx, image_shader);
        let image_bindings = Bindings {
            vertex_buffers: vec![vertex_buf, image_instance_buffer],
            index_buffer: index_buf,
            images: vec![emoji_atlas],
        };

        #[rustfmt::skip]
        let fsq_verts: [f32; 8] = [
            -1.0, -1.0,
             1.0, -1.0,
             1.0,  1.0,
            -1.0,  1.0,
        ];
        let fsq_vert_buf = ctx.new_buffer(
            BufferType::VertexBuffer,
            BufferUsage::Immutable,
            BufferSource::slice(&fsq_verts),
        );
        eprintln!(
            "[Renderer] FSQ vertex buffer created: {} bytes",
            fsq_verts.len() * std::mem::size_of::<f32>()
        );
        let fsq_idx_buf = ctx.new_buffer(
            BufferType::IndexBuffer,
            BufferUsage::Immutable,
            BufferSource::slice(&indices),
        );
        eprintln!(
            "[Renderer] FSQ index buffer created: {} bytes",
            indices.len() * std::mem::size_of::<u16>()
        );

        let grid_shader = ctx
            .new_shader(
                ShaderSource::Glsl {
                    vertex: GRID_VERTEX_SRC,
                    fragment: GRID_FRAGMENT_SRC,
                },
                ShaderMeta {
                    uniforms: UniformBlockLayout {
                        uniforms: vec![
                            UniformDesc::new("u_inv_mvp", UniformType::Mat4),
                            UniformDesc::new("u_cell", UniformType::Float1),
                        ],
                    },
                    images: vec![],
                },
            )
            .expect("grid shader compile failed");

        let grid_pipeline = ctx.new_pipeline(
            &[BufferLayout::default()],
            &[VertexAttribute::with_buffer("a_pos", VertexFormat::Float2, 0)],
            grid_shader,
            PipelineParams {
                color_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Value(BlendValue::SourceAlpha),
                    BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                )),
                ..Default::default()
            },
        );

        let grid_bindings = Bindings {
            vertex_buffers: vec![fsq_vert_buf],
            index_buffer: fsq_idx_buf,
            images: vec![],
        };

        Self {
            shape_pipeline,
            shape_bindings,
            instance_buffer,
            scene_shape_bindings,
            scene_instance_buffer,
            scene_shape_count: 0,
            text_pipeline,
            text_bindings,
            color_text_pipeline,
            color_text_bindings,
            text_instance_buffer,
            image_pipeline,
            image_bindings,
            image_instance_buffer,
            scene_text_bindings,
            scene_color_text_bindings,
            scene_mono_text_buffer,
            scene_color_text_buffer,
            scene_mono_text_count: 0,
            scene_color_text_count: 0,
            text_atlas,
            emoji_atlas,
            grid_pipeline,
            grid_bindings,
        }
    }

    fn new_text_pipeline(ctx: &mut dyn RenderingBackend, shader: ShaderId) -> Pipeline {
        ctx.new_pipeline(
            &[
                BufferLayout::default(),
                BufferLayout {
                    step_func: VertexStep::PerInstance,
                    ..Default::default()
                },
            ],
            &[
                VertexAttribute::with_buffer("a_pos", VertexFormat::Float2, 0),
                VertexAttribute::with_buffer("i_pos", VertexFormat::Float2, 1),
                VertexAttribute::with_buffer("i_size", VertexFormat::Float2, 1),
                VertexAttribute::with_buffer("i_uv_min", VertexFormat::Short2, 1),
                VertexAttribute::with_buffer("i_uv_max", VertexFormat::Short2, 1),
                VertexAttribute::with_buffer("i_origin", VertexFormat::Short2, 1),
                VertexAttribute::with_buffer("i_color", VertexFormat::Byte4, 1),
                VertexAttribute::with_buffer("i_rotation", VertexFormat::Float1, 1),
                VertexAttribute::with_buffer("i_pack", VertexFormat::Byte4, 1),
            ],
            shader,
            PipelineParams {
                color_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Value(BlendValue::SourceAlpha),
                    BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                )),
                alpha_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::One,
                    BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                )),
                ..Default::default()
            },
        )
    }
}