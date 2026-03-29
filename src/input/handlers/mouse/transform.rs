use glam::Vec2;
use crate::board::{
    Board, Element, ElementPropertyChange, ElementPropertyPatch, ElementTransform,
    ShapeType,
};
use crate::input::state::{DragMode, HandleDir, InputState, SelectionBounds};
use super::selection::current_multi_selection_bounds;

pub fn begin_transform_drag(state: &mut InputState, board: &Board, drag_mode: DragMode, world: Vec2) {
    state.pending_drag_mode = DragMode::None;
    state.drag_mode = drag_mode;
    state.move_origin = board
        .elements
        .iter()
        .filter(|element| element.selected)
        .map(|element| {
            (
                element.id,
                element.pos,
                element.size,
                element.rotation,
                element.line_bend,
                element.line_midpoint_shift,
            )
        })
        .collect();
    state.move_start_world = world;
    state.move_delta = Vec2::ZERO;
    state.rotate_delta = 0.0;
    state.transform_bounds_origin =
        current_multi_selection_bounds(state, board).or_else(|| board.selected_bounds());
    state.drag_selection_bounds = state.transform_bounds_origin;
}

pub fn transform_ids(state: &InputState) -> Vec<u64> {
    state
        .move_origin
        .iter()
        .map(|&(id, _, _, _, _, _)| id)
        .collect()
}

pub fn move_transform_changes(state: &InputState) -> Vec<ElementPropertyChange> {
    state
        .move_origin
        .iter()
        .filter_map(|&(id, orig_pos, orig_size, orig_rot, _, _)| {
            let before = ElementTransform::new(orig_pos, orig_size, orig_rot);
            let after = ElementTransform::new(orig_pos + state.move_delta, orig_size, orig_rot);
            (before != after).then_some(ElementPropertyChange {
                id,
                patch: ElementPropertyPatch::Transform { before, after },
            })
        })
        .collect()
}

pub fn rotation_angle_delta(start_world: Vec2, current_world: Vec2, center: Vec2) -> f32 {
    let start_vec = start_world - center;
    let current_vec = current_world - center;
    current_vec.y.atan2(current_vec.x) - start_vec.y.atan2(start_vec.x)
}

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

pub fn inverse_rotate_point(point: Vec2, center: Vec2, angle: f32) -> Vec2 {
    let offset = point - center;
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    center
        + Vec2::new(
            offset.x * cos_a + offset.y * sin_a,
            -offset.x * sin_a + offset.y * cos_a,
        )
}

pub fn scale_point_from_anchor(point: Vec2, anchor: Vec2, scale_x: f32, scale_y: f32) -> Vec2 {
    anchor
        + Vec2::new(
            (point.x - anchor.x) * scale_x,
            (point.y - anchor.y) * scale_y,
        )
}

pub fn rotate_vector(vector: Vec2, angle: f32) -> Vec2 {
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    Vec2::new(
        vector.x * cos_a - vector.y * sin_a,
        vector.x * sin_a + vector.y * cos_a,
    )
}

pub fn inverse_rotate_vector(vector: Vec2, angle: f32) -> Vec2 {
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    Vec2::new(
        vector.x * cos_a + vector.y * sin_a,
        -vector.x * sin_a + vector.y * cos_a,
    )
}

pub fn scale_vector_in_frame(vector: Vec2, frame_rotation: f32, scale_x: f32, scale_y: f32) -> Vec2 {
    let local_vector = inverse_rotate_vector(vector, frame_rotation);
    let scaled_local = Vec2::new(local_vector.x * scale_x, local_vector.y * scale_y);
    rotate_vector(scaled_local, frame_rotation)
}

pub fn scale_point_from_anchor_in_frame(
    point: Vec2,
    anchor: Vec2,
    scale_x: f32,
    scale_y: f32,
    frame_center: Vec2,
    frame_rotation: f32,
) -> Vec2 {
    let local_point = inverse_rotate_point(point, frame_center, frame_rotation);
    let local_anchor = inverse_rotate_point(anchor, frame_center, frame_rotation);
    let scaled_local = scale_point_from_anchor(local_point, local_anchor, scale_x, scale_y);
    rotate_point(scaled_local, frame_center, frame_rotation)
}

