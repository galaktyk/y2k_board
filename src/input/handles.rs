use glam::Vec2;

use crate::board::{Element, ShapeType};
use crate::input::state::SelectionBounds;
use crate::rendering::renderer::InstanceData;

const HANDLE_SIZE_PX: f32 = 10.0;
const ROTATION_HANDLE_OFFSET_PX: f32 = 30.0;
const ROTATION_STICK_LENGTH_PX: f32 = 30.0;
const ROTATION_STICK_WIDTH_PX: f32 = 2.0;

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
    let th = -hh - ROTATION_HANDLE_OFFSET_PX * world_units_per_screen_px(zoom);

    Some(vec![rot(-hw, -hh), rot(hw, -hh), rot(hw, hh), rot(-hw, hh), rot(0.0, th)])
}

pub fn get_selection_bounds_handles(bounds: SelectionBounds, zoom: f32) -> Vec<Vec2> {
    let [tl, tr, br, bl] = bounds.corners();
    let top_center = bounds.rotate_point(
        bounds.pos
            + Vec2::new(
                bounds.size.x * 0.5,
                -ROTATION_HANDLE_OFFSET_PX * world_units_per_screen_px(zoom),
            ),
    );

    vec![
        tl,
        tr,
        br,
        bl,
        top_center,
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

    let center = e.pos + e.size * 0.5;
    let c = e.rotation.cos();
    let s_rot = e.rotation.sin();
    let rot = |rx: f32, ry: f32| -> Vec2 {
        center + Vec2::new(rx * c - ry * s_rot, rx * s_rot + ry * c)
    };
    let stick_half_length = ROTATION_STICK_LENGTH_PX * 0.5 * world_per_px;
    let stick_width = ROTATION_STICK_WIDTH_PX * world_per_px;
    let stick_center = rot(0.0, -e.size.y * 0.5 - stick_half_length);

    out.push(InstanceData::new(
        [stick_center.x - stick_width * 0.5, stick_center.y - stick_half_length],
        [stick_width, stick_half_length * 2.0],
        e.rotation,
        [1.0, 1.0, 1.0, 0.9],
        0.0,
        1.0, false,
    ));

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

    out
}

pub fn selection_bounds_handles_to_instances(bounds: SelectionBounds, zoom: f32) -> Vec<InstanceData> {
    let mut out = Vec::new();
    let world_per_px = world_units_per_screen_px(zoom);
    let handles = get_selection_bounds_handles(bounds, zoom);
    let handle_size = HANDLE_SIZE_PX * world_per_px;
    let stick_half_length = ROTATION_STICK_LENGTH_PX * 0.5 * world_per_px;
    let stick_width = ROTATION_STICK_WIDTH_PX * world_per_px;
    let stick_center = bounds.rotate_point(
        bounds.pos + Vec2::new(bounds.size.x * 0.5, -stick_half_length),
    );

    out.push(InstanceData::new(
        [stick_center.x - stick_width * 0.5, stick_center.y - stick_half_length],
        [stick_width, stick_half_length * 2.0],
        bounds.rotation,
        [1.0, 1.0, 1.0, 0.9],
        0.0,
        1.0, false,
    ));

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

    out
}