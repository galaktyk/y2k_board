use glam::Vec2;

use crate::board::{line_bend_handle_position, Element, ShapeType};
use crate::input::state::{HandleDir, SelectionBounds};
use crate::palette;
use crate::rendering::renderer::InstanceData;

const HANDLE_SIZE_PX: f32 = 10.0;
const ROTATION_HANDLE_OFFSET_PX: f32 = 30.0;
const CONNECTION_HELPER_SIZE_PX: f32 = 12.0;
const CONNECTION_HELPER_OFFSET_PX: f32 = 20.0;
const EDGE_HIT_MARGIN_PX: f32 = 10.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConnectionHelper {
    pub point: Vec2,
    pub norm_pos: Vec2,
}

fn world_units_per_screen_px(zoom: f32) -> f32 {
    1.0 / zoom.max(0.0001)
}

pub fn handle_hit_radius(zoom: f32) -> f32 {
    15.0 * world_units_per_screen_px(zoom)
}

pub fn edge_hit_margin(zoom: f32) -> f32 {
    EDGE_HIT_MARGIN_PX * world_units_per_screen_px(zoom)
}

fn push_circle_handle_instances(
    out: &mut Vec<InstanceData>,
    point: Vec2,
    size: f32,
    fill_color: [f32; 4],
) {
    out.push(
        InstanceData::new(
            [point.x - size * 0.5, point.y - size * 0.5],
            [size, size],
            0.0,
            palette::GRAY_TRANSPARENT,
            4.0,
            1.0,
            false,
        )
        .with_stroke_width(2),
    );
    out.push(InstanceData::new(
        [point.x - size * 0.5, point.y - size * 0.5],
        [size, size],
        0.0,
        fill_color,
        1.0,
        1.0,
        false,
    ));
}

pub fn get_connection_helpers(e: &Element, zoom: f32) -> Option<[ConnectionHelper; 4]> {
    if e.shape == ShapeType::Line {
        return None;
    }

    let center = e.pos + e.size * 0.5;
    let c = e.rotation.cos();
    let s = e.rotation.sin();
    let rot = |rx: f32, ry: f32| -> Vec2 { center + Vec2::new(rx * c - ry * s, rx * s + ry * c) };

    let hw = e.size.x * 0.5;
    let hh = e.size.y * 0.5;
    let offset = CONNECTION_HELPER_OFFSET_PX * world_units_per_screen_px(zoom);

    Some([
        ConnectionHelper {
            point: rot(0.0, -hh - offset),
            norm_pos: Vec2::new(0.5, 0.0),
        },
        ConnectionHelper {
            point: rot(hw + offset, 0.0),
            norm_pos: Vec2::new(1.0, 0.5),
        },
        ConnectionHelper {
            point: rot(0.0, hh + offset),
            norm_pos: Vec2::new(0.5, 1.0),
        },
        ConnectionHelper {
            point: rot(-hw - offset, 0.0),
            norm_pos: Vec2::new(0.0, 0.5),
        },
    ])
}

pub fn get_element_handles(e: &Element, zoom: f32) -> Option<Vec<Vec2>> {
    if e.shape == ShapeType::Line {
        return Some(vec![e.pos, e.pos + e.size, line_bend_handle_position(e)]);
    }
    let center = e.pos + e.size * 0.5;
    let c = e.rotation.cos();
    let s = e.rotation.sin();
    let rot = |rx: f32, ry: f32| -> Vec2 { center + Vec2::new(rx * c - ry * s, rx * s + ry * c) };

    let hw = e.size.x * 0.5;
    let hh = e.size.y * 0.5;
    let offset = ROTATION_HANDLE_OFFSET_PX * world_units_per_screen_px(zoom);
    let rx = -hw - offset;
    let ry = hh + offset;

    Some(vec![
        rot(-hw, -hh),
        rot(hw, -hh),
        rot(hw, hh),
        rot(-hw, hh),
        rot(rx, ry),
    ])
}