pub fn resize_rotated_element_in_frame(
    element: &mut Element,
    orig_pos: Vec2,
    orig_size: Vec2,
    orig_rot: f32,
    anchor: Vec2,
    scale_x: f32,
    scale_y: f32,
    frame_bounds: SelectionBounds,
) {
    let original_center = orig_pos + orig_size * 0.5;
    let scaled_center = scale_point_from_anchor_in_frame(
        original_center,
        anchor,
        scale_x,
        scale_y,
        frame_bounds.center(),
        frame_bounds.rotation,
    );

    let half_width_axis = rotate_vector(Vec2::new(orig_size.x * 0.5, 0.0), orig_rot);
    let half_height_axis = rotate_vector(Vec2::new(0.0, orig_size.y * 0.5), orig_rot);
    let scaled_width_axis =
        scale_vector_in_frame(half_width_axis, frame_bounds.rotation, scale_x, scale_y);
    let scaled_height_axis =
        scale_vector_in_frame(half_height_axis, frame_bounds.rotation, scale_x, scale_y);

    let width = (scaled_width_axis.length() * 2.0).max(1.0);
    let height = (scaled_height_axis.length() * 2.0).max(1.0);
    let rotation = if scaled_width_axis.length_squared() > 0.0001 {
        scaled_width_axis.y.atan2(scaled_width_axis.x)
    } else if scaled_height_axis.length_squared() > 0.0001 {
        scaled_height_axis.y.atan2(scaled_height_axis.x) - std::f32::consts::FRAC_PI_2
    } else {
        orig_rot
    };

    element.rotation = rotation;
    element.size = Vec2::new(width, height);
    element.pos = scaled_center - element.size * 0.5;
}

pub fn element_corners(element: &Element) -> Vec<Vec2> {
    if element.shape == ShapeType::Line {
        return vec![element.pos, element.pos + element.size];
    }

    let center = element.pos + element.size * 0.5;
    let half_size = element.size * 0.5;
    vec![
        rotate_point(
            center + Vec2::new(-half_size.x, -half_size.y),
            center,
            element.rotation,
        ),
        rotate_point(
            center + Vec2::new(half_size.x, -half_size.y),
            center,
            element.rotation,
        ),
        rotate_point(
            center + Vec2::new(half_size.x, half_size.y),
            center,
            element.rotation,
        ),
        rotate_point(
            center + Vec2::new(-half_size.x, half_size.y),
            center,
            element.rotation,
        ),
    ]
}

pub fn selection_bounds_from_selected_elements_in_frame(
    board: &Board,
    frame_rotation: f32,
) -> Option<SelectionBounds> {
    let mut local_min: Option<Vec2> = None;
    let mut local_max: Option<Vec2> = None;

    for element in board.elements.iter().filter(|element| element.selected) {
        for corner in element_corners(element) {
            let local_corner = inverse_rotate_vector(corner, frame_rotation);
            local_min = Some(match local_min {
                Some(current) => current.min(local_corner),
                None => local_corner,
            });
            local_max = Some(match local_max {
                Some(current) => current.max(local_corner),
                None => local_corner,
            });
        }
    }

    let (local_min, local_max) = match (local_min, local_max) {
        (Some(local_min), Some(local_max)) => (local_min, local_max),
        _ => return None,
    };

    let size = (local_max - local_min).max(Vec2::splat(1.0));
    let local_center = (local_min + local_max) * 0.5;
    let world_center = rotate_vector(local_center, frame_rotation);

    Some(SelectionBounds {
        pos: world_center - size * 0.5,
        size,
        rotation: frame_rotation,
    })
}

