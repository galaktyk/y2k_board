use glam::Vec2;

use super::{Element, ShapeType};

const CURVE_EPSILON: f32 = 0.001;
const MIN_CURVE_TANGENT_OFFSET: f32 = 8.0;
const MAX_CURVE_TANGENT_OFFSET: f32 = 160.0;
const MIN_CURVE_SEGMENTS: usize = 1;
const MAX_CURVE_SEGMENTS: usize = 24;
const MIDPOINT_HANDLE_TO_CONTROL_SHIFT: f32 = 4.0 / 3.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubicBezier {
    pub p0: Vec2,
    pub c1: Vec2,
    pub c2: Vec2,
    pub p3: Vec2,
}

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
        ShapeType::Line => line_hit(element, point),
    }
}

pub fn line_bend_axis(line: &Element) -> Vec2 {
    let chord = line.size;
    let len = chord.length();
    if len <= CURVE_EPSILON {
        Vec2::Y
    } else {
        Vec2::new(-chord.y / len, chord.x / len)
    }
}

pub fn line_chord_axis(line: &Element) -> Vec2 {
    let chord = line.size;
    let len = chord.length();
    if len <= CURVE_EPSILON {
        Vec2::X
    } else {
        chord / len
    }
}

pub fn line_bend_handle_position(line: &Element) -> Vec2 {
    if let Some(curve) = line_curve(line) {
        sample_cubic(curve, 0.5)
    } else {
        let (start, end) = line.line_endpoints();
        (start + end) * 0.5
    }
}

pub fn line_curve_handle_offset_from_handle(line: &Element, handle_world: Vec2) -> Vec2 {
    let midpoint = base_curve_midpoint(line);
    let delta = handle_world - midpoint;
    Vec2::new(
        delta.dot(line_chord_axis(line)),
        delta.dot(line_bend_axis(line)),
    )
}

pub fn line_curve(element: &Element) -> Option<CubicBezier> {
    line_curve_from_state(
        element.pos,
        element.size,
        element.line_bend,
        element.line_midpoint_shift,
        element.line_start_normal,
        element.line_end_normal,
    )
}

pub fn line_curve_from_state(
    pos: Vec2,
    size: Vec2,
    line_bend: f32,
    line_midpoint_shift: f32,
    line_start_normal: Option<Vec2>,
    line_end_normal: Option<Vec2>,
) -> Option<CubicBezier> {
    let mut curve = base_line_curve_from_state(pos, size, line_start_normal, line_end_normal)?;
    let control_shift =
        line_curve_midpoint_offset_from_state(size, line_bend, line_midpoint_shift)
            * MIDPOINT_HANDLE_TO_CONTROL_SHIFT;
    curve.c1 += control_shift;
    curve.c2 += control_shift;

    Some(curve)
}

fn base_line_curve(element: &Element) -> Option<CubicBezier> {
    base_line_curve_from_state(
        element.pos,
        element.size,
        element.line_start_normal,
        element.line_end_normal,
    )
}

fn base_line_curve_from_state(
    pos: Vec2,
    size: Vec2,
    line_start_normal: Option<Vec2>,
    line_end_normal: Option<Vec2>,
) -> Option<CubicBezier> {
    let p0 = pos;
    let p3 = pos + size;
    let chord = p3 - p0;
    let len = chord.length();
    if len <= CURVE_EPSILON {
        return None;
    }

    let dir = chord / len;
    let start_normal = line_start_normal.and_then(normalize_or_none);
    let end_normal = line_end_normal.and_then(normalize_or_none).map(|normal| -normal);
    let start_dir = start_normal.unwrap_or(dir);
    let end_dir = end_normal.unwrap_or(dir);
    let start_offset = tangent_offset_for_direction(len, chord, start_dir, start_normal.is_some());
    let end_offset = tangent_offset_for_direction(len, chord, end_dir, end_normal.is_some());

    let c1 = p0 + start_dir * start_offset;
    let c2 = p3 - end_dir * end_offset;

    Some(CubicBezier { p0, c1, c2, p3 })
}

pub fn sample_cubic(curve: CubicBezier, t: f32) -> Vec2 {
    let mt = 1.0 - t;
    curve.p0 * (mt * mt * mt)
        + curve.c1 * (3.0 * mt * mt * t)
        + curve.c2 * (3.0 * mt * t * t)
        + curve.p3 * (t * t * t)
}

pub fn sample_line_polyline(element: &Element) -> Vec<Vec2> {
    if element.shape != ShapeType::Line {
        return Vec::new();
    }

    let mut points = Vec::with_capacity(usize::from(line_sample_count(element)));
    visit_line_polyline_points(element, |point| points.push(point));
    points
}

pub fn line_aabb(element: &Element, expand: f32) -> (Vec2, Vec2) {
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);

    visit_line_polyline_points(element, |point| {
        min = min.min(point);
        max = max.max(point);
    });

    if !min.is_finite() || !max.is_finite() {
        let (start, end) = element.line_endpoints();
        min = start.min(end);
        max = start.max(end);
    }

    let pad = Vec2::splat(expand + f32::from(element.stroke_width.max(1)) * 0.5);
    (min - pad, max + pad)
}