pub fn get_selection_bounds_handles(bounds: SelectionBounds, zoom: f32) -> Vec<Vec2> {
    let [tl, tr, br, bl] = bounds.corners();
    let offset = ROTATION_HANDLE_OFFSET_PX * world_units_per_screen_px(zoom);
    let rotate_handle =
        bounds.rotate_point(bounds.pos + Vec2::new(-offset, bounds.size.y + offset));

    vec![tl, tr, br, bl, rotate_handle]
}

pub fn handles_to_instances(e: &Element, zoom: f32) -> Vec<InstanceData> {
    let mut out = Vec::new();
    let handles = match get_element_handles(e, zoom) {
        Some(handles) => handles,
        None => return out,
    };

    let world_per_px = world_units_per_screen_px(zoom);
    let handle_size = HANDLE_SIZE_PX * world_per_px;
    if e.shape == ShapeType::Line {
        for (index, pt) in handles.into_iter().enumerate() {
            let fill = if index == 2 {
                palette::TEAL
            } else {
                [1.0, 1.0, 1.0, 1.0]
            };
            push_circle_handle_instances(&mut out, pt, handle_size, fill);
        }
        return out;
    }

    for pt in handles.iter().take(4) {
        push_circle_handle_instances(&mut out, *pt, handle_size, [1.0, 1.0, 1.0, 1.0]);
    }

    out
}

pub fn selection_bounds_handles_to_instances(
    bounds: SelectionBounds,
    zoom: f32,
) -> Vec<InstanceData> {
    let mut out = Vec::new();
    let world_per_px = world_units_per_screen_px(zoom);
    let handles = get_selection_bounds_handles(bounds, zoom);
    let handle_size = HANDLE_SIZE_PX * world_per_px;

    for pt in handles.iter().take(4) {
        push_circle_handle_instances(&mut out, *pt, handle_size, [1.0, 1.0, 1.0, 1.0]);
    }

    out
}

pub fn connection_helpers_to_instances(e: &Element, zoom: f32) -> Vec<InstanceData> {
    let mut out = Vec::new();
    let Some(helpers) = get_connection_helpers(e, zoom) else {
        return out;
    };

    let helper_size = CONNECTION_HELPER_SIZE_PX * world_units_per_screen_px(zoom);
    for helper in helpers {
        push_circle_handle_instances(&mut out, helper.point, helper_size, palette::TEAL);
    }

    out
}

pub fn element_resize_bounds(element: &Element) -> Option<SelectionBounds> {
    (element.shape != ShapeType::Line).then_some(SelectionBounds {
        pos: element.pos,
        size: element.size.abs().max(Vec2::splat(1.0)),
        rotation: element.rotation,
    })
}

pub fn edge_handle_hit(bounds: SelectionBounds, world: Vec2, zoom: f32) -> Option<HandleDir> {
    let center = bounds.center();
    let offset = world - center;
    let c = bounds.rotation.cos();
    let s = bounds.rotation.sin();
    let local = Vec2::new(offset.x * c + offset.y * s, -offset.x * s + offset.y * c);
    let half = bounds.size.max(Vec2::splat(1.0)) * 0.5;
    let margin = edge_hit_margin(zoom);

    if local.x < -half.x - margin
        || local.x > half.x + margin
        || local.y < -half.y - margin
        || local.y > half.y + margin
    {
        return None;
    }

    let dist_left = (local.x + half.x).abs();
    let dist_right = (local.x - half.x).abs();
    let dist_top = (local.y + half.y).abs();
    let dist_bottom = (local.y - half.y).abs();

    let inside_vertical_span = local.y >= -half.y - margin && local.y <= half.y + margin;
    let inside_horizontal_span = local.x >= -half.x - margin && local.x <= half.x + margin;

    let candidates = [
        (dist_left, HandleDir::Left, inside_vertical_span),
        (dist_right, HandleDir::Right, inside_vertical_span),
        (dist_top, HandleDir::Top, inside_horizontal_span),
        (dist_bottom, HandleDir::Bottom, inside_horizontal_span),
    ];

    candidates
        .into_iter()
        .filter(|(distance, _, inside_span)| *inside_span && *distance <= margin)
        .min_by(|(distance_a, _, _), (distance_b, _, _)| distance_a.total_cmp(distance_b))
        .map(|(_, dir, _)| dir)
}
