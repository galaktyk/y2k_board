use glam::Vec2;

use super::{Element, ShapeType};

// Shared board-space geometry helpers live here so transform math and hit-testing
// stay separate from board state mutation and history management.

pub fn rotate_point(point: Vec2, center: Vec2, angle: f32) -> Vec2 {
    let offset = point - center;
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    center
        + Vec2::new(
            offset.x * cos_a - offset.y * sin_a,
            offset.x * sin_a + offset.y * cos_a,
        )
}

pub fn world_to_local_norm(world: Vec2, target: &Element) -> Vec2 {
    let origin = target.pos + target.size * 0.5;
    let local = rotate_point(world, origin, -target.rotation);
    (local - target.pos) / target.size
}

pub(super) fn element_hit(element: &Element, mut point: Vec2) -> bool {
    let center = element.pos + element.size * 0.5;
    let cos_r = element.rotation.cos();
    let sin_r = element.rotation.sin();
    let dx = point.x - center.x;
    let dy = point.y - center.y;
    point.x = center.x + dx * cos_r + dy * sin_r;
    point.y = center.y - dx * sin_r + dy * cos_r;

    match element.shape {
        ShapeType::Rect | ShapeType::Image => {
            let min_x = element.pos.x.min(element.pos.x + element.size.x);
            let max_x = element.pos.x.max(element.pos.x + element.size.x);
            let min_y = element.pos.y.min(element.pos.y + element.size.y);
            let max_y = element.pos.y.max(element.pos.y + element.size.y);
            point.x >= min_x && point.x <= max_x && point.y >= min_y && point.y <= max_y
        }
        ShapeType::Ellipse => {
            let center = element.pos + element.size * 0.5;
            let radius = (element.size * 0.5).abs();
            if radius.x == 0.0 || radius.y == 0.0 {
                return false;
            }
            let delta = (point - center) / radius;
            delta.dot(delta) <= 1.0
        }
        ShapeType::Line => {
            let start = element.pos;
            let end = element.pos + element.size;
            dist_point_segment(point, start, end)
                <= (f32::from(element.stroke_width.max(1)) * 0.5 + 8.0)
        }
    }
}

fn dist_point_segment(point: Vec2, start: Vec2, end: Vec2) -> f32 {
    let segment = end - start;
    let len2 = segment.dot(segment);
    if len2 == 0.0 {
        return (point - start).length();
    }

    let t = ((point - start).dot(segment) / len2).clamp(0.0, 1.0);
    (point - (start + segment * t)).length()
}