use crate::camera::Camera;
use glam::Vec2;
use miniquad::*;

// ── Instance data ─────────────────────────────────────────────────────────────

/// One instance in the GPU buffer.
/// Layout must match the vertex shader attributes.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct InstanceData {
    /// Top-left position in whichever space the pass uses.
    pub pos: [f32; 2],
    /// Width/height (or dx/dy for lines).
    pub size: [f32; 2],
    /// Rotation in radians
    pub rotation: f32,
    /// RGBA colour.
    pub color: [f32; 4],
    /// 0 = rect, 1 = ellipse, 2 = line.
    pub shape_type: f32,
    /// Packed alpha multiplier (for previews), and a spare float.
    pub alpha: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TextInstanceData {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub origin: [f32; 2],
    pub rotation: f32,
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    pub color: [f32; 4],
}

// ── Shaders ───────────────────────────────────────────────────────────────────

const VERTEX_SRC: &str = r#"#version 100
// per-vertex (unit quad)
attribute vec2 a_pos;      // 0..1 range
// per-instance
attribute vec2 i_pos;
attribute vec2 i_size;
attribute float i_rotation;
attribute vec4 i_color;
attribute float i_shape;
attribute float i_alpha;

uniform mat4 u_mvp;

varying vec2 v_uv;
varying vec4 v_color;
varying float v_shape;
varying float v_alpha;
varying vec2 v_line_p;
varying float v_line_len;
varying vec2 v_size;

void main() {
    vec2 world_pos;
    if (i_shape > 1.5 && i_shape < 2.5) {
        // Line
        vec2 dir = i_size;
        float len = length(dir);
        if (len < 0.0001) { len = 0.0001; }
        vec2 u = dir / len;
        vec2 v = vec2(-u.y, u.x);
        
        float margin = 8.0; // half-thickness + antialiasing
        
        vec2 p = vec2(
            mix(-margin, len + margin, a_pos.x),
            mix(-margin, margin, a_pos.y)
        );
        world_pos = i_pos + p.x * u + p.y * v;
        
        v_line_p = p;
        v_line_len = len;
        v_uv = a_pos;
    } else {
        world_pos = i_pos + a_pos * i_size;
        v_uv = a_pos;
    }

    vec2 center = i_pos + i_size * 0.5;
    float c = cos(i_rotation);
    float s = sin(i_rotation);
    mat2 rot = mat2(c, s, -s, c);
    world_pos = center + rot * (world_pos - center);

    gl_Position   = u_mvp * vec4(world_pos, 0.0, 1.0);
    v_color = i_color;
    v_shape = i_shape;
    v_alpha = i_alpha;
    v_size  = i_size;
}
"#;

const FRAGMENT_SRC: &str = r#"#version 100
precision highp float;

varying vec2 v_uv;
varying vec4 v_color;
varying float v_shape;
varying float v_alpha;
varying vec2 v_line_p;
varying float v_line_len;
varying vec2 v_size;

void main() {
    float alpha = v_color.a * v_alpha;
    vec2 uv = v_uv;          // 0..1

    if (v_shape < 0.5) {
        // Rect — solid fill with 1.5px anti-aliased edge
        vec2 d = min(uv, 1.0 - uv);
        float edge = min(d.x, d.y);
        float a = smoothstep(0.0, 0.01, edge);
        gl_FragColor = vec4(v_color.rgb, alpha * a);

    } else if (v_shape < 1.5) {
        // Ellipse SDF
        vec2 c = uv * 2.0 - 1.0;           // -1..1
        float d = length(c);
        float a = smoothstep(1.0, 0.98, d);
        gl_FragColor = vec4(v_color.rgb, alpha * a);

    } else if (v_shape < 2.5) {
        // Line
        vec2 p = v_line_p;
        float dx = p.x - clamp(p.x, 0.0, v_line_len);
        float d = length(vec2(dx, p.y));
        float thickness = 4.0; // visual half-thickness
        float a = 1.0 - smoothstep(thickness - 1.0, thickness + 1.0, d);
        gl_FragColor = vec4(v_color.rgb, alpha * a);

    } else if (v_shape < 3.5) {
        // Rect border outline
        vec2 dist = min(uv, 1.0 - uv) * v_size;
        float edge = min(dist.x, dist.y);
        float border = 2.5;
        float a = smoothstep(0.0, 1.0, edge) * (1.0 - smoothstep(border, border + 1.0, edge));
        gl_FragColor = vec4(v_color.rgb, alpha * a);

    } else if (v_shape < 4.5) {
        // Ellipse border outline
        vec2 c = uv * 2.0 - 1.0;
        float d = length(c);
        float r = min(v_size.x, v_size.y) * 0.5;
        float br = 2.5 / r;
        float outer = 1.0 - smoothstep(1.0, 1.0 + br * 0.5, d);
        float inner = smoothstep(1.0 - br - 1.0 / r, 1.0 - br, d);
        gl_FragColor = vec4(v_color.rgb, alpha * outer * inner);

    } else {
        gl_FragColor = vec4(0.0, 0.0, 0.0, 0.0);
    }
}
"#;