pub fn line_world_normals_from_anchor(norm_pos: Vec2, rotation: f32) -> Vec2 {
    let local = if norm_pos.x <= 0.001 {
        Vec2::new(-1.0, 0.0)
    } else if norm_pos.x >= 0.999 {
        Vec2::new(1.0, 0.0)
    } else if norm_pos.y <= 0.001 {
        Vec2::new(0.0, -1.0)
    } else if norm_pos.y >= 0.999 {
        Vec2::new(0.0, 1.0)
    } else {
        let centered = norm_pos - Vec2::splat(0.5);
        if centered.length_squared() <= CURVE_EPSILON {
            Vec2::new(1.0, 0.0)
        } else {
            centered.normalize()
        }
    };

    let c = rotation.cos();
    let s = rotation.sin();
    Vec2::new(local.x * c - local.y * s, local.x * s + local.y * c)
}

fn line_hit(element: &Element, point: Vec2) -> bool {
    let tolerance = f32::from(element.stroke_width.max(1)) * 0.5 + 8.0;
    let mut previous = None;
    let mut min_distance = f32::INFINITY;

    visit_line_polyline_points(element, |current| {
        if let Some(start) = previous {
            min_distance = min_distance.min(dist_point_segment(point, start, current));
        }
        previous = Some(current);
    });

    min_distance <= tolerance
}

fn line_curve_segment_count(element: &Element, curve: CubicBezier) -> usize {
    let chord = (curve.p3 - curve.p0).length();
    let control_span = (curve.c1 - curve.p0).length() + (curve.c2 - curve.p3).length();
    let midpoint_offset = Vec2::new(element.line_midpoint_shift, element.line_bend).length();
    let estimated = ((chord + control_span + midpoint_offset * 2.0) / 48.0).ceil() as usize;
    estimated
        .clamp(MIN_CURVE_SEGMENTS, MAX_CURVE_SEGMENTS)
        .max(1)
}

fn normalize_or_none(v: Vec2) -> Option<Vec2> {
    (v.length_squared() > CURVE_EPSILON * CURVE_EPSILON).then_some(v.normalize())
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

fn base_curve_midpoint(line: &Element) -> Vec2 {
    if let Some(curve) = base_line_curve(line) {
        sample_cubic(curve, 0.5)
    } else {
        let (start, end) = line.line_endpoints();
        (start + end) * 0.5
    }
}

fn line_curve_midpoint_offset(line: &Element) -> Vec2 {
    line_curve_midpoint_offset_from_state(line.size, line.line_bend, line.line_midpoint_shift)
}

fn line_curve_midpoint_offset_from_state(
    line_size: Vec2,
    line_bend: f32,
    line_midpoint_shift: f32,
) -> Vec2 {
    chord_axis_from_size(line_size) * line_midpoint_shift + bend_axis_from_size(line_size) * line_bend
}

fn bend_axis_from_size(size: Vec2) -> Vec2 {
    let len = size.length();
    if len <= CURVE_EPSILON {
        Vec2::Y
    } else {
        Vec2::new(-size.y / len, size.x / len)
    }
}

fn chord_axis_from_size(size: Vec2) -> Vec2 {
    let len = size.length();
    if len <= CURVE_EPSILON {
        Vec2::X
    } else {
        size / len
    }
}

fn line_sample_count(element: &Element) -> u8 {
    let count = if let Some(curve) = line_curve(element) {
        line_curve_segment_count(element, curve) + 1
    } else if element.shape == ShapeType::Line {
        2
    } else {
        0
    };
    count as u8
}

fn visit_line_polyline_points<F>(element: &Element, mut visit: F)
where
    F: FnMut(Vec2),
{
    if element.shape != ShapeType::Line {
        return;
    }

    let Some(curve) = line_curve(element) else {
        let (start, end) = element.line_endpoints();
        visit(start);
        visit(end);
        return;
    };

    let segments = line_curve_segment_count(element, curve);
    for step in 0..=segments {
        let t = step as f32 / segments as f32;
        visit(sample_cubic(curve, t));
    }
}

fn tangent_offset_for_direction(
    length: f32,
    chord: Vec2,
    tangent_dir: Vec2,
    anchored_to_normal: bool,
) -> f32 {
    let projected = chord.dot(tangent_dir).abs();
    let alignment = (projected / length.max(CURVE_EPSILON)).clamp(0.0, 1.0);
    let perpendicularity = if anchored_to_normal {
        1.0 - alignment
    } else {
        0.0
    };
    let desired = length * (0.28 + perpendicularity * 0.14);
    let directional_cap = if projected > CURVE_EPSILON {
        projected * 0.65 + length * (0.08 + perpendicularity * 0.14)
    } else {
        length * (0.18 + perpendicularity * 0.24)
    };

    let max_offset_scale = 0.5 + perpendicularity * 0.1;
    let max_offset =
        MAX_CURVE_TANGENT_OFFSET.min((length * max_offset_scale).max(MIN_CURVE_TANGENT_OFFSET));
    desired
        .min(directional_cap)
        .clamp(MIN_CURVE_TANGENT_OFFSET.min(max_offset), max_offset)
}
