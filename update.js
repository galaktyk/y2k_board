const fs = require('fs');
let r = fs.readFileSync('src/renderer.rs', 'utf8');

// 1. SHADER UPDATES
// shape vertex
r = r.replace('attribute vec2 i_pos;\nattribute vec2 i_size;\nattribute float i_rotation;\nattribute vec4 i_color;\nattribute float i_shape;\nattribute float i_alpha;',
ttribute vec2 i_pos;
attribute vec2 i_size;
attribute vec4 i_color;
attribute float i_rotation;
attribute vec4 i_pack;);

r = r.replace(/vec2 world_pos;\n\s*if \(i_shape > 1\.5[\s\S]*?else/m,
    vec4 actual_color = i_color / 255.0;
    float i_alpha = i_pack.x / 255.0;
    float i_shape = i_pack.y;

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
    } else);

r = r.replace('v_color = i_color;', 'v_color = actual_color;');
r = r.replace('v_shape = i_shape;', 'v_shape = i_shape;');
r = r.replace('v_alpha = i_alpha;', 'v_alpha = i_alpha;');

// text vertex
r = r.replace('attribute vec4 i_color;', 'attribute vec4 i_color;\nattribute vec2 i_origin;\n'); // wait, let's just make it exact
fs.writeFileSync('src/renderer.rs', r);