pub fn resized_selection_bounds(
    bounds: SelectionBounds,
    dir: HandleDir,
    world: Vec2,
) -> Option<SelectionBounds> {
    let center = bounds.center();
    let local_world = inverse_rotate_point(world, center, bounds.rotation);
    let min = bounds.min();
    let max = bounds.max();
    let anchor = match dir {
        HandleDir::TL => max,
        HandleDir::TR => Vec2::new(min.x, max.y),
        HandleDir::BR => min,
        HandleDir::BL => Vec2::new(max.x, min.y),
        HandleDir::Top => Vec2::new(bounds.center().x, max.y),
        HandleDir::Right => Vec2::new(min.x, bounds.center().y),
        HandleDir::Bottom => Vec2::new(bounds.center().x, min.y),
        HandleDir::Left => Vec2::new(max.x, bounds.center().y),
        _ => return None,
    };
    let mut local_min = min;
    let mut local_max = max;

    match dir {
        HandleDir::TL | HandleDir::TR | HandleDir::BR | HandleDir::BL => {
            local_min = local_world.min(anchor);
            local_max = local_world.max(anchor);
        }
        HandleDir::Top => {
            local_min.y = local_world.y.min(anchor.y - 1.0);
            local_max.y = anchor.y;
        }
        HandleDir::Right => {
            local_min.x = anchor.x;
            local_max.x = local_world.x.max(anchor.x + 1.0);
        }
        HandleDir::Bottom => {
            local_min.y = anchor.y;
            local_max.y = local_world.y.max(anchor.y + 1.0);
        }
        HandleDir::Left => {
            local_min.x = local_world.x.min(anchor.x - 1.0);
            local_max.x = anchor.x;
        }
        _ => return None,
    }

    Some(SelectionBounds {
        pos: local_min,
        size: (local_max - local_min).max(Vec2::splat(1.0)),
        rotation: bounds.rotation,
    })
}

pub fn group_resize_from_handle(
    bounds: SelectionBounds,
    dir: HandleDir,
    world: Vec2,
) -> Option<(Vec2, f32, f32)> {
    let center = bounds.center();
    let local_world = inverse_rotate_point(world, center, bounds.rotation);
    let min = bounds.min();
    let max = bounds.max();
    let width = bounds.size.x.max(1.0);
    let height = bounds.size.y.max(1.0);

    match dir {
        HandleDir::TL => Some((
            rotate_point(max, center, bounds.rotation),
            (max.x - local_world.x) / width,
            (max.y - local_world.y) / height,
        )),
        HandleDir::TR => Some((
            rotate_point(Vec2::new(min.x, max.y), center, bounds.rotation),
            (local_world.x - min.x) / width,
            (max.y - local_world.y) / height,
        )),
        HandleDir::BR => Some((
            rotate_point(min, center, bounds.rotation),
            (local_world.x - min.x) / width,
            (local_world.y - min.y) / height,
        )),
        HandleDir::BL => Some((
            rotate_point(Vec2::new(max.x, min.y), center, bounds.rotation),
            (max.x - local_world.x) / width,
            (local_world.y - min.y) / height,
        )),
        HandleDir::Top => Some((
            rotate_point(
                Vec2::new((min.x + max.x) * 0.5, max.y),
                center,
                bounds.rotation,
            ),
            1.0,
            (max.y - local_world.y) / height,
        )),
        HandleDir::Right => Some((
            rotate_point(
                Vec2::new(min.x, (min.y + max.y) * 0.5),
                center,
                bounds.rotation,
            ),
            (local_world.x - min.x) / width,
            1.0,
        )),
        HandleDir::Bottom => Some((
            rotate_point(
                Vec2::new((min.x + max.x) * 0.5, min.y),
                center,
                bounds.rotation,
            ),
            1.0,
            (local_world.y - min.y) / height,
        )),
        HandleDir::Left => Some((
            rotate_point(
                Vec2::new(max.x, (min.y + max.y) * 0.5),
                center,
                bounds.rotation,
            ),
            (max.x - local_world.x) / width,
            1.0,
        )),
        _ => None,
    }
}
