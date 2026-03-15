use crate::camera::Camera;
use glam::Vec2;
use miniquad::*;

pub const MAX_SHAPE_INSTANCES: usize = 100_000;
pub const MAX_TEXT_INSTANCES: usize = 200_000;
pub const MAX_IMAGE_INSTANCES: usize = 8_192;

// ── Instance data ─────────────────────────────────────────────────────────────

/// One instance in the GPU buffer.
/// Layout must match the vertex shader attributes.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct InstanceData {
    pub pos:        [f32; 2],  // 8
    pub size:       [f32; 2],  // 8
    pub color:      [u8; 4],   // 4
    pub rotation:   f32,       // 4
    pub alpha:      u8,        // 1
    pub shape_type: u8,        // 1
    pub _pad:       [u8; 2],   // 2
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TextInstanceData {
    pub pos:      [f32; 2],  // 8
    pub size:     [f32; 2],  // 8
    pub uv_min:   [u16; 2],  // 4
    pub uv_max:   [u16; 2],  // 4
    pub origin:   [i16; 2],  // 4
    pub color:    [u8; 4],   // 4
    pub rotation: f32,       // 4
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ImageInstanceData {
    pub pos:      [f32; 2],
    pub size:     [f32; 2],
    pub uv_min:   [u16; 2],
    pub uv_max:   [u16; 2],
    pub origin:   [i16; 2],
    pub color:    [u8; 4],
    pub rotation: f32,
}

#[derive(Clone, Copy)]
pub struct PreparedImageDraw {
    pub texture: TextureId,
    pub instance: ImageInstanceData,
}

impl InstanceData {
    pub fn new(pos: [f32; 2], size: [f32; 2], rotation: f32, color_f32: [f32; 4], shape_type: f32, alpha_f32: f32) -> Self {
        Self {
            pos,
            size,
            color: [
                (color_f32[0] * 255.0) as u8,
                (color_f32[1] * 255.0) as u8,
                (color_f32[2] * 255.0) as u8,
                (color_f32[3] * 255.0) as u8,
            ],
            rotation,
            alpha: (alpha_f32 * 255.0) as u8,
            shape_type: shape_type as u8,
            _pad: [0, 0],
        }
    }
}

impl TextInstanceData {
    pub fn new(pos: [f32; 2], size: [f32; 2], origin: [f32; 2], rotation: f32, uv_min: [f32; 2], uv_max: [f32; 2], color_f32: [f32; 4]) -> Self {
        Self {
            pos,
            size,
            uv_min: [(uv_min[0] * 65535.0) as u16, (uv_min[1] * 65535.0) as u16],
            uv_max: [(uv_max[0] * 65535.0) as u16, (uv_max[1] * 65535.0) as u16],
            origin: [origin[0] as i16, origin[1] as i16],
            color: [
                (color_f32[0] * 255.0) as u8,
                (color_f32[1] * 255.0) as u8,
                (color_f32[2] * 255.0) as u8,
                (color_f32[3] * 255.0) as u8,
            ],
            rotation,
        }
    }
}

impl ImageInstanceData {
    pub fn new(pos: [f32; 2], size: [f32; 2], origin: [f32; 2], rotation: f32, uv_min: [f32; 2], uv_max: [f32; 2], color_f32: [f32; 4]) -> Self {
        Self {
            pos,
            size,
            uv_min: [(uv_min[0] * 65535.0) as u16, (uv_min[1] * 65535.0) as u16],
            uv_max: [(uv_max[0] * 65535.0) as u16, (uv_max[1] * 65535.0) as u16],
            origin: [origin[0] as i16, origin[1] as i16],
            color: [
                (color_f32[0] * 255.0) as u8,
                (color_f32[1] * 255.0) as u8,
                (color_f32[2] * 255.0) as u8,
                (color_f32[3] * 255.0) as u8,
            ],
            rotation,
        }
    }
}

// ── Shaders ───────────────────────────────────────────────────────────────────

const VERTEX_SRC: &str = r#"#version 100
// per-vertex (unit quad)
attribute vec2 a_pos;      // 0..1 range
// per-instance
attribute vec2 i_pos;
attribute vec2 i_size;
attribute vec4 i_color;
attribute float i_rotation;
attribute vec4 i_pack;

uniform mat4 u_mvp;
uniform float u_world_per_px;

varying vec2 v_uv;
varying vec4 v_color;
varying float v_shape;
varying float v_alpha;
varying vec2 v_line_p;
varying float v_line_len;
varying vec2 v_size;

void main() {
    float i_alpha = i_pack.x / 255.0;
    float i_shape = i_pack.y;
    vec4 actual_color = i_color / 255.0;

    vec2 world_pos;
    if ((i_shape > 1.5 && i_shape < 2.5) || (i_shape > 6.5 && i_shape < 7.5)) {
        // Line
        vec2 dir = i_size;
        float len = length(dir);
        if (len < 0.0001) { len = 0.0001; }
        vec2 u = dir / len;
        vec2 v = vec2(-u.y, u.x);
        
        float margin = (i_shape > 6.5 && i_shape < 7.5)
            ? max(u_world_per_px * 3.0, 0.0001)
            : 8.0; // half-thickness + antialiasing
        
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
    v_color = actual_color;
    v_shape = i_shape;
    v_alpha = i_alpha;
    v_size  = i_size;
}
"#;

const FRAGMENT_SRC: &str = r#"#version 100
precision highp float;

uniform float u_world_per_px;

varying vec2 v_uv;
varying vec4 v_color;
varying float v_shape;
varying float v_alpha;
varying vec2 v_line_p;
varying float v_line_len;
varying vec2 v_size;

float outline_alpha(float edge, float width, float aa) {
    return smoothstep(0.0, aa, edge)
        * (1.0 - smoothstep(width - aa, width + aa, edge));
}

float ellipse_outline_alpha(float d, float radius, float width, float aa) {
    float inv_radius = 1.0 / max(radius, 0.0001);
    float width_n = width * inv_radius;
    float aa_n = aa * inv_radius;
    float outer = 1.0 - smoothstep(1.0, 1.0 + aa_n, d);
    float inner = smoothstep(1.0 - width_n - aa_n, 1.0 - width_n + aa_n, d);
    return outer * inner;
}

float line_segment_distance(vec2 p, float len) {
    float dx = p.x - clamp(p.x, 0.0, len);
    return length(vec2(dx, p.y));
}

float fixed_stroke_width() {
    return max(u_world_per_px * 1.25, 0.0001);
}

float fixed_stroke_aa() {
    return max(u_world_per_px * 1.25, 0.0001);
}

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
        float d = line_segment_distance(p, v_line_len);
        float thickness = 4.0; // visual half-thickness
        float a = 1.0 - smoothstep(thickness - 1.0, thickness + 1.0, d);
        gl_FragColor = vec4(v_color.rgb, alpha * a);

    } else if (v_shape < 3.5) {
        // Rect border outline
        vec2 dist = min(uv, 1.0 - uv) * v_size;
        float edge = min(dist.x, dist.y);
        float aa = max(u_world_per_px * 1.25, 0.0001);
        float border = max(2.5, aa);
        float a = outline_alpha(edge, border, aa);
        gl_FragColor = vec4(v_color.rgb, alpha * a);

    } else if (v_shape < 4.5) {
        // Ellipse border outline
        vec2 c = uv * 2.0 - 1.0;
        float d = length(c);
        float r = min(v_size.x, v_size.y) * 0.5;
        float aa = max(u_world_per_px * 1.25, 0.0001);
        float border = max(2.5, aa);
        float a = ellipse_outline_alpha(d, r, border, aa);
        gl_FragColor = vec4(v_color.rgb, alpha * a);

    } else if (v_shape < 5.5) {
        // Rect border outline with a fixed 1px screen-space stroke.
        vec2 dist = min(uv, 1.0 - uv) * v_size;
        float edge = min(dist.x, dist.y);
        float border = fixed_stroke_width();
        float aa = fixed_stroke_aa();
        float a = outline_alpha(edge, border, aa);
        gl_FragColor = vec4(v_color.rgb, alpha * a);

    } else if (v_shape < 6.5) {
        // Ellipse border outline with a fixed 1px screen-space stroke.
        vec2 c = uv * 2.0 - 1.0;
        float d = length(c);
        float r = min(v_size.x, v_size.y) * 0.5;
        float border = fixed_stroke_width();
        float aa = fixed_stroke_aa();
        float a = ellipse_outline_alpha(d, r, border, aa);
        gl_FragColor = vec4(v_color.rgb, alpha * a);

    } else if (v_shape < 7.5) {
        // Line highlight with the same fixed screen-space stroke/AA treatment
        // as the rect and ellipse border highlights.
        vec2 p = v_line_p;
        float d = line_segment_distance(p, v_line_len);
        float half_width = fixed_stroke_width();
        float aa = fixed_stroke_aa();
        float a = 1.0 - smoothstep(half_width, half_width + aa, d);
        gl_FragColor = vec4(v_color.rgb, alpha * a);

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
    vec4 actual_color = i_color / 255.0;
    vec2 actual_uv_min = i_uv_min / 65535.0;
    vec2 actual_uv_max = i_uv_max / 65535.0;

    vec2 actual_origin = i_origin;
    if (actual_origin.x > 32767.0) { actual_origin.x -= 65536.0; }
    if (actual_origin.y > 32767.0) { actual_origin.y -= 65536.0; }

    vec2 world_pos = i_pos + a_pos * i_size;
    float c = cos(i_rotation);
    float s = sin(i_rotation);
    mat2 rot = mat2(c, s, -s, c);
    world_pos = actual_origin + rot * (world_pos - actual_origin);

    v_uv = mix(actual_uv_min, actual_uv_max, a_pos);
    v_color = actual_color;
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

const IMAGE_FRAGMENT_SRC: &str = r#"#version 100
precision highp float;

varying vec2 v_uv;
varying vec4 v_color;

uniform sampler2D u_image_texture;

void main() {
    vec4 sample_color = texture2D(u_image_texture, v_uv);
    if (sample_color.a <= 0.0) {
        discard;
    }
    gl_FragColor = sample_color * v_color;
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
    float a = 1.0 - smoothstep(0.0, 0.03, line);
    gl_FragColor = vec4(0.76, 0.79, 0.83, a * 0.3);
}
"#;

// ── Renderer ─────────────────────────────────────────────────────────────────

pub struct Renderer {
    // dynamic shape pipeline
    shape_pipeline: Pipeline,
    shape_bindings: Bindings,
    instance_buffer: BufferId,

    // persistent full-scene shape draw
    scene_shape_bindings: Bindings,
    scene_instance_buffer: BufferId,
    scene_shape_count: usize,

    // dynamic text pipelines
    text_pipeline: Pipeline,
    text_bindings: Bindings,
    color_text_pipeline: Pipeline,
    color_text_bindings: Bindings,
    text_instance_buffer: BufferId,

    // dynamic image pipeline
    image_pipeline: Pipeline,
    image_bindings: Bindings,
    image_instance_buffer: BufferId,

    // persistent full-scene text draw
    scene_text_bindings: Bindings,
    scene_color_text_bindings: Bindings,
    scene_mono_text_buffer: BufferId,
    scene_color_text_buffer: BufferId,
    scene_mono_text_count: usize,
    scene_color_text_count: usize,

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
            ((MAX_SHAPE_INSTANCES * std::mem::size_of::<InstanceData>()) + (MAX_TEXT_INSTANCES * std::mem::size_of::<TextInstanceData>())) / (1024 * 1024),
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
                        uniforms: vec![
                            UniformDesc::new("u_mvp", UniformType::Mat4),
                            UniformDesc::new("u_world_per_px", UniformType::Float1),
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
                // buffer 0: a_pos
                VertexAttribute::with_buffer("a_pos", VertexFormat::Float2, 0),
                // buffer 1: per-instance
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
                VertexAttribute::with_buffer("i_uv_min", VertexFormat::Short2, 1),
                VertexAttribute::with_buffer("i_uv_max", VertexFormat::Short2, 1),
                VertexAttribute::with_buffer("i_origin", VertexFormat::Short2, 1),
                VertexAttribute::with_buffer("i_color", VertexFormat::Byte4, 1),
                VertexAttribute::with_buffer("i_rotation", VertexFormat::Float1, 1),
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
                VertexAttribute::with_buffer("i_uv_min", VertexFormat::Short2, 1),
                VertexAttribute::with_buffer("i_uv_max", VertexFormat::Short2, 1),
                VertexAttribute::with_buffer("i_origin", VertexFormat::Short2, 1),
                VertexAttribute::with_buffer("i_color", VertexFormat::Byte4, 1),
                VertexAttribute::with_buffer("i_rotation", VertexFormat::Float1, 1),
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
                        uniforms: vec![UniformDesc::new("u_mvp", UniformType::Mat4)],
                    },
                    images: vec!["u_image_texture".to_string()],
                },
            )
            .expect("image shader compile failed");

        let image_pipeline = ctx.new_pipeline(
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
            ],
            image_shader,
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

        let image_bindings = Bindings {
            vertex_buffers: vec![vertex_buf, image_instance_buffer],
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
        }));
        ctx.draw(0, 6, instances.len() as i32);
    }

    pub fn upload_scene_instances(
        &mut self,
        ctx: &mut dyn RenderingBackend,
        instances: &[InstanceData],
    ) {
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
    ) {
        if self.scene_shape_count == 0 {
            return;
        }

        let world_per_px = Self::world_per_px(mvp, screen_size);

        ctx.apply_pipeline(&self.shape_pipeline);
        ctx.apply_bindings(&self.scene_shape_bindings);
        ctx.apply_uniforms(UniformsSource::table(&ShapeUniforms {
            u_mvp: mvp.to_cols_array_2d(),
            u_world_per_px: world_per_px,
        }));
        ctx.draw(0, 6, self.scene_shape_count as i32);
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
        ctx.apply_uniforms(UniformsSource::table(&TextUniforms {
            u_mvp: mvp.to_cols_array_2d(),
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
    ) {
        if self.scene_mono_text_count == 0 {
            return;
        }

        ctx.apply_pipeline(&self.text_pipeline);
        ctx.apply_bindings(&self.scene_text_bindings);
        ctx.apply_uniforms(UniformsSource::table(&TextUniforms {
            u_mvp: mvp.to_cols_array_2d(),
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
        }));
        ctx.draw(0, 6, instances.len() as i32);
    }

    pub fn draw_scene_color_text_instances(
        &mut self,
        ctx: &mut dyn RenderingBackend,
        mvp: glam::Mat4,
    ) {
        if self.scene_color_text_count == 0 {
            return;
        }

        ctx.apply_pipeline(&self.color_text_pipeline);
        ctx.apply_bindings(&self.scene_color_text_bindings);
        ctx.apply_uniforms(UniformsSource::table(&TextUniforms {
            u_mvp: mvp.to_cols_array_2d(),
        }));
        ctx.draw(0, 6, self.scene_color_text_count as i32);
    }

    pub fn draw_image_draws(
        &mut self,
        ctx: &mut dyn RenderingBackend,
        draws: &[PreparedImageDraw],
        mvp: glam::Mat4,
    ) {
        if draws.is_empty() {
            return;
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
            }));
            ctx.draw(0, 6, batch.len() as i32);

            start = end;
        }
    }

    fn world_per_px(mvp: glam::Mat4, screen_size: Vec2) -> f32 {
        let pixels_per_world_x = (mvp.x_axis.x * screen_size.x * 0.5).abs();
        let pixels_per_world_y = (mvp.y_axis.y * screen_size.y * 0.5).abs();
        let pixels_per_world = pixels_per_world_x.min(pixels_per_world_y).max(0.0001);
        1.0 / pixels_per_world
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
    u_world_per_px: f32,
}

#[repr(C)]
struct TextUniforms {
    u_mvp: [[f32; 4]; 4],
}

#[repr(C)]
struct GridUniforms {
    u_inv_mvp: [[f32; 4]; 4],
    u_cell: f32,
}
