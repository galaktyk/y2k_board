use glam::Vec2;


use crate::board::{
    world_to_local_norm, Board, BoardOperation, Element, ElementPropertyChange,
    ElementPropertyPatch, ElementTransform, LineAnchor, LineConnectionChange, LineEndpoints,
    ShapeType, ToolStyleDefaults,
};
use crate::camera::Camera;
use crate::input::handles::{get_element_handles, get_selection_bounds_handles, handle_hit_radius};
use crate::input::state::{DragMode, HandleDir, InputState, SelectionBounds};
use crate::ui::tool::Tool;

const MARQUEE_MIN_SIZE: f32 = 4.0;
const DRAG_START_DISTANCE: f32 = 3.0;
const COMPUTE_TEXT_LAYOUT_DEBOUNCE: f64 = 0.05;

// The pan velocity smoothing factor, between 0 and 1. 
// Higher values make the velocity change more smoothly but also more slowly.
const PAN_VELOCITY_SMOOTHING: f32 = 0.45;

// Minimum initial pan velocity (in screen pixels per second) required to trigger pan glide on mouse release.
const PAN_GLIDE_MIN_LAUNCH_SPEED_SCREEN: f32 = 120.0;

const PAN_GLIDE_MAX_LAUNCH_SPEED_SCREEN: f32 = 4000.0;

// If the user releases the mouse after panning but has been idle for more than this duration, we won't trigger pan glide.
const PAN_GLIDE_MAX_IDLE_BEFORE_RELEASE_SECS: f64 = 0.075;

fn cancel_pan_glide(state: &mut InputState) {
    state.pan_velocity = Vec2::ZERO;
    state.pan_velocity_sample_time = None;
}

fn begin_pan(state: &mut InputState, camera: &Camera) {
    cancel_pan_glide(state);
    state.panning = true;
    state.pan_start_screen = state.mouse_pos;
    state.pan_start_world = camera.pan;
    state.pan_velocity_sample_time = Some(miniquad::date::now());
}

fn finalize_pan_glide(state: &mut InputState, zoom: f32) {
    let idle_before_release = state
        .pan_velocity_sample_time
        .map(|last_motion| miniquad::date::now() - last_motion)
        .unwrap_or(f64::INFINITY);
    let launch_speed_screen = state.pan_velocity.length() * zoom;
    if idle_before_release > PAN_GLIDE_MAX_IDLE_BEFORE_RELEASE_SECS
        || launch_speed_screen < PAN_GLIDE_MIN_LAUNCH_SPEED_SCREEN
    {
        state.pan_velocity = Vec2::ZERO;
    } else if launch_speed_screen > PAN_GLIDE_MAX_LAUNCH_SPEED_SCREEN {
        state.pan_velocity = state.pan_velocity.normalize() * (PAN_GLIDE_MAX_LAUNCH_SPEED_SCREEN / zoom);
    }
    state.pan_velocity_sample_time = None;
}

fn begin_transform_drag(
    state: &mut InputState,
    board: &Board,
    drag_mode: DragMode,
    world: Vec2,
) {
    state.pending_drag_mode = DragMode::None;
    state.drag_mode = drag_mode;
    state.move_origin = board
        .elements
        .iter()
        .filter(|element| element.selected)
        .map(|element| (element.id, element.pos, element.size, element.rotation))
        .collect();
    state.move_start_world = world;
    state.move_delta = Vec2::ZERO;
    state.rotate_delta = 0.0;
    state.transform_bounds_origin = current_multi_selection_bounds(state, board).or_else(|| board.selected_bounds());
    state.drag_selection_bounds = state.transform_bounds_origin;
}

fn begin_pending_drag(state: &mut InputState, drag_mode: DragMode, screen: Vec2, world: Vec2) {
    state.pending_drag_mode = drag_mode;
    state.pending_drag_start_screen = screen;
    state.pending_drag_start_world = world;
}

fn clear_pending_drag(state: &mut InputState) {
    state.pending_drag_mode = DragMode::None;
}

fn begin_marquee_drag(state: &mut InputState, world: Vec2) {
    state.pending_drag_mode = DragMode::None;
    state.drag_mode = DragMode::MarqueeSelect;
    state.move_start_world = world;
    state.move_delta = Vec2::ZERO;
    state.marquee_bounds = Some(SelectionBounds::from_points(world, world));
    state.selection_bounds = None;
    state.drag_selection_bounds = None;
    state.transform_bounds_origin = None;
}

