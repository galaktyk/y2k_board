use glam::Vec2;
use crate::board::{
    Element, ElementKind, ShapeType, ToolStyleDefaults,
};

pub const STICKY_NOTE_SIZE: f32 = 128.0;

pub fn sticky_note_element(tool_style_defaults: &ToolStyleDefaults, pos: Vec2) -> Element {
    Element {
        id: 0,
        shape: ShapeType::Rect,
        kind: ElementKind::StickyNote,
        pos,
        size: Vec2::splat(STICKY_NOTE_SIZE),
        rotation: 0.0,
        color: tool_style_defaults.sticky.fill_color,
        stroke_color: tool_style_defaults.sticky.stroke_color,
        border_width: tool_style_defaults.sticky.border_width,
        stroke_width: crate::board::DEFAULT_LINE_STROKE_WIDTH,
        line_arrow_start: false,
        line_arrow_end: false,
        line_bend: 0.0,
        line_midpoint_shift: 0.0,
        line_start_normal: None,
        line_end_normal: None,
        selected: false,
        text: Some(crate::board::TextData {
            content: String::new(),
            font_size: 24.0,
            color: tool_style_defaults.sticky.text_color,
        }),
        image: None,
        text_layout_generation: 0,
    }
}
