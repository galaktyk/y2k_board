use crate::palette;


use crate::board::{
    default_text_box_color, ShapeType, DEFAULT_ELLIPSE_COLOR, DEFAULT_LINE_COLOR,
    DEFAULT_RECT_COLOR,
};






pub fn default_color(shape: ShapeType) -> [f32; 4] {
    match shape {
        ShapeType::Rect => DEFAULT_RECT_COLOR,
        ShapeType::Ellipse => DEFAULT_ELLIPSE_COLOR,
        ShapeType::Line => DEFAULT_LINE_COLOR,
        ShapeType::Text => default_text_box_color(),
        ShapeType::Image => palette::PALETTE_PURE_BLACK,
    }
}