fn current_multi_selection_bounds(state: &InputState, board: &Board) -> Option<SelectionBounds> {
    if board.selected_count() <= 1 {
        return None;
    }

    state.selection_bounds.or_else(|| board.selected_bounds())
}

fn sync_multi_selection_bounds(state: &mut InputState, board: &Board) {
    state.selection_bounds = if board.selected_count() > 1 {
        board.selected_bounds()
    } else {
        None
    };
}

fn transform_ids(state: &InputState) -> Vec<u64> {
    state
        .move_origin
        .iter()
        .map(|&(id, _, _, _)| id)
        .collect()
}

fn move_transform_changes(state: &InputState) -> Vec<ElementPropertyChange> {
    state
        .move_origin
        .iter()
        .filter_map(|&(id, orig_pos, orig_size, orig_rot)| {
            let before = ElementTransform::new(orig_pos, orig_size, orig_rot);
            let after = ElementTransform::new(orig_pos + state.move_delta, orig_size, orig_rot);
            (before != after).then_some(ElementPropertyChange {
                id,
                patch: ElementPropertyPatch::Transform { before, after },
            })
        })
        .collect()
}

fn rotation_angle_delta(start_world: Vec2, current_world: Vec2, center: Vec2) -> f32 {
    let start_vec = start_world - center;
    let current_vec = current_world - center;
    current_vec.y.atan2(current_vec.x) - start_vec.y.atan2(start_vec.x)
}

fn selection_handle_hit(state: &InputState, board: &Board, world: Vec2, zoom: f32) -> Option<DragMode> {
    let hit_radius = handle_hit_radius(zoom);
    if board.selected_count() > 1 {
        let bounds = current_multi_selection_bounds(state, board)?;
        for (index, point) in get_selection_bounds_handles(bounds, zoom).iter().enumerate() {
            let delta = world - *point;
            if delta.length_squared() < hit_radius * hit_radius {
                return Some(match index {
                    0 => DragMode::ResizingHandle(HandleDir::TL),
                    1 => DragMode::ResizingHandle(HandleDir::TR),
                    2 => DragMode::ResizingHandle(HandleDir::BR),
                    3 => DragMode::ResizingHandle(HandleDir::BL),
                    4 => DragMode::Rotating,
                    _ => unreachable!(),
                });
            }
        }
        return None;
    }

    for element in board.elements.iter().filter(|element| element.selected).rev() {
        if let Some(handles) = get_element_handles(element, zoom) {
            for (index, point) in handles.iter().enumerate() {
                let delta = world - *point;
                if delta.length_squared() < hit_radius * hit_radius {
                    return Some(if element.shape == ShapeType::Line {
                        match index {
                            0 => DragMode::ResizingHandle(HandleDir::LineStart),
                            1 => DragMode::ResizingHandle(HandleDir::LineEnd),
                            _ => unreachable!(),
                        }
                    } else {
                        match index {
                            0 => DragMode::ResizingHandle(HandleDir::TL),
                            1 => DragMode::ResizingHandle(HandleDir::TR),
                            2 => DragMode::ResizingHandle(HandleDir::BR),
                            3 => DragMode::ResizingHandle(HandleDir::BL),
                            4 => DragMode::Rotating,
                            _ => unreachable!(),
                        }
                    });
                }
            }
        }
    }

    None
}

fn rotate_point(point: Vec2, center: Vec2, angle: f32) -> Vec2 {
    let offset = point - center;
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    center + Vec2::new(
        offset.x * cos_a - offset.y * sin_a,
        offset.x * sin_a + offset.y * cos_a,
    )
}

fn inverse_rotate_point(point: Vec2, center: Vec2, angle: f32) -> Vec2 {
    let offset = point - center;
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    center + Vec2::new(
        offset.x * cos_a + offset.y * sin_a,
        -offset.x * sin_a + offset.y * cos_a,
    )
}

fn scale_point_from_anchor(point: Vec2, anchor: Vec2, scale_x: f32, scale_y: f32) -> Vec2 {
    anchor + Vec2::new((point.x - anchor.x) * scale_x, (point.y - anchor.y) * scale_y)
}

fn rotate_vector(vector: Vec2, angle: f32) -> Vec2 {
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    Vec2::new(
        vector.x * cos_a - vector.y * sin_a,
        vector.x * sin_a + vector.y * cos_a,
    )
}

fn inverse_rotate_vector(vector: Vec2, angle: f32) -> Vec2 {
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    Vec2::new(
        vector.x * cos_a + vector.y * sin_a,
        -vector.x * sin_a + vector.y * cos_a,
    )
}