const TEXT_VERTEX_SRC: &str = r#"#version 100
attribute vec2 a_pos;
attribute vec2 i_pos;
attribute vec2 i_size;
attribute vec2 i_origin;
attribute float i_rotation;
attribute vec2 i_uv_min;
attribute vec2 i_uv_max;
attribute vec4 i_color;

uniform mat4 u_mvp;

varying vec2 v_uv;
varying vec4 v_color;

void main() {
    vec2 world_pos = i_pos + a_pos * i_size;
    float c = cos(i_rotation);
    float s = sin(i_rotation);
    mat2 rot = mat2(c, s, -s, c);
    world_pos = i_origin + rot * (world_pos - i_origin);

    v_uv = mix(i_uv_min, i_uv_max, a_pos);
    v_color = i_color;
    gl_Position = u_mvp * vec4(world_pos, 0.0, 1.0);
}
"#;

const TEXT_FRAGMENT_SRC: &str = r#"#version 100
precision highp float;

varying vec2 v_uv;
varying vec4 v_color;

uniform sampler2D u_text_atlas;

void main() {
    float mask = texture2D(u_text_atlas, v_uv).a;
    if (mask <= 0.0) {
        discard;
    }
    gl_FragColor = vec4(v_color.rgb, v_color.a * mask);
}
"#;

const COLOR_TEXT_FRAGMENT_SRC: &str = r#"#version 100
precision highp float;

varying vec2 v_uv;
varying vec4 v_color;

uniform sampler2D u_color_atlas;

void main() {
    vec4 sample_color = texture2D(u_color_atlas, v_uv);
    if (sample_color.a <= 0.0) {
        discard;
    }
    gl_FragColor = vec4(sample_color.rgb * v_color.rgb, sample_color.a * v_color.a);
}
"#;

// ── Grid shaders ─────────────────────────────────────────────────────────────
//
// The grid is a fullscreen quad; we draw the grid pattern in the fragment
// shader using fract() math, avoiding per-line draw calls.

const GRID_VERTEX_SRC: &str = r#"#version 100
attribute vec2 a_pos;

uniform mat4 u_inv_mvp;   // maps clip coords back to world space
uniform float u_cell;
varying vec2 v_cell;

void main() {
    gl_Position = vec4(a_pos, 0.0, 1.0);
    vec4 w = u_inv_mvp * vec4(a_pos, 0.0, 1.0);
    v_cell = (w.xy / w.w) / u_cell;
}
"#;

const GRID_FRAGMENT_SRC: &str = r#"#version 100
precision highp float;

varying vec2 v_cell;

void main() {
    vec2 f = fract(v_cell);
    vec2 d = min(f, 1.0 - f);
    float line = min(d.x, d.y);
    float a = 1.0 - smoothstep(0.0, 0.04, line);
    gl_FragColor = vec4(0.25, 0.26, 0.28, a * 0.6);
}
"#;

// ── Renderer ─────────────────────────────────────────────────────────────────

pub struct Renderer {
    // shape pipeline
    shape_pipeline: Pipeline,
    shape_bindings: Bindings,
    instance_buffer: BufferId,

    // text pipelines
    text_pipeline: Pipeline,
    text_bindings: Bindings,
    color_text_pipeline: Pipeline,
    color_text_bindings: Bindings,
    text_instance_buffer: BufferId,
    text_atlas: TextureId,
    emoji_atlas: TextureId,

