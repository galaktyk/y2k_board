pub const VERTEX_SRC: &str = r#"#version 100
attribute vec2 a_pos;
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
        vec2 dir = i_size;
        float len = length(dir);
        if (len < 0.0001) { len = 0.0001; }
        vec2 u = dir / len;
        vec2 v = vec2(-u.y, u.x);

        float margin = (i_shape > 6.5 && i_shape < 7.5)
            ? max(u_world_per_px * 3.0, 0.0001)
            : 8.0;

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

    gl_Position = u_mvp * vec4(world_pos, 0.0, 1.0);
    v_color = actual_color;
    v_shape = i_shape;
    v_alpha = i_alpha;
    v_size = i_size;
}
"#;

pub const FRAGMENT_SRC: &str = r#"#version 100
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

float ellipse_signed_distance(vec2 p, vec2 radius) {
    vec2 safe_radius = max(radius, vec2(0.0001));
    vec2 inv_radius2 = 1.0 / (safe_radius * safe_radius);
    float f = dot(p * p, inv_radius2) - 1.0;
    vec2 grad = 2.0 * p * inv_radius2;
    return f / max(length(grad), 0.0001);
}

float ellipse_outline_alpha(vec2 p, vec2 radius, float width, float aa) {
    float sd = ellipse_signed_distance(p, radius);
    float edge = max(-sd, 0.0);
    float outer = 1.0 - smoothstep(0.0, aa, sd);
    float inner = 1.0 - smoothstep(width - aa, width + aa, edge);
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
    vec2 uv = v_uv;

    if (v_shape < 0.5) {
        vec2 d = min(uv, 1.0 - uv);
        float edge = min(d.x, d.y);
        float a = smoothstep(0.0, 0.01, edge);
        gl_FragColor = vec4(v_color.rgb, alpha * a);
    } else if (v_shape < 1.5) {
        vec2 c = uv * 2.0 - 1.0;
        float d = length(c);
        float aa = max(u_world_per_px * 2.0 / min(v_size.x, v_size.y), 0.0001);
        float a = 1.0 - smoothstep(1.0 - aa, 1.0, d);
        gl_FragColor = vec4(v_color.rgb, alpha * a);
    } else if (v_shape < 2.5) {
        vec2 p = v_line_p;
        float d = line_segment_distance(p, v_line_len);
        float thickness = 4.0;
        float a = 1.0 - smoothstep(thickness - 1.0, thickness + 1.0, d);
        gl_FragColor = vec4(v_color.rgb, alpha * a);
    } else if (v_shape < 3.5) {
        vec2 dist = min(uv, 1.0 - uv) * v_size;
        float edge = min(dist.x, dist.y);
        float aa = max(u_world_per_px * 1.25, 0.0001);
        float border = max(2.5, aa);
        float a = outline_alpha(edge, border, aa);
        gl_FragColor = vec4(v_color.rgb, alpha * a);
    } else if (v_shape < 4.5) {
        vec2 p = (uv - 0.5) * v_size;
        vec2 r = abs(v_size) * 0.5;
        float aa = max(u_world_per_px * 1.25, 0.0001);
        float border = max(2.5, aa);
        float a = ellipse_outline_alpha(p, r, border, aa);
        gl_FragColor = vec4(v_color.rgb, alpha * a);
    } else if (v_shape < 5.5) {
        vec2 dist = min(uv, 1.0 - uv) * v_size;
        float edge = min(dist.x, dist.y);
        float border = fixed_stroke_width();
        float aa = fixed_stroke_aa();
        float a = outline_alpha(edge, border, aa);
        gl_FragColor = vec4(v_color.rgb, alpha * a);
    } else if (v_shape < 6.5) {
        vec2 p = (uv - 0.5) * v_size;
        vec2 r = abs(v_size) * 0.5;
        float border = fixed_stroke_width();
        float aa = fixed_stroke_aa();
        float a = ellipse_outline_alpha(p, r, border, aa);
        gl_FragColor = vec4(v_color.rgb, alpha * a);
    } else if (v_shape < 7.5) {
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

pub const TEXT_VERTEX_SRC: &str = r#"#version 100
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

pub const TEXT_FRAGMENT_SRC: &str = r#"#version 100
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

pub const COLOR_TEXT_FRAGMENT_SRC: &str = r#"#version 100
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

pub const IMAGE_FRAGMENT_SRC: &str = r#"#version 100
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

pub const GRID_VERTEX_SRC: &str = r#"#version 100
attribute vec2 a_pos;

uniform mat4 u_inv_mvp;
uniform float u_cell;
varying vec2 v_cell;

void main() {
    gl_Position = vec4(a_pos, 0.0, 1.0);
    vec4 w = u_inv_mvp * vec4(a_pos, 0.0, 1.0);
    v_cell = (w.xy / w.w) / u_cell;
}
"#;

pub const GRID_FRAGMENT_SRC: &str = r#"#version 100
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