use glam::Vec2;

use crate::board::{Element, ShapeType};
use crate::input::state::SelectionBounds;
use crate::rendering::renderer::InstanceData;

const HANDLE_SIZE_PX: f32 = 10.0;
const ROTATION_HANDLE_OFFSET_PX: f32 = 30.0;

fn world_units_per_screen_px(zoom: f32) -> f32 {
    1.0 / zoom.max(0.0001)
}

pub fn handle_hit_radius(zoom: f32) -> f32 {
    15.0 * world_units_per_screen_px(zoom)
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
            out.push(InstanceData::new(
                [pt.x - handle_size * 0.5, pt.y - handle_size * 0.5],
                [handle_size, handle_size],
                0.0,
                [1.0, 1.0, 1.0, 1.0],
                1.0,
                1.0, false,
            ));
        }
        return out;
    }

    for pt in handles.iter().take(4) {
        out.push(InstanceData::new(
            [pt.x - handle_size * 0.5, pt.y - handle_size * 0.5],
            [handle_size, handle_size],
            0.0,
            [1.0, 1.0, 1.0, 1.0],
            1.0,
            1.0, false,
        ));
    }

    out
}

pub fn selection_bounds_handles_to_instances(bounds: SelectionBounds, zoom: f32) -> Vec<InstanceData> {
    let mut out = Vec::new();
    let world_per_px = world_units_per_screen_px(zoom);
    let handles = get_selection_bounds_handles(bounds, zoom);
    let handle_size = HANDLE_SIZE_PX * world_per_px;

    for pt in handles.iter().take(4) {
        out.push(InstanceData::new(
            [pt.x - handle_size * 0.5, pt.y - handle_size * 0.5],
            [handle_size, handle_size],
            0.0,
            [1.0, 1.0, 1.0, 1.0],
            1.0,
            1.0, false,
        ));
    }

    out
}