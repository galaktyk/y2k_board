use cosmic_text::{Attrs, Color, Family, Metrics};
use crate::palette;

pub fn text_metrics(font_size: f32, line_height: Option<f32>) -> Metrics {
    let font_size = font_size.max(8.0);
    Metrics::new(
        font_size,
        line_height.unwrap_or((font_size * 1.35).max(font_size + 4.0)),
    )
}

pub fn default_text_attrs(content: &str, color: [f32; 4]) -> Attrs<'static> {
    Attrs::new()
        .family(preferred_family_for_text(content))
        .color(rgba_to_cosmic_color(color))
}

pub fn preferred_family_for_text(text: &str) -> Family<'static> {
    if text.chars().any(is_emoji_like) {
        Family::Name("Noto Emoji")
    } else if text.chars().any(is_symbol_like) {
        Family::Name("DejaVu Sans")
    } else {
        Family::SansSerif
    }
}

pub fn is_emoji_like(ch: char) -> bool {
    let codepoint = ch as u32;
    matches!(codepoint, 0x1F300..=0x1FAFF | 0xFE0E..=0xFE0F)
}

pub fn is_symbol_like(ch: char) -> bool {
    let codepoint = ch as u32;
    matches!(codepoint, 0x2190..=0x21FF | 0x2300..=0x23FF | 0x25A0..=0x25FF | 0x2600..=0x27BF)
}

pub fn cosmic_color_to_rgba(color: Color) -> [f32; 4] {
    [
        color.r() as f32 / 255.0,
        color.g() as f32 / 255.0,
        color.b() as f32 / 255.0,
        color.a() as f32 / 255.0,
    ]
}

pub fn rgba_to_cosmic_color(color: [f32; 4]) -> Color {
    Color::rgba(
        (color[0].clamp(0.0, 1.0) * 255.0) as u8,
        (color[1].clamp(0.0, 1.0) * 255.0) as u8,
        (color[2].clamp(0.0, 1.0) * 255.0) as u8,
        (color[3].clamp(0.0, 1.0) * 255.0) as u8,
    )
}

pub const SELECTION_COLOR: [f32; 4] = palette::TEXT_SELECTION_COLOR;
pub const CARET_COLOR: [f32; 4] = palette::GRAY_3;
