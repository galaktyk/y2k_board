use glam::Vec2;
use crate::rendering::renderer::{InstanceData, TextInstanceData, MAX_SHAPE_INSTANCES, MAX_TEXT_INSTANCES};
use crate::text::UiTextSpec;

const STATS_TEXT_SIZE: f32 = 12.0;
const STATS_LINE_HEIGHT: f32 = 14.0;
const STATS_PADDING: f32 = 8.0;

pub struct StatsPanelLayout {
    pub background_origin: Vec2,
    pub background_size: Vec2,
    pub text_origin: Vec2,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Build screen-space text specs for the stats overlay.
pub fn build_stats_text_specs(
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
) -> Vec<UiTextSpec> {
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

    let mut text_specs = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        text_specs.push(
            UiTextSpec::top_left(line, Vec2::new(0.0, i as f32 * STATS_LINE_HEIGHT), STATS_TEXT_SIZE, text_color)
                .with_line_height(STATS_LINE_HEIGHT),
        );
    }

    text_specs
}

pub fn build_stats_layout(text_size: Vec2, screen: Vec2) -> StatsPanelLayout {
    let background_size = Vec2::new(
        text_size.x + STATS_PADDING * 2.0,
        text_size.y + STATS_PADDING * 2.0,
    );
    let background_origin = Vec2::new(
        screen.x - background_size.x - 6.0,
        screen.y - background_size.y - 6.0,
    );

    StatsPanelLayout {
        background_origin,
        background_size,
        text_origin: background_origin + Vec2::splat(STATS_PADDING),
    }
}

pub fn build_stats_background_instances(layout: &StatsPanelLayout) -> Vec<InstanceData> {
    vec![InstanceData::new(
        layout.background_origin.to_array(),
        layout.background_size.to_array(),
        0.0,
        [0.04, 0.05, 0.07, 0.80],
        0.0,
        1.0,
    )]
}
