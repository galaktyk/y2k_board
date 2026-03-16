use crate::renderer::InstanceData;

// 3x5 bitmap font. Each entry is 5 row bytes and each row is 3 bits wide.
fn char_rows(c: char) -> [u8; 5] {
    match c {
        '0' | 'O' | 'o' => [7, 5, 5, 5, 7],
        '1' => [2, 6, 2, 2, 7],
        '2' => [7, 1, 7, 4, 7],
        '3' => [7, 1, 7, 1, 7],
        '4' => [5, 5, 7, 1, 1],
        '5' | 'S' | 's' => [7, 4, 7, 1, 7],
        '6' => [7, 4, 7, 5, 7],
        '7' => [7, 1, 1, 1, 1],
        '8' => [7, 5, 7, 5, 7],
        '9' => [7, 5, 7, 1, 7],
        'A' | 'a' => [7, 5, 7, 5, 5],
        'B' | 'b' => [6, 5, 6, 5, 6],
        'C' | 'c' => [7, 4, 4, 4, 7],
        'E' | 'e' => [7, 4, 7, 4, 7],
        'F' | 'f' => [7, 4, 7, 4, 4],
        'G' | 'g' => [7, 4, 5, 5, 7],
        'H' | 'h' => [5, 5, 7, 5, 5],
        'J' | 'j' => [3, 1, 1, 5, 7],
        'D' | 'd' => [6, 5, 5, 5, 6],
        'I' | 'i' => [7, 2, 2, 2, 7],
        'L' | 'l' => [4, 4, 4, 4, 7],
        'M' => [5, 7, 5, 5, 5],
        'm' => [0, 6, 5, 5, 5],
        'N' => [5, 7, 7, 5, 5],
        'n' => [0, 6, 5, 5, 5],
        'P' | 'p' => [7, 5, 7, 4, 4],
        'R' | 'r' => [6, 5, 6, 5, 5],
        'T' | 't' => [7, 2, 2, 2, 2],
        'V' | 'v' => [5, 5, 5, 2, 2],
        'X' | 'x' => [5, 5, 2, 5, 5],
        'Z' | 'z' => [7, 1, 2, 4, 7],
        'U' | 'u' => [5, 5, 5, 5, 7],
        ':' => [0, 2, 0, 2, 0],
        '/' => [1, 1, 2, 4, 4],
        '.' => [0, 0, 0, 0, 2],
        ' ' => [0, 0, 0, 0, 0],
        '▨' => [7, 7, 7, 7, 7],
        '☐' => [7, 5, 5, 5, 7],
        _ => [5, 0, 2, 0, 5],
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
                out.push(InstanceData::new(
                    [ox + col as f32 * scale, oy + row as f32 * scale],
                    [scale, scale],
                    0.0,
                    color,
                    0.0,
                    1.0,
                ));
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
    let stride = 3.0 * scale + scale;
    for (i, c) in s.chars().enumerate() {
        emit_char(c, x + i as f32 * stride, y, scale, color, out);
    }
}

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