    // grid pipeline
    grid_pipeline: Pipeline,
    grid_bindings: Bindings,
}

impl Renderer {
    pub fn new(ctx: &mut dyn RenderingBackend) -> Self {
        // ── Unit quad (vertex buffer) ─────────────────────────────────────
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

        // ── Instance buffer ───────────────────────────────────────────────
        let max_instances = 100_000usize;
        let instance_buffer = ctx.new_buffer(
            BufferType::VertexBuffer,
            BufferUsage::Stream,
            BufferSource::empty::<InstanceData>(max_instances),
        );
        let text_instance_buffer = ctx.new_buffer(
            BufferType::VertexBuffer,
            BufferUsage::Stream,
            BufferSource::empty::<TextInstanceData>(max_instances),
        );
        eprintln!(
            "[Renderer] Instance buffer created: {} MB (max {} instances)",
            (max_instances * std::mem::size_of::<InstanceData>()) / (1024 * 1024),
            max_instances
        );

        let text_atlas = ctx.new_texture(
            TextureAccess::Static,
            TextureSource::Bytes(&vec![0u8; 1024 * 1024]),
            TextureParams {
                width: 1024,
                height: 1024,
                format: TextureFormat::Alpha,
                wrap: TextureWrap::Clamp,
                min_filter: FilterMode::Linear,
                mag_filter: FilterMode::Linear,
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
                min_filter: FilterMode::Linear,
                mag_filter: FilterMode::Linear,
                ..Default::default()
            },
        );

        // ── Shape pipeline ────────────────────────────────────────────────
        let shape_shader = ctx
            .new_shader(
                ShaderSource::Glsl {
                    vertex: VERTEX_SRC,
                    fragment: FRAGMENT_SRC,
                },
                ShaderMeta {
                    uniforms: UniformBlockLayout {
                        uniforms: vec![UniformDesc::new("u_mvp", UniformType::Mat4)],
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
                // buffer 0: a_pos
                VertexAttribute::with_buffer("a_pos", VertexFormat::Float2, 0),
                // buffer 1: per-instance
                VertexAttribute::with_buffer("i_pos", VertexFormat::Float2, 1),
                VertexAttribute::with_buffer("i_size", VertexFormat::Float2, 1),
                VertexAttribute::with_buffer("i_rotation", VertexFormat::Float1, 1),
                VertexAttribute::with_buffer("i_color", VertexFormat::Float4, 1),
                VertexAttribute::with_buffer("i_shape", VertexFormat::Float1, 1),
                VertexAttribute::with_buffer("i_alpha", VertexFormat::Float1, 1),
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

        let text_shader = ctx
            .new_shader(
                ShaderSource::Glsl {
                    vertex: TEXT_VERTEX_SRC,
                    fragment: TEXT_FRAGMENT_SRC,
                },
                ShaderMeta {
                    uniforms: UniformBlockLayout {
                        uniforms: vec![UniformDesc::new("u_mvp", UniformType::Mat4)],
                    },
                    images: vec!["u_text_atlas".to_string()],
                },
            )
            .expect("text shader compile failed");

        let text_pipeline = ctx.new_pipeline(
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
                VertexAttribute::with_buffer("i_origin", VertexFormat::Float2, 1),
                VertexAttribute::with_buffer("i_rotation", VertexFormat::Float1, 1),
                VertexAttribute::with_buffer("i_uv_min", VertexFormat::Float2, 1),
                VertexAttribute::with_buffer("i_uv_max", VertexFormat::Float2, 1),
                VertexAttribute::with_buffer("i_color", VertexFormat::Float4, 1),
            ],
            text_shader,
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

        let text_bindings = Bindings {
            vertex_buffers: vec![vertex_buf, text_instance_buffer],
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
                        uniforms: vec![UniformDesc::new("u_mvp", UniformType::Mat4)],
                    },
                    images: vec!["u_color_atlas".to_string()],
                },
            )
            .expect("color text shader compile failed");

        let color_text_pipeline = ctx.new_pipeline(
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
                VertexAttribute::with_buffer("i_origin", VertexFormat::Float2, 1),
                VertexAttribute::with_buffer("i_rotation", VertexFormat::Float1, 1),
                VertexAttribute::with_buffer("i_uv_min", VertexFormat::Float2, 1),
                VertexAttribute::with_buffer("i_uv_max", VertexFormat::Float2, 1),
                VertexAttribute::with_buffer("i_color", VertexFormat::Float4, 1),
            ],
            color_text_shader,
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

        let color_text_bindings = Bindings {
            vertex_buffers: vec![vertex_buf, text_instance_buffer],
            index_buffer: index_buf,
            images: vec![emoji_atlas],
        };

        // ── Grid pipeline ─────────────────────────────────────────────────
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
            &[VertexAttribute::with_buffer(
                "a_pos",
                VertexFormat::Float2,
                0,
            )],
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
            text_pipeline,
            text_bindings,
            color_text_pipeline,
            color_text_bindings,
            text_instance_buffer,
            text_atlas,
            emoji_atlas,
            grid_pipeline,
            grid_bindings,
        }
    }

