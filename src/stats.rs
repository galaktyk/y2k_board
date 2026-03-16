use glam::Vec2;
use crate::rendering::emit_text;
use crate::renderer::{InstanceData, TextInstanceData, MAX_SHAPE_INSTANCES, MAX_TEXT_INSTANCES};

// ── Public API ────────────────────────────────────────────────────────────────

/// Build screen-space InstanceData for the stats overlay (bottom-right corner).
pub fn build_stats_instances(
    zoom:      f32,
    shapes_count: usize,
    char_count: usize,
    atlas_count: usize,
    atlas_total: usize,
    image_ram_used_bytes: usize,
    image_ram_total_bytes: usize,
    image_vram_used_bytes: usize,
    image_vram_total_bytes: usize,
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

    // calculate sizes
    let shape_bytes = MAX_SHAPE_INSTANCES * std::mem::size_of::<InstanceData>();
    let text_bytes = MAX_TEXT_INSTANCES * std::mem::size_of::<TextInstanceData>();
    let total_mb: f64 = (shape_bytes + text_bytes) as f64 / (1024.0 * 1024.0);

    let mb_usage: f64 = ((shapes_count * std::mem::size_of::<InstanceData>()) as f64 + (char_count * std::mem::size_of::<TextInstanceData>()) as f64) / (1024.0 * 1024.0);
    let image_ram_mb = image_ram_used_bytes as f64 / (1024.0 * 1024.0);
    let image_ram_total_mb = image_ram_total_bytes as f64 / (1024.0 * 1024.0);
    let image_vram_mb = image_vram_used_bytes as f64 / (1024.0 * 1024.0);
    let image_vram_total_mb = image_vram_total_bytes as f64 / (1024.0 * 1024.0);

    // Lines listed top → bottom inside the panel
    let lines: Vec<String> = vec![
        format!("ZOOM  {:.3}X", zoom),
        format!("SHAPE {}/{}", shapes_count, MAX_SHAPE_INSTANCES),
        format!("TEXT  {}/{}", char_count, MAX_TEXT_INSTANCES),
        format!("VRAM  {:.1}MB/{:.0}MB", mb_usage, total_mb),
        format!("ATLAS {}/{}", atlas_count, atlas_total),
        format!("IRAM  {:.1}MB/{:.0}MB", image_ram_mb, image_ram_total_mb),
        format!("IVRAM {:.1}MB/{:.0}MB", image_vram_mb, image_vram_total_mb),
        format!("FPS   {:.0}", fps),
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
    out.push(InstanceData::new(
        [bg_x, bg_y],
        [bg_w, bg_h],
        0.0,
        [0.04, 0.05, 0.07, 0.80],
        0.0,
        1.0,
    ));

    for (i, line) in lines.iter().enumerate() {
        let tx = bg_x + pad;
        let ty = bg_y + pad + i as f32 * line_h;
        emit_text(line, tx, ty, SCALE, text_color, &mut out);
    }

    out
}
