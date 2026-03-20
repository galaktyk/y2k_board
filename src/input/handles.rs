use glam::Vec2;

use crate::board::{Element, ShapeType};
use crate::input::state::SelectionBounds;
use crate::palette;
use crate::rendering::renderer::InstanceData;

const HANDLE_SIZE_PX: f32 = 10.0;
const ROTATION_HANDLE_OFFSET_PX: f32 = 30.0;
const CONNECTION_HELPER_SIZE_PX: f32 = 12.0;
const CONNECTION_HELPER_OFFSET_PX: f32 = 20.0;

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
    let rot = |rx: f32, ry: f32| -> Vec2 {
        center + Vec2::new(rx * c - ry * s, rx * s + ry * c)
    };

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
        return Some(vec![e.pos, e.pos + e.size]);
    }
    let center = e.pos + e.size * 0.5;
    let c = e.rotation.cos();
    let s = e.rotation.sin();
    let rot = |rx: f32, ry: f32| -> Vec2 {
        center + Vec2::new(rx * c - ry * s, rx * s + ry * c)
    };

    let hw = e.size.x * 0.5;
    let hh = e.size.y * 0.5;
    let offset = ROTATION_HANDLE_OFFSET_PX * world_units_per_screen_px(zoom) ;
    let rx = -hw - offset;
    let ry = hh + offset;

    Some(vec![rot(-hw, -hh), rot(hw, -hh), rot(hw, hh), rot(-hw, hh), rot(rx, ry)])
}

pub fn get_selection_bounds_handles(bounds: SelectionBounds, zoom: f32) -> Vec<Vec2> {
    let [tl, tr, br, bl] = bounds.corners();
    let offset = ROTATION_HANDLE_OFFSET_PX * world_units_per_screen_px(zoom) ;
    let rotate_handle = bounds.rotate_point(
        bounds.pos
            + Vec2::new(
                -offset,
                bounds.size.y + offset,
            ),
    );

    vec![
        tl,
        tr,
        br,
        bl,
        rotate_handle,
    ]
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
        for pt in handles {
            push_circle_handle_instances(&mut out, pt, handle_size, [1.0, 1.0, 1.0, 1.0]);
        }
        return out;
    }

    for pt in handles.iter().take(4) {
        push_circle_handle_instances(&mut out, *pt, handle_size, [1.0, 1.0, 1.0, 1.0]);
    }

    out
}

pub fn selection_bounds_handles_to_instances(bounds: SelectionBounds, zoom: f32) -> Vec<InstanceData> {
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