    // ── MVP helpers ────────────────────────────────────────────────────────

    /// Build an orthographic MVP that maps world → clip, applying camera pan/zoom.
    pub fn camera_mvp(camera: &Camera, screen_size: Vec2) -> glam::Mat4 {
        let w = screen_size.x;
        let h = screen_size.y;
        let z = camera.zoom;
        let px = camera.pan.x;
        let py = camera.pan.y;
        // Ortho: maps world rect [pan ± half/zoom] to clip [-1..1]
        let l = px - w * 0.5 / z;
        let r = px + w * 0.5 / z;
        let b = py + h * 0.5 / z; // flip y: world-y up = screen-y down
        let t = py - h * 0.5 / z;
        glam::Mat4::orthographic_rh_gl(l, r, b, t, -1.0, 1.0)
    }

    /// Screen-space identity MVP: maps pixel coords → clip.
    pub fn screen_mvp(screen_size: Vec2) -> glam::Mat4 {
        glam::Mat4::orthographic_rh_gl(0.0, screen_size.x, screen_size.y, 0.0, -1.0, 1.0)
    }

    // ── Draw calls ─────────────────────────────────────────────────────────

    pub fn draw_background_grid(
        &mut self,
        ctx: &mut dyn RenderingBackend,
        camera: &Camera,
        screen_size: Vec2,
    ) {
        // Choose a nice grid cell size (world units), snapping to powers of 2.
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
    ) {
        if instances.is_empty() {
            return;
        }

        ctx.buffer_update(self.instance_buffer, BufferSource::slice(instances));

        ctx.apply_pipeline(&self.shape_pipeline);
        ctx.apply_bindings(&self.shape_bindings);
        ctx.apply_uniforms(UniformsSource::table(&ShapeUniforms {
            u_mvp: mvp.to_cols_array_2d(),
        }));
        ctx.draw(0, 6, instances.len() as i32);
    }

    pub fn draw_text_instances(
        &mut self,
        ctx: &mut dyn RenderingBackend,
        instances: &[TextInstanceData],
        mvp: glam::Mat4,
    ) {
        if instances.is_empty() {
            return;
        }

        ctx.buffer_update(self.text_instance_buffer, BufferSource::slice(instances));
        ctx.apply_pipeline(&self.text_pipeline);
        ctx.apply_bindings(&self.text_bindings);
        ctx.apply_uniforms(UniformsSource::table(&ShapeUniforms {
            u_mvp: mvp.to_cols_array_2d(),
        }));
        ctx.draw(0, 6, instances.len() as i32);
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
        ctx.apply_uniforms(UniformsSource::table(&ShapeUniforms {
            u_mvp: mvp.to_cols_array_2d(),
        }));
        ctx.draw(0, 6, instances.len() as i32);
    }

    pub fn text_atlas(&self) -> TextureId {
        self.text_atlas
    }

    pub fn emoji_atlas(&self) -> TextureId {
        self.emoji_atlas
    }
}

// ── Uniform structs ───────────────────────────────────────────────────────────

#[repr(C)]
struct ShapeUniforms {
    u_mvp: [[f32; 4]; 4],
}

#[repr(C)]
struct GridUniforms {
    u_inv_mvp: [[f32; 4]; 4],
    u_cell: f32,
}