fn scale_vector_in_frame(vector: Vec2, frame_rotation: f32, scale_x: f32, scale_y: f32) -> Vec2 {
    let local_vector = inverse_rotate_vector(vector, frame_rotation);
    let scaled_local = Vec2::new(local_vector.x * scale_x, local_vector.y * scale_y);
    rotate_vector(scaled_local, frame_rotation)
}

fn scale_point_from_anchor_in_frame(
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

fn resize_rotated_element_in_frame(
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

fn element_corners(element: &Element) -> Vec<Vec2> {
    if element.shape == ShapeType::Line {
        return vec![element.pos, element.pos + element.size];
    }

    let center = element.pos + element.size * 0.5;
    let half_size = element.size * 0.5;
    vec![
        rotate_point(center + Vec2::new(-half_size.x, -half_size.y), center, element.rotation),
        rotate_point(center + Vec2::new(half_size.x, -half_size.y), center, element.rotation),
        rotate_point(center + Vec2::new(half_size.x, half_size.y), center, element.rotation),
        rotate_point(center + Vec2::new(-half_size.x, half_size.y), center, element.rotation),
    ]
}

fn selection_bounds_from_selected_elements_in_frame(
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

fn rect_edge_anchor(norm_pos: Vec2) -> Vec2 {
    let mut snapped = norm_pos.clamp(Vec2::ZERO, Vec2::ONE);
    let dist_left = snapped.x;
    let dist_right = 1.0 - snapped.x;
    let dist_top = snapped.y;
    let dist_bottom = 1.0 - snapped.y;

    let min_x = dist_left.min(dist_right);
    let min_y = dist_top.min(dist_bottom);

    if min_x <= min_y {
        snapped.x = if dist_left <= dist_right { 0.0 } else { 1.0 };
    } else {
        snapped.y = if dist_top <= dist_bottom { 0.0 } else { 1.0 };
    }

    snapped
}

fn ellipse_edge_anchor(norm_pos: Vec2) -> Vec2 {
    let centered = norm_pos * 2.0 - Vec2::ONE;
    if centered.length_squared() <= 0.0001 {
        return Vec2::new(1.0, 0.5);
    }

    let boundary = centered.normalize();
    (boundary + Vec2::ONE) * 0.5
}

fn line_anchor_from_drop(target: &Element, world: Vec2) -> Option<LineAnchor> {
    let norm_pos = match target.shape {
        ShapeType::Rect | ShapeType::Image => rect_edge_anchor(world_to_local_norm(world, target)),
        ShapeType::Ellipse => ellipse_edge_anchor(world_to_local_norm(world, target)),
        ShapeType::Line => return None,
    };

    Some(LineAnchor {
        target_id: target.id,
        norm_pos,
    })
}

fn find_line_anchor(board: &Board, line_id: u64, world: Vec2) -> Option<LineAnchor> {
    board
        .hit_test_all(world)
        .into_iter()
        .filter(|&target_id| target_id != line_id)
        .find_map(|target_id| {
            let target = board.element(target_id)?;
            line_anchor_from_drop(target, world)
        })
}

fn resolved_line_endpoints(board: &Board, line_id: u64) -> Option<LineEndpoints> {
    let line = board.element(line_id)?;
    if line.shape != ShapeType::Line {
        return None;
    }

    Some(LineEndpoints {
        start: find_line_anchor(board, line_id, line.pos),
        end: find_line_anchor(board, line_id, line.pos + line.size),
    })
}

fn line_connection_change_for_handle_release(
    board: &Board,
    line_id: u64,
    dir: HandleDir,
) -> Option<LineConnectionChange> {
    let line = board.element(line_id)?;
    if line.shape != ShapeType::Line {
        return None;
    }

    let before = board
        .line_attachments
        .get(&line_id)
        .cloned()
        .unwrap_or_default();
    let mut after = before.clone();
    let start_world = line.pos;
    let end_world = line.pos + line.size;

    match dir {
        HandleDir::LineStart => after.start = find_line_anchor(board, line_id, start_world),
        HandleDir::LineEnd => after.end = find_line_anchor(board, line_id, end_world),
        _ => return None,
    }

    (after != before).then_some(LineConnectionChange {
        id: line_id,
        before,
        after,
    })
}

fn line_connection_change_for_move_release(
    board: &Board,
    line_id: u64,
) -> Option<LineConnectionChange> {
    let before = board
        .line_attachments
        .get(&line_id)
        .cloned()
        .unwrap_or_default();
    let after = resolved_line_endpoints(board, line_id)?;

    (after != before).then_some(LineConnectionChange {
        id: line_id,
        before,
        after,
    })
}

fn new_line_connections(board: &Board, line_id: u64) -> Option<LineConnectionChange> {
    let after = resolved_line_endpoints(board, line_id)?;

    (!matches!(after, LineEndpoints { start: None, end: None })).then_some(LineConnectionChange {
        id: line_id,
        before: LineEndpoints::default(),
        after,
    })
}

fn resized_selection_bounds(bounds: SelectionBounds, dir: HandleDir, world: Vec2) -> Option<SelectionBounds> {
    let center = bounds.center();
    let local_world = inverse_rotate_point(world, center, bounds.rotation);
    let anchor = match dir {
        HandleDir::TL => bounds.max(),
        HandleDir::TR => Vec2::new(bounds.min().x, bounds.max().y),
        HandleDir::BR => bounds.min(),
        HandleDir::BL => Vec2::new(bounds.max().x, bounds.min().y),
        _ => return None,
    };
    let local_min = local_world.min(anchor);
    let local_max = local_world.max(anchor);
    Some(SelectionBounds {
        pos: local_min,
        size: (local_max - local_min).max(Vec2::splat(1.0)),
        rotation: bounds.rotation,
    })
}

fn group_resize_from_handle(
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
        HandleDir::TL => Some((rotate_point(max, center, bounds.rotation), (max.x - local_world.x) / width, (max.y - local_world.y) / height)),
        HandleDir::TR => Some((rotate_point(Vec2::new(min.x, max.y), center, bounds.rotation), (local_world.x - min.x) / width, (max.y - local_world.y) / height)),
        HandleDir::BR => Some((rotate_point(min, center, bounds.rotation), (local_world.x - min.x) / width, (local_world.y - min.y) / height)),
        HandleDir::BL => Some((rotate_point(Vec2::new(max.x, min.y), center, bounds.rotation), (max.x - local_world.x) / width, (local_world.y - min.y) / height)),
        _ => None,
    }
}

pub fn on_mouse_down(
    state: &mut InputState,
    board: &mut Board,
    camera: &Camera,
    active_tool: Tool,
    screen_size: Vec2,
    x: f32,
    y: f32,
    btn: miniquad::MouseButton,
) -> bool {
    const DOUBLE_CLICK_WINDOW: f64 = 0.4;
    let order_changed = false;

    state.mouse_pos = Vec2::new(x, y);

    match btn {
        miniquad::MouseButton::Left => state.mouse_down_left = true,
        miniquad::MouseButton::Right => state.mouse_down_right = true,
        miniquad::MouseButton::Middle => {
            state.mouse_down_middle = true;
            begin_pan(state, camera);
        }
        _ => {}
    }

    if btn != miniquad::MouseButton::Left {
        return false;
    }

    if state.want_pan() {
        begin_pan(state, camera);
        return false;
    }

    let world = camera.screen_to_world(state.mouse_pos, screen_size);

    match active_tool {
        Tool::Select => {
            let now = miniquad::date::now();
            clear_pending_drag(state);
            if let Some(drag_mode) = selection_handle_hit(state, board, world, camera.zoom) {
                state.active_text_id = None;
                state.text_selecting = false;
                begin_transform_drag(state, board, drag_mode, world);
                return false;
            }

            if board.selected_count() > 1 {
                if let Some(bounds) = current_multi_selection_bounds(state, board) {
                    if bounds.contains(world) {
                        state.active_text_id = None;
                        state.text_selecting = false;
                        begin_pending_drag(state, DragMode::MoveSelected, state.mouse_pos, world);
                        return false;
                    }
                }
            }

            if let Some(id) = board.hit_test(world) {
                if state.active_text_id.is_some() && state.active_text_id != Some(id) {
                    state.active_text_id = None;
                }

                if state.shift_held {
                    state.last_click_id = None;
                    state.last_click_at = None;
                    state.text_selecting = false;
                    board.toggle_selected(id);
                    sync_multi_selection_bounds(state, board);
                    state.drag_selection_bounds = state.selection_bounds;
                    return order_changed;
                }

                let allows_text_edit = board
                    .element(id)
                    .map(|element| element.can_host_text())
                    .unwrap_or(false);

                let is_double_click = allows_text_edit
                    && state.last_click_id == Some(id)
                    && state
                        .last_click_at
                        .map(|last| (now - last) <= DOUBLE_CLICK_WINDOW)
                        .unwrap_or(false);

                state.last_click_id = Some(id);
                state.last_click_at = Some(now);

                let already_selected = board
                    .is_selected(id);
                if !already_selected {
                    board.deselect_all();
                    board.select_only(id);
                    state.selection_bounds = None;
                }

                if is_double_click {
                    state.active_text_id = Some(id);
                    state.text_cursor = board
                        .element(id)
                        .and_then(|element| element.text.as_ref())
                        .map(|text| text.content.chars().count())
                        .unwrap_or(0);
                    state.text_selecting = false;
                    state.drag_mode = DragMode::None;
                    state.move_origin.clear();
                    return order_changed;
                }

                if state.active_text_id == Some(id) {
                    return order_changed;
                }

                begin_pending_drag(state, DragMode::MoveSelected, state.mouse_pos, world);
            } else {
                state.last_click_id = None;
                state.last_click_at = None;
                state.active_text_id = None;
                state.text_selecting = false;
                begin_pending_drag(state, DragMode::MarqueeSelect, state.mouse_pos, world);
            }
        }
        Tool::Rect | Tool::Ellipse | Tool::Line | Tool::Text => {
            clear_pending_drag(state);
            state.active_text_id = None;
            state.text_selecting = false;
            state.dragging_tool = true;
            state.drag_start_world = world;
            state.preview = None;
        }
    }

    order_changed
}

pub fn on_mouse_up(
    state: &mut InputState,
    board: &mut Board,
    camera: &Camera,
    active_tool: Tool,
    screen_size: Vec2,
    x: f32,
    y: f32,
    btn: miniquad::MouseButton,
) -> Option<Tool> {
    let was_panning = state.panning;
    let completed_drag_mode = state.drag_mode;
    state.mouse_pos = Vec2::new(x, y);

    match btn {
        miniquad::MouseButton::Left => state.mouse_down_left = false,
        miniquad::MouseButton::Right => state.mouse_down_right = false,
        miniquad::MouseButton::Middle => {
            state.mouse_down_middle = false;
            state.panning = false;
        }
        _ => {}
    }

    if btn != miniquad::MouseButton::Left {
        if was_panning && !state.panning {
            finalize_pan_glide(state, camera.zoom);
        }
        return None;
    }

    state.text_selecting = false;

    if state.pending_drag_mode != DragMode::None {
        if state.pending_drag_mode == DragMode::MarqueeSelect && !state.shift_held {
            board.deselect_all();
            state.selection_bounds = None;
        }
        clear_pending_drag(state);
    }

    if state.drag_mode != DragMode::None {
        match state.drag_mode {
            DragMode::MarqueeSelect => {
                if let Some(bounds) = state.marquee_bounds.take() {
                    if bounds.size.x >= MARQUEE_MIN_SIZE || bounds.size.y >= MARQUEE_MIN_SIZE {
                        board.select_intersecting_bounds(bounds, state.shift_held);
                        sync_multi_selection_bounds(state, board);
                    } else if !state.shift_held {
                        board.deselect_all();
                        state.selection_bounds = None;
                    }
                } else if !state.shift_held {
                    board.deselect_all();
                    state.selection_bounds = None;
                }
            }
            DragMode::MoveSelected => {
                let changes = if state.move_origin.len() > 1 {
                    move_transform_changes(state)
                } else {
                    board.selected_transform_changes(&state.move_origin)
                };
                if !changes.is_empty() {
                    board.apply_operation(BoardOperation::SetProperty {
                        changes,
                        sync_connected_lines: state.move_origin.len() <= 1,
                    });
                }

                if state.move_origin.len() <= 1 {
                    let line_connection_changes: Vec<LineConnectionChange> = state
                        .move_origin
                        .iter()
                        .map(|&(id, _, _, _)| id)
                        .filter_map(|id| line_connection_change_for_move_release(board, id))
                        .collect();
                    if !line_connection_changes.is_empty() {
                        board.apply_operation(BoardOperation::SetLineConnections {
                            changes: line_connection_changes,
                        });
                    }
                }

                if state.move_origin.len() > 1 {
                    state.selection_bounds = board.selected_bounds();
                }
            }
            DragMode::Rotating if state.move_origin.len() > 1 => {
                if let Some(bounds) = state.transform_bounds_origin {
                    let ids = transform_ids(state);
                    if !ids.is_empty() && state.rotate_delta != 0.0 {
                        board.apply_operation(BoardOperation::RotateElements {
                            ids,
                            center: bounds.center(),
                            angle: state.rotate_delta,
                        });
                    }
                }
                state.selection_bounds = state.drag_selection_bounds;
            }
            _ => {
                let changes = board.selected_transform_changes(&state.move_origin);
                if !changes.is_empty() {
                    board.apply_operation(BoardOperation::SetProperty {
                        changes,
                        sync_connected_lines: true,
                    });
                }
                if state.move_origin.len() > 1 {
                    state.selection_bounds = state.drag_selection_bounds;
                }
            }
        }

        if state.move_origin.len() == 1 {
            if let DragMode::ResizingHandle(dir @ (HandleDir::LineStart | HandleDir::LineEnd)) =
                completed_drag_mode
            {
                let line_id = state.move_origin[0].0;
                if let Some(change) = line_connection_change_for_handle_release(board, line_id, dir) {
                    board.apply_operation(BoardOperation::SetLineConnections {
                        changes: vec![change],
                    });
                }
            }
        }
    }
    state.drag_mode = DragMode::None;
    state.move_delta = Vec2::ZERO;
    state.rotate_delta = 0.0;
    state.move_origin.clear();
    state.marquee_bounds = None;
    state.drag_selection_bounds = None;
    state.transform_bounds_origin = None;

    if state.dragging_tool {
        state.dragging_tool = false;
        if let Some(prev) = state.preview.take() {
            let min_size = 4.0f32;
            if prev.size.x.abs() >= min_size || prev.size.y.abs() >= min_size {
                let mut element = prev;
                if element.shape != ShapeType::Line {
                    if element.size.x < 0.0 {
                        element.pos.x += element.size.x;
                        element.size.x = -element.size.x;
                    }
                    if element.size.y < 0.0 {
                        element.pos.y += element.size.y;
                        element.size.y = -element.size.y;
                    }
                }
                let _ = camera;
                let _ = screen_size;
                let new_id = board.next_id();
                element.id = new_id;
                if false {
                    element.text = Some(crate::board::TextData {
                        content: String::new(),
                        font_size: 24.0,
                        color: crate::board::DEFAULT_TEXT_COLOR,
                    });
                }
                board.apply_operation(BoardOperation::AddElement(element));
                board.deselect_all();
                board.select_only(new_id);
                if let Some(change) = new_line_connections(board, new_id) {
                    board.apply_operation(BoardOperation::SetLineConnections {
                        changes: vec![change],
                    });
                }
                if matches!(active_tool, Tool::Text) {
                    state.active_text_id = Some(new_id);
                    state.text_cursor = 0;
                }
                return Some(Tool::Select);
            }
        }
    }

    if btn == miniquad::MouseButton::Left && !state.mouse_down_middle {
        state.panning = false;
    }

    if was_panning && !state.panning {
        finalize_pan_glide(state, camera.zoom);
    }

    None
}

pub fn on_mouse_move(
    state: &mut InputState,
    board: &mut Board,
    preview_visible_ids: Option<&std::collections::HashSet<u64>>,
    camera: &mut Camera,
    tool_style_defaults: &ToolStyleDefaults,
    active_tool: Tool,
    screen_size: Vec2,
    x: f32,
    y: f32,
) {
    let prev = state.mouse_pos;
    state.mouse_pos = Vec2::new(x, y);
    let delta_screen = state.mouse_pos - prev;

    if state.panning {
        let pan_delta = -delta_screen / camera.zoom;
        camera.pan += pan_delta;

        if pan_delta.length_squared() == 0.0 {
            return;
        }

        let now = miniquad::date::now();
        if let Some(last_sample_time) = state.pan_velocity_sample_time {
            let dt = (now - last_sample_time) as f32;
            if dt > 0.0 {
                let instant_velocity = pan_delta / dt;
                state.pan_velocity = if state.pan_velocity.length_squared() > 0.0 {
                    state.pan_velocity.lerp(instant_velocity, PAN_VELOCITY_SMOOTHING)
                } else {
                    instant_velocity
                };
            }
        }
        state.pan_velocity_sample_time = Some(now);
        return;
    }

    let world = camera.screen_to_world(state.mouse_pos, screen_size);

    if state.pending_drag_mode != DragMode::None {
        let pending_delta = state.mouse_pos - state.pending_drag_start_screen;
        if pending_delta.length_squared() >= DRAG_START_DISTANCE * DRAG_START_DISTANCE {
            match state.pending_drag_mode {
                DragMode::MoveSelected => {
                    begin_transform_drag(state, board, DragMode::MoveSelected, state.pending_drag_start_world);
                }
                DragMode::MarqueeSelect => {
                    begin_marquee_drag(state, state.pending_drag_start_world);
                }
                DragMode::ResizingHandle(_) | DragMode::Rotating | DragMode::None => {
                    clear_pending_drag(state);
                }
            }
        }
    }

    if state.drag_mode != DragMode::None {
        if state.drag_mode == DragMode::MarqueeSelect {
            state.marquee_bounds = Some(SelectionBounds::from_points(state.move_start_world, world));
            return;
        }

        state.move_delta = world - state.move_start_world;
        state.rotate_delta = 0.0;
        if state.drag_mode == DragMode::MoveSelected {
            if state.move_origin.len() <= 1 {
                let moving_ids = transform_ids(state);

                for &(id, orig_pos, orig_size, orig_rot) in &state.move_origin {
                    if let Some(element) = board.element_mut(id) {
                        element.pos = orig_pos + state.move_delta;
                        element.size = orig_size;
                        element.rotation = orig_rot;
                    }
                }

                board.update_connected_lines_for_targets_filtered(moving_ids, preview_visible_ids);
                state.drag_selection_bounds = None;
            } else {
                state.drag_selection_bounds = state
                    .transform_bounds_origin
                    .map(|bounds| bounds.with_position(bounds.pos + state.move_delta));
            }
            return;
        }

        let is_group_transform = state.move_origin.len() > 1;

        if state.drag_mode == DragMode::Rotating && is_group_transform {
            let Some(bounds) = state.transform_bounds_origin else {
                return;
            };
            state.rotate_delta = rotation_angle_delta(state.move_start_world, world, bounds.center());
            state.drag_selection_bounds = Some(bounds.with_rotation(bounds.rotation + state.rotate_delta));
            return;
        }

        for element in &mut board.elements {
            if element.selected {
                if let Some(&(_, orig_pos, orig_size, orig_rot)) = state
                    .move_origin
                    .iter()
                    .find(|&&(id, _, _, _)| id == element.id)
                {
                    match state.drag_mode {
                        DragMode::Rotating => {
                            if !is_group_transform {
                                let center = orig_pos + orig_size * 0.5;
                                let angle_diff = rotation_angle_delta(state.move_start_world, world, center);
                                element.rotation = orig_rot + angle_diff;
                            }
                        }
                        DragMode::ResizingHandle(dir) => {
                            if is_group_transform {
                                let Some(bounds) = state.transform_bounds_origin else {
                                    continue;
                                };
                                let Some((anchor, scale_x, scale_y)) =
                                    group_resize_from_handle(bounds, dir, world)
                                else {
                                    continue;
                                };

                                if element.shape == ShapeType::Line {
                                    let start = scale_point_from_anchor_in_frame(
                                        orig_pos,
                                        anchor,
                                        scale_x,
                                        scale_y,
                                        bounds.center(),
                                        bounds.rotation,
                                    );
                                    let end = scale_point_from_anchor_in_frame(
                                        orig_pos + orig_size,
                                        anchor,
                                        scale_x,
                                        scale_y,
                                        bounds.center(),
                                        bounds.rotation,
                                    );
                                    element.pos = start;
                                    element.size = end - start;
                                } else {
                                    resize_rotated_element_in_frame(
                                        element,
                                        orig_pos,
                                        orig_size,
                                        orig_rot,
                                        anchor,
                                        scale_x,
                                        scale_y,
                                        bounds,
                                    );
                                }
                            } else if element.shape == ShapeType::Line {
                                match dir {
                                    HandleDir::LineStart => {
                                        let old_end = orig_pos + orig_size;
                                        element.pos = orig_pos + state.move_delta;
                                        element.size = old_end - element.pos;
                                    }
                                    HandleDir::LineEnd => {
                                        element.size = orig_size + state.move_delta;
                                    }
                                    _ => {}
                                }
                            } else {
                                let c = orig_rot.cos();
                                let s = orig_rot.sin();
                                let dx = state.move_delta.x;
                                let dy = state.move_delta.y;
                                let l_dx = dx * c + dy * s;
                                let l_dy = -dx * s + dy * c;

                                let mut new_pos = orig_pos;
                                let mut new_size = orig_size;

                                match dir {
                                    HandleDir::TL => {
                                        new_pos += Vec2::new(l_dx, l_dy);
                                        new_size -= Vec2::new(l_dx, l_dy);
                                    }
                                    HandleDir::TR => {
                                        new_pos.y += l_dy;
                                        new_size.x += l_dx;
                                        new_size.y -= l_dy;
                                    }
                                    HandleDir::BL => {
                                        new_pos.x += l_dx;
                                        new_size.x -= l_dx;
                                        new_size.y += l_dy;
                                    }
                                    HandleDir::BR => {
                                        new_size += Vec2::new(l_dx, l_dy);
                                    }
                                    _ => {}
                                }

                                let local_center = new_pos + new_size * 0.5;
                                let orig_local_center = orig_pos + orig_size * 0.5;
                                let d_cx = local_center.x - orig_local_center.x;
                                let d_cy = local_center.y - orig_local_center.y;
                                let w_dcx = d_cx * c - d_cy * s;
                                let w_dcy = d_cx * s + d_cy * c;
                                let w_center =
                                    orig_pos + orig_size * 0.5 + Vec2::new(w_dcx, w_dcy);

                                element.size = new_size;
                                element.pos = w_center - new_size * 0.5;
                            }
                            
                            let now = miniquad::date::now();
                            if now - state.last_resize_text_bump > COMPUTE_TEXT_LAYOUT_DEBOUNCE {
                                element.bump_text_generation();
                                state.last_resize_text_bump = now;
                            }
                        }
                        DragMode::MoveSelected | DragMode::MarqueeSelect | DragMode::None => {}
                    }
                }
            }
        }
        if is_group_transform {
            if let DragMode::ResizingHandle(dir) = state.drag_mode {
                if let Some(bounds) = state.transform_bounds_origin {
                    state.drag_selection_bounds = selection_bounds_from_selected_elements_in_frame(
                        board,
                        bounds.rotation,
                    )
                    .or_else(|| resized_selection_bounds(bounds, dir, world));
                }
            }
        } else {
            state.drag_selection_bounds = None;
        }
        return;
    }

    if state.dragging_tool {
        let start = state.drag_start_world;
        let current = world;

        let shape = match active_tool {
            Tool::Rect => ShapeType::Rect,
            Tool::Ellipse => ShapeType::Ellipse,
            Tool::Line => ShapeType::Line,
            Tool::Text => ShapeType::Rect,
            Tool::Select => return,
        };

        let (pos, size) = match shape {
            ShapeType::Line => (start, current - start),
            _ => (
                Vec2::new(start.x.min(current.x), start.y.min(current.y)),
                Vec2::new((current.x - start.x).abs(), (current.y - start.y).abs()),
            ),
        };

        state.preview = Some(Element {
            id: 0,
            shape,
            pos,
            size,
            rotation: 0.0,
            color: preview_fill_color(&active_tool, tool_style_defaults),
            stroke_color: preview_stroke_color(&active_tool, tool_style_defaults),
            border_width: preview_border_width(&active_tool, tool_style_defaults),
            stroke_width: preview_line_stroke_width(&active_tool, tool_style_defaults),
            selected: false,
            text: if matches!(shape, ShapeType::Rect | ShapeType::Ellipse) {
                Some(crate::board::TextData {
                    content: String::new(),
                    font_size: 24.0,
                    color: preview_text_color(&active_tool, tool_style_defaults),
                })
            } else {
                None
            },
            image: None,
            text_layout_generation: 0,
        });
    }
}

fn preview_fill_color(tool: &Tool, defaults: &ToolStyleDefaults) -> [f32; 4] {
    match tool {
        Tool::Rect => defaults.rect.fill_color,
        Tool::Ellipse => defaults.ellipse.fill_color,
        Tool::Text => defaults.text.fill_color,
        Tool::Line => defaults.line.color,
        _ => crate::palette::PURE_BLACK,
    }
}

fn preview_stroke_color(tool: &Tool, defaults: &ToolStyleDefaults) -> [f32; 4] {
    match tool {
        Tool::Rect => defaults.rect.stroke_color,
        Tool::Ellipse => defaults.ellipse.stroke_color,
        Tool::Text => defaults.text.stroke_color,
        Tool::Line => defaults.line.color,
        _ => crate::board::DEFAULT_STROKE_COLOR,
    }
}

fn preview_border_width(tool: &Tool, defaults: &ToolStyleDefaults) -> u8 {
    match tool {
        Tool::Rect => defaults.rect.border_width,
        Tool::Ellipse => defaults.ellipse.border_width,
        Tool::Text => defaults.text.border_width,
        _ => crate::board::DEFAULT_BORDER_WIDTH,
    }
}

fn preview_line_stroke_width(tool: &Tool, defaults: &ToolStyleDefaults) -> u8 {
    match tool {
        Tool::Line => defaults.line.stroke_width,
        _ => crate::board::DEFAULT_LINE_STROKE_WIDTH,
    }
}

fn preview_text_color(tool: &Tool, defaults: &ToolStyleDefaults) -> [f32; 4] {
    match tool {
        Tool::Rect => defaults.rect.text_color,
        Tool::Ellipse => defaults.ellipse.text_color,
        Tool::Text => defaults.text.text_color,
        _ => crate::board::DEFAULT_TEXT_COLOR,
    }
}