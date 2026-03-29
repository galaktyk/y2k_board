use crate::rendering::renderer::{InstanceData, RendererMemoryStats};
use crate::text::UiTextSpec;
use glam::Vec2;

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
    zoom: f32,
    element_count: usize,
    _char_count: usize,
    renderer_memory: RendererMemoryStats,
    atlas_count: usize,
    atlas_total: usize,
    image_ram_used_bytes: usize,
    image_ram_total_bytes: usize,
    image_vram_used_bytes: usize,
    image_vram_total_bytes: usize,
    fps: f32,
    frame_ms: f32,
) -> Vec<UiTextSpec> {
    let text_color = [0.88f32, 0.92, 0.96, 1.0];
    let frame_label = if frame_ms >= 10.0 {
        format!("Frame time   {:.1}MS", frame_ms)
    } else if frame_ms >= 1.0 {
        format!("Frame time   {:.2}MS", frame_ms)
    } else {
        format!("Frame time   {:.3}MS", frame_ms)
    };

    let scene_mb = renderer_memory.active_scene_bytes as f64 / (1024.0 * 1024.0);
    let reserved_mb = renderer_memory.reserved_gpu_bytes as f64 / (1024.0 * 1024.0);
    let atlas_reserved_mb = renderer_memory.reserved_atlas_bytes as f64 / (1024.0 * 1024.0);

    let image_ram_mb = image_ram_used_bytes as f64 / (1024.0 * 1024.0);
    let image_ram_total_mb = image_ram_total_bytes as f64 / (1024.0 * 1024.0);
    let image_vram_mb = image_vram_used_bytes as f64 / (1024.0 * 1024.0);
    let image_vram_total_mb = image_vram_total_bytes as f64 / (1024.0 * 1024.0);

    // Lines listed top → bottom inside the panel
    let lines: Vec<String> = vec![
        format!("Zoom  {:.3}X", zoom),
        format!("Elements     {}", element_count),
        format!("---Render Instances---"),
        format!(
            "Shape Draws  {}/{}",
            renderer_memory.scene_shape_instances, renderer_memory.scene_shape_limit
        ),
        format!(
            "Line Draws   {}/{}",
            renderer_memory.scene_line_instances, renderer_memory.scene_line_limit
        ),
        format!(
            "Text Glyphs  {}/{}",
            renderer_memory.scene_text_instances, renderer_memory.scene_text_limit
        ),
        format!(
            "Image Draws  {}/{}",
            renderer_memory.scene_image_instances, renderer_memory.scene_image_limit
        ),
        format!("Scene Buffers  {:.1}MB", scene_mb),
        format!("---GPU Memory---"),
        format!("Reserved GPU  {:.1}MB", reserved_mb),
        format!("Font Atlas    {:.1}MB", atlas_reserved_mb),
        format!("---Image Caching---"),
        format!("Image RAM     {:.1}MB/{:.0}MB", image_ram_mb, image_ram_total_mb),
        format!("Image VRAM    {:.1}MB/{:.0}MB", image_vram_mb, image_vram_total_mb),
        format!("Loaded Images {}/{}", atlas_count, atlas_total),
        format!("FPS   {:.0}", fps),
        frame_label,
    ];

    let mut text_specs = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        text_specs.push(
            UiTextSpec::top_left(
                line,
                Vec2::new(0.0, i as f32 * STATS_LINE_HEIGHT),
                STATS_TEXT_SIZE,
                text_color,
            )
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
        false,
    )]
}
