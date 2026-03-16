use glam::Vec2;


use crate::board::{
    Board, BoardOperation, Element, ElementPropertyChange, ElementPropertyPatch, ElementTransform,
    ShapeType, DEFAULT_TEXT_COLOR,
};
use crate::camera::Camera;
use crate::input::handles::{get_element_handles, get_selection_bounds_handles, handle_hit_radius};
use crate::input::preview::default_color;
use crate::input::state::{DragMode, HandleDir, InputState, SelectionBounds};
use crate::tool::Tool;

const MARQUEE_MIN_SIZE: f32 = 4.0;
const DRAG_START_DISTANCE: f32 = 3.0;

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

fn move_changes_from_delta(state: &InputState) -> Vec<ElementPropertyChange> {
    state
        .move_origin
        .iter()
        .filter_map(|&(id, pos, size, rotation)| {
            let before = ElementTransform::new(pos, size, rotation);
            let after = ElementTransform::new(pos + state.move_delta, size, rotation);
            (before != after).then_some(ElementPropertyChange {
                id,
                patch: ElementPropertyPatch::Transform { before, after },
            })
        })
        .collect()
}

fn transform_ids(state: &InputState) -> Vec<u64> {
    state
        .move_origin
        .iter()
        .map(|&(id, _, _, _)| id)
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
    let mut order_changed = false;

    state.mouse_pos = Vec2::new(x, y);

    match btn {
        miniquad::MouseButton::Left => state.mouse_down_left = true,
        miniquad::MouseButton::Right => state.mouse_down_right = true,
        miniquad::MouseButton::Middle => {
            state.mouse_down_middle = true;
            state.panning = true;
            state.pan_start_screen = state.mouse_pos;
            state.pan_start_world = camera.pan;
        }
        _ => {}
    }

    if btn != miniquad::MouseButton::Left {
        return false;
    }

    if state.want_pan() {
        state.panning = true;
        state.pan_start_screen = state.mouse_pos;
        state.pan_start_world = camera.pan;
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
                    if board.is_selected(id) {
                        order_changed = board.bring_shape_to_front(id);
                    }
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
                    order_changed = board.bring_shape_to_front(id);
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
                if state.move_origin.len() > 1 {
                    let ids = transform_ids(state);
                    if !ids.is_empty() && state.move_delta != Vec2::ZERO {
                        board.apply_operation(BoardOperation::MoveElements {
                            ids,
                            delta: state.move_delta,
                        });
                    }
                } else {
                    let changes = move_changes_from_delta(state);
                    if !changes.is_empty() {
                        board.apply_operation(BoardOperation::SetProperty { changes });
                    }
                }
                if state.move_origin.len() > 1 {
                    state.selection_bounds = state.drag_selection_bounds;
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
                    board.apply_operation(BoardOperation::SetProperty { changes });
                }
                if state.move_origin.len() > 1 {
                    state.selection_bounds = state.drag_selection_bounds;
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
                if element.shape == ShapeType::Text {
                    element.text = Some(crate::board::TextData {
                        content: String::new(),
                        font_size: 24.0,
                        color: DEFAULT_TEXT_COLOR,
                    });
                }
                board.apply_operation(BoardOperation::AddElement(element));
                board.deselect_all();
                board.select_only(new_id);
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

    None
}

pub fn on_mouse_move(
    state: &mut InputState,
    board: &mut Board,
    camera: &mut Camera,
    active_tool: Tool,
    screen_size: Vec2,
    x: f32,
    y: f32,
) {
    let prev = state.mouse_pos;
    state.mouse_pos = Vec2::new(x, y);
    let delta_screen = state.mouse_pos - prev;

    if state.panning {
        camera.pan -= delta_screen / camera.zoom;
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
            state.drag_selection_bounds = state
                .transform_bounds_origin
                .map(|bounds| bounds.with_rotation(bounds.rotation).with_position(bounds.pos + state.move_delta));
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
                            element.bump_text_generation();
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

                                state.drag_selection_bounds = resized_selection_bounds(bounds, dir, world);

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
                                    let original_center = orig_pos + orig_size * 0.5;
                                    let scaled_center = scale_point_from_anchor_in_frame(
                                        original_center,
                                        anchor,
                                        scale_x,
                                        scale_y,
                                        bounds.center(),
                                        bounds.rotation,
                                    );
                                    let new_size = Vec2::new(
                                        orig_size.x * scale_x.abs(),
                                        orig_size.y * scale_y.abs(),
                                    )
                                    .max(Vec2::splat(1.0));
                                    element.pos = scaled_center - new_size * 0.5;
                                    element.size = new_size;
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
                            element.bump_text_generation();
                        }
                        DragMode::MoveSelected | DragMode::MarqueeSelect | DragMode::None => {}
                    }
                }
            }
        }
        if !is_group_transform {
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
            Tool::Text => ShapeType::Text,
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
            color: default_color(shape),
            selected: false,
            text: if shape == ShapeType::Text {
                Some(crate::board::TextData {
                    content: String::new(),
                    font_size: 24.0,
                    color: DEFAULT_TEXT_COLOR,
                })
            } else {
                None
            },
            image: None,
            text_layout_generation: 0,
        });
    }
}