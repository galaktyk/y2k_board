use glam::Vec2;
use crate::renderer::InstanceData;

// ── 3×5 bitmap font ───────────────────────────────────────────────────────────
//
// Each entry is 5 row bytes. Each row is 3 bits wide:
//   bit 2 (0b100) = left column
//   bit 1 (0b010) = middle column
//   bit 0 (0b001) = right column
//
fn char_rows(c: char) -> [u8; 5] {
    match c {
        '0' | 'O' | 'o' => [7, 5, 5, 5, 7],
        '1'              => [2, 6, 2, 2, 7],
        '2'              => [7, 1, 7, 4, 7],
        '3'              => [7, 1, 7, 1, 7],
        '4'              => [5, 5, 7, 1, 1],
        '5' | 'S' | 's' => [7, 4, 7, 1, 7],
        '6'              => [7, 4, 7, 5, 7],
        '7'              => [7, 1, 1, 1, 1],
        '8'              => [7, 5, 7, 5, 7],
        '9'              => [7, 5, 7, 1, 7],
        'A' | 'a'        => [7, 5, 7, 5, 5],
        'B' | 'b'        => [6, 5, 6, 5, 6],
        'C' | 'c'        => [7, 4, 4, 4, 7],
        'E' | 'e'        => [7, 4, 7, 4, 7],
        'F' | 'f'        => [7, 4, 7, 4, 4],
        'G' | 'g'        => [7, 4, 5, 5, 7],
        'H' | 'h'        => [5, 5, 7, 5, 5],
        'J' | 'j'        => [3, 1, 1, 5, 7],
        'D' | 'd'        => [6, 5, 5, 5, 6],
        'I' | 'i'        => [7, 2, 2, 2, 7],
        'L' | 'l'        => [4, 4, 4, 4, 7],
        'M'              => [5, 7, 5, 5, 5],
        'm'              => [0, 6, 5, 5, 5],
        'N'              => [5, 7, 7, 5, 5],
        'n'              => [0, 6, 5, 5, 5],
        'P' | 'p'        => [7, 5, 7, 4, 4],
        'R' | 'r'        => [6, 5, 6, 5, 5],
        'T' | 't'        => [7, 2, 2, 2, 2],
        'V' | 'v'        => [5, 5, 5, 2, 2],
        'X' | 'x'        => [5, 5, 2, 5, 5],
        'Z' | 'z'        => [7, 1, 2, 4, 7],
        'U' | 'u'        => [5, 5, 5, 5, 7],
        ':'              => [0, 2, 0, 2, 0],
        '.'              => [0, 0, 0, 0, 2],
        ' '              => [0, 0, 0, 0, 0],
        _                => [5, 0, 2, 0, 5], // unknown → "?"
    }
}

fn emit_char(
    c: char,
    ox: f32,
    oy: f32,
    scale: f32,
    color: [f32; 4],
    out: &mut Vec<InstanceData>,
) {
    let rows = char_rows(c);
    for (row, &bits) in rows.iter().enumerate() {
        for col in 0u8..3 {
            if bits & (4 >> col) != 0 {
                out.push(InstanceData {
                    pos:        [ox + col as f32 * scale, oy + row as f32 * scale],
                    size:       [scale, scale],
                    rotation:   0.0,
                    color,
                    shape_type: 0.0,
                    alpha:      1.0,
                });
            }
        }
    }
}

fn emit_string(
    s: &str,
    x: f32,
    y: f32,
    scale: f32,
    color: [f32; 4],
    out: &mut Vec<InstanceData>,
) {
    let stride = 3.0 * scale + scale; // 3-wide glyph + 1-px gap
    for (i, c) in s.chars().enumerate() {
        emit_char(c, x + i as f32 * stride, y, scale, color, out);
    }
}

/// Public: emit a text string at screen-space position (x, y) with the given
/// pixel scale and color into `out`.
pub fn emit_text(
    s: &str,
    x: f32,
    y: f32,
    scale: f32,
    color: [f32; 4],
    out: &mut Vec<InstanceData>,
) {
    emit_string(s, x, y, scale, color, out);
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Build screen-space InstanceData for the stats overlay (bottom-right corner).
pub fn build_stats_instances(
    zoom:      f32,
    obj_count: usize,
    fps:       f32,
    frame_ms:  f32,
    screen:    Vec2,
) -> Vec<InstanceData> {
    const SCALE: f32 = 2.0;
    let char_h  = 5.0 * SCALE;
    let stride  = 3.0 * SCALE + SCALE; // char width + gap
    let line_h  = char_h + SCALE * 2.0; // vertical spacing
    let pad     = 8.0f32;

    let text_color = [0.88f32, 0.92, 0.96, 1.0];
    let frame_label = if frame_ms >= 10.0 {
        format!("FT   {:.1}MS", frame_ms)
    } else if frame_ms >= 1.0 {
        format!("FT   {:.2}MS", frame_ms)
    } else {
        format!("FT   {:.3}MS", frame_ms)
    };

    // Lines listed top → bottom inside the panel
    let lines: [String; 4] = [
        format!("ZOOM {:.3}X",  zoom),
        format!("OBJ  {}",      obj_count),
        format!("FPS  {:.0}",   fps),
        frame_label,
    ];

    let max_chars = lines.iter().map(|l| l.chars().count()).max().unwrap_or(1) as f32;
    let text_w    = max_chars * stride - SCALE; // trim trailing gap
    let text_h    = lines.len() as f32 * line_h - SCALE * 2.0;

    let bg_w = text_w + pad * 2.0;
    let bg_h = text_h + pad * 2.0;
    let bg_x = screen.x - bg_w - 6.0;
    let bg_y = screen.y - bg_h - 6.0;

    let mut out = Vec::new();

    // Semi-transparent background panel
    out.push(InstanceData {
        pos:        [bg_x, bg_y],
        size:       [bg_w, bg_h],
        rotation:   0.0,
        color:      [0.04, 0.05, 0.07, 0.80],
        shape_type: 0.0,
        alpha:      1.0,
    });

    for (i, line) in lines.iter().enumerate() {
        let tx = bg_x + pad;
        let ty = bg_y + pad + i as f32 * line_h;
        emit_string(line, tx, ty, SCALE, text_color, &mut out);
    }

    out
}
