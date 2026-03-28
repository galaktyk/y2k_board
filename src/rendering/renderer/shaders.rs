pub const VERTEX_SRC: &str = r#"#version 100
attribute vec2 a_pos;
attribute vec2 i_pos;
attribute vec2 i_size;
attribute vec4 i_color;
attribute float i_rotation;
attribute vec4 i_pack;

uniform mat4 u_mvp;
uniform float u_world_per_px;
uniform vec2 u_move_offset;
uniform vec2 u_rotate_center;
uniform float u_rotate_angle;

varying vec2 v_uv;
varying vec4 v_color;
varying float v_shape;
varying float v_alpha;
varying vec2 v_line_p;
varying float v_line_len;
varying vec2 v_line_arrows;
varying vec2 v_size;
varying float v_stroke_width;

void main() {
    // Instance data is packed on the Rust side to keep the vertex format compact.
    float i_alpha = i_pack.x / 255.0;
    float i_shape = i_pack.y;
    float i_stroke_width = i_pack.z;
    float i_flags = i_pack.w;
    float i_selected = mod(i_flags, 2.0);
    float i_arrow_start = mod(floor(i_flags / 2.0), 2.0);
    float i_arrow_end = mod(floor(i_flags / 4.0), 2.0);
    vec4 actual_color = i_color / 255.0;

    vec2 world_pos;
    // Lines and line outlines are rendered from a padded quad built around the
    // segment direction so the fragment shader can measure distance to the line.
    if ((i_shape > 1.5 && i_shape < 2.5) || (i_shape > 5.5 && i_shape < 6.5)) {
        vec2 dir = i_size;
        float len = length(dir);
        if (len < 0.0001) { len = 0.0001; }
        vec2 u = dir / len;
        vec2 v = vec2(-u.y, u.x);

        // Shape 6 (selection overlay) needs only a small screen-space margin.
        // Shape 2 (line with arrows) needs enough room for the world-space arrow head.
        float margin = (i_shape > 5.5 && i_shape < 6.5)
            ? u_world_per_px * 3.0
            : i_stroke_width * 4.0 + u_world_per_px * 2.0;

        vec2 p = vec2(
            mix(-margin, len + margin, a_pos.x),
            mix(-margin, margin, a_pos.y)
        );
        world_pos = i_pos + p.x * u + p.y * v;

        v_line_p = p;
        v_line_len = len;
        v_line_arrows = vec2(i_arrow_start, i_arrow_end);
        v_uv = a_pos;
    } else {
        vec2 draw_pos = i_pos;
        vec2 draw_size = i_size;
        if (i_shape > 0.5 && i_shape < 4.5) {
            float margin = max(u_world_per_px * 2.0, 0.0);
            draw_pos -= vec2(margin);
            draw_size += vec2(margin * 2.0);
        }
        world_pos = draw_pos + a_pos * draw_size;
        v_uv = a_pos;
        v_size = draw_size;
        v_line_arrows = vec2(0.0);
    }

    vec2 center = i_pos + i_size * 0.5;
    float c = cos(i_rotation);
    float s = sin(i_rotation);
    mat2 rot = mat2(c, s, -s, c);
    world_pos = center + rot * (world_pos - center);

    if (i_selected > 0.5) {
        float sel_c = cos(u_rotate_angle);
        float sel_s = sin(u_rotate_angle);
        mat2 sel_rot = mat2(sel_c, sel_s, -sel_s, sel_c);
        world_pos += u_move_offset;
        world_pos = u_rotate_center + sel_rot * (world_pos - u_rotate_center);
    }

    gl_Position = u_mvp * vec4(world_pos, 0.0, 1.0);
    v_color = actual_color;
    v_shape = i_shape;
    v_alpha = i_alpha;
    if (!((i_shape > 0.5 && i_shape < 4.5) || ((i_shape > 1.5 && i_shape < 2.5) || (i_shape > 5.5 && i_shape < 6.5)))) {
        v_size = i_size;
    }
    v_stroke_width = i_stroke_width;
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
varying vec2 v_line_arrows;
varying vec2 v_size;
varying float v_stroke_width;

// Rectangle and ellipse outlines use edge distance inside an expanded quad.
float outline_alpha(float edge, float width, float aa) {
    return smoothstep(0.0, aa, edge)
        * (1.0 - smoothstep(width - aa, width + aa, edge));
}

// Approximate signed distance for an axis-aligned ellipse in local space.
float ellipse_signed_distance(vec2 p, vec2 radius) {
    vec2 safe_radius = max(radius, vec2(0.0001));
    vec2 inv_radius2 = 1.0 / (safe_radius * safe_radius);
    float f = dot(p * p, inv_radius2) - 1.0;
    vec2 grad = 2.0 * p * inv_radius2;
    return f / max(length(grad), 0.0001);
}

// Distance from a local point to the finite line segment laid out in v_line_p.
float line_segment_distance(vec2 p, float len) {
    float dx = p.x - clamp(p.x, 0.0, len);
    return length(vec2(dx, p.y));
}

float line_segment_alpha(vec2 p, float start, float end, float thickness, float aa) {
    float len = max(end - start, 0.0);
    if (len <= 0.0001) {
        return 0.0;
    }

    float d = line_segment_distance(vec2(p.x - start, p.y), len);
    return 1.0 - smoothstep(thickness - aa, thickness + aa, d);
}

// Simple triangular arrowhead (not rounded).
// Tip at (0,0), base at (arrow_length, +-arrow_half_width).
float arrow_triangle_alpha(
    float axial,
    float lateral,
    float arrow_length,
    float arrow_half_width,
    float aa
) {
    if (arrow_length <= 0.0001 || arrow_half_width <= 0.0001) {
        return 0.0;
    }

    float L = arrow_length;
    float W = arrow_half_width;
    float hyp = length(vec2(L, W));

    // Signed distances from the three edges (positive = outside).
    float d_top  = (-W * axial + L * abs(lateral)) / hyp;
    float d_base = axial - L;
    float sdf = max(d_top, d_base);

    return 1.0 - smoothstep(-aa, aa, sdf);
}

float fixed_stroke_width() {
    return max(u_world_per_px * 1.00, 0.0001);
}

float fixed_stroke_aa() {
    return max(u_world_per_px * 1.25, 0.0001);
}

float fixed_centered_stroke_half_width() {
    return max(u_world_per_px * 0.50, 0.0001);
}

// Used for centered strokes such as the selected line overlay.
float centered_stroke_alpha(float distance, float half_width, float aa) {
    return 1.0 - smoothstep(half_width - aa, half_width + aa, distance);
}

float hard_edge_alpha(float signed_distance) {
    return 1.0 - step(0.0, signed_distance);
}

float sticky_note_shadow_alpha(vec2 uv, vec2 size, float softness) {
    vec2 dist = min(uv, 1.0 - uv) * size;
    float edge = min(dist.x, dist.y);
    return smoothstep(0.0, softness, edge);
}

void main() {
    float alpha = v_color.a * v_alpha;
    vec2 uv = v_uv;

    // Shape ids are assigned in Rust when building instance data.
    if (v_shape < 0.5) {
        gl_FragColor = vec4(v_color.rgb, alpha);
    } else if (v_shape < 1.5) {
        vec2 p = (uv - 0.5) * v_size;
        vec2 outer_r = abs(v_size) * 0.5;
        float aa = max(u_world_per_px * 0.75, 0.0001);
        float sd = ellipse_signed_distance(p, outer_r);
        float a = smoothstep(0.0, aa, -sd);
        gl_FragColor = vec4(v_color.rgb, alpha * a);
    } else if (v_shape < 2.5) {
        vec2 p = v_line_p;
        float aa = max(u_world_per_px * 0.75, 0.0001);
        // Line body thickness is screen-space (constant pixel width).
        float thickness = max(v_stroke_width * u_world_per_px, u_world_per_px * 1.0);
        // Arrow head is pure world-space so it scales with zoom like normal elements.
        // We add a minimum screen-space size so it's visible when zoomed out.
        float arrow_half_width = max(v_stroke_width * 3.125, u_world_per_px * 5.0);
        float arrow_length = max(v_stroke_width * 6.25, u_world_per_px * 10.0);

        float max_arrow_length = v_line_len * 0.45;
        if (arrow_length > max_arrow_length) {
            float scale = max_arrow_length / arrow_length;
            arrow_length = max_arrow_length;
            arrow_half_width *= scale;
        }

        float body_start = (v_line_arrows.x > 0.5) ? arrow_length : 0.0;
        float body_end = (v_line_arrows.y > 0.5) ? v_line_len - arrow_length : v_line_len;
        float a = 0.0;
        if (body_end > body_start) {
            a = line_segment_alpha(p, body_start, body_end, thickness, aa);
        }

        if (v_line_arrows.x > 0.5) {
            a = max(a, arrow_triangle_alpha(p.x, p.y, arrow_length, arrow_half_width, aa));
        }
        if (v_line_arrows.y > 0.5) {
            a = max(a, arrow_triangle_alpha(v_line_len - p.x, p.y, arrow_length, arrow_half_width, aa));
        }

        gl_FragColor = vec4(v_color.rgb, alpha * a);
    } else if (v_shape < 3.5) {
        vec2 dist = min(uv, 1.0 - uv) * v_size;
        float edge = min(dist.x, dist.y);
        float width = max(v_stroke_width * u_world_per_px, u_world_per_px * 1.0);
        float aa = max(u_world_per_px * 0.75, 0.0001);
        float a = outline_alpha(edge, width, aa);
        gl_FragColor = vec4(v_color.rgb, alpha * a);
    } else if (v_shape < 4.5) {
        vec2 p = (uv - 0.5) * v_size;
        vec2 outer_r = abs(v_size) * 0.5;
        float width = max(v_stroke_width * u_world_per_px, u_world_per_px * 1.0);
        float aa = max(u_world_per_px * 0.75, 0.0001);
        vec2 inner_r = max(outer_r - vec2(width), vec2(0.0001));
        float outer_sd = ellipse_signed_distance(p, outer_r);
        float inner_sd = ellipse_signed_distance(p, inner_r);
        float outer_alpha = smoothstep(0.0, aa, -outer_sd);
        float inner_alpha = smoothstep(0.0, aa, inner_sd);
        float a = clamp(outer_alpha * inner_alpha, 0.0, 1.0);
        gl_FragColor = vec4(v_color.rgb, alpha * a);
    } else if (v_shape < 5.5) {
        vec2 dist = min(uv, 1.0 - uv) * v_size;
        float edge = min(dist.x, dist.y);
        float border = fixed_stroke_width();
        float aa = fixed_stroke_aa();
        float a = outline_alpha(edge, border, aa);
        gl_FragColor = vec4(v_color.rgb, alpha * a);
    } else if (v_shape < 6.5) {
        vec2 p = v_line_p;
        float d = line_segment_distance(p, v_line_len);
        // Keep the overlay centered on the line with a total visible width of 1 px.
        float half_width = fixed_centered_stroke_half_width();
        float aa = max(u_world_per_px * 0.5, 0.0001);
        float a = centered_stroke_alpha(d, half_width, aa);
        gl_FragColor = vec4(v_color.rgb, alpha * a);
    } else if (v_shape < 7.5) {
        float softness = max(min(v_size.x, v_size.y) * 0.16, u_world_per_px * 10.0);
        float a = sticky_note_shadow_alpha(uv, v_size, softness);
        gl_FragColor = vec4(v_color.rgb, alpha * a);
    }
    else {
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
attribute vec4 i_pack;

uniform mat4 u_mvp;
uniform vec2 u_move_offset;
uniform vec2 u_rotate_center;
uniform float u_rotate_angle;

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

    if (i_pack.x > 0.0) {
        float sel_c = cos(u_rotate_angle);
        float sel_s = sin(u_rotate_angle);
        mat2 sel_rot = mat2(sel_c, sel_s, -sel_s, sel_c);
        world_pos += u_move_offset;
        world_pos = u_rotate_center + sel_rot * (world_pos - u_rotate_center);  
    }

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