use crate::board::ShapeType;

pub fn default_color(shape: ShapeType) -> [f32; 4] {
    match shape {
        ShapeType::Rect => [0.97, 0.96, 0.90, 0.85], // rgb(247, 246, 229)
        ShapeType::Ellipse => [0.73, 0.80, 0.39, 0.85], // rgb(187, 203, 100)
        ShapeType::Line => [0.85, 0.28, 0.28, 1.00], // rgb(218, 72, 72)
    }
}