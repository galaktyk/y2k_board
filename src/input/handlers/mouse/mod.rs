use glam::Vec2;

use crate::board::{
    line_curve_handle_offset_from_handle, world_to_local_norm, Board, BoardOperation, Element,
    ElementKind, ElementPropertyChange, ElementPropertyPatch, ElementTransform, LineAnchor,
    LineConnectionChange, LineEndpoints, ShapeType, ToolStyleDefaults,
};
use crate::camera::Camera;
use crate::input::handles::{
    edge_handle_hit, element_resize_bounds, get_connection_helpers, get_element_handles,
    get_selection_bounds_handles, handle_hit_radius,
};
use crate::input::state::{ConnectionDrag, DragMode, HandleDir, InputState, SelectionBounds};
use crate::spatial::SpatialGrid;
use crate::ui::tool::Tool;

pub mod pan;
pub mod selection;
pub mod transform;
pub mod connection;
pub mod tools;

pub use pan::{cancel_pan_glide, begin_pan, finalize_pan_glide, PAN_VELOCITY_SMOOTHING};
pub use selection::{begin_marquee_drag, current_multi_selection_bounds, sync_multi_selection_bounds, selection_handle_hit, selection_edge_hit, MARQUEE_MIN_SIZE, DRAG_START_DISTANCE};
pub use transform::{begin_transform_drag, transform_ids, move_transform_changes, rotation_angle_delta, rotate_point, inverse_rotate_point, scale_point_from_anchor, rotate_vector, inverse_rotate_vector, scale_vector_in_frame, scale_point_from_anchor_in_frame, resize_rotated_element_in_frame, element_corners, selection_bounds_from_selected_elements_in_frame, resized_selection_bounds, group_resize_from_handle};
pub use connection::{begin_connection_drag, connection_helper_hit, find_line_anchor, anchored_position_from_element, anchored_position_from_anchor, resolved_line_endpoints, line_connection_change_for_handle_release, line_connection_change_for_move_release, new_line_connections, snap_radius};
pub use tools::{sticky_note_element, STICKY_NOTE_SIZE};

pub const COMPUTE_TEXT_LAYOUT_DEBOUNCE: f64 = 0.10;

fn begin_pending_drag(state: &mut InputState, drag_mode: DragMode, screen: Vec2, world: Vec2) {
    state.pending_drag_mode = drag_mode;
    state.pending_drag_start_screen = screen;
    state.pending_drag_start_world = world;
}

fn clear_pending_drag(state: &mut InputState) {
    state.pending_drag_mode = DragMode::None;
}


fn resize_cursor(dir: HandleDir) -> miniquad::CursorIcon {
    match dir {
        HandleDir::TL | HandleDir::BR => miniquad::CursorIcon::NWSEResize,
        HandleDir::TR | HandleDir::BL => miniquad::CursorIcon::NESWResize,
        HandleDir::Left | HandleDir::Right => miniquad::CursorIcon::EWResize,
        HandleDir::Top | HandleDir::Bottom => miniquad::CursorIcon::NSResize,
        HandleDir::LineStart | HandleDir::LineEnd | HandleDir::LineCurve => {
            miniquad::CursorIcon::Default
        }
    }
}

pub fn hover_cursor(
    state: &InputState,
    board: &Board,
    camera: &Camera,
    active_tool: Tool,
    screen_size: Vec2,
) -> miniquad::CursorIcon {
    if active_tool != Tool::Select {
        return miniquad::CursorIcon::Default;
    }

    if let DragMode::ResizingHandle(dir) = state.drag_mode {
        return resize_cursor(dir);
    }

    let world = camera.screen_to_world(state.mouse_pos, screen_size);

    if let Some(DragMode::ResizingHandle(dir)) =
        selection_handle_hit(state, board, world, camera.zoom)
    {
        return resize_cursor(dir);
    }

    if let Some(DragMode::ResizingHandle(dir)) =
        selection_edge_hit(state, board, world, camera.zoom)
    {
        return resize_cursor(dir);
    }

    miniquad::CursorIcon::Default
}

pub fn on_mouse_down(
    state: &mut InputState,
    board: &mut Board,
    spatial: &SpatialGrid,
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
            if let Some(connection_drag) = connection_helper_hit(board, world, camera.zoom) {
                state.active_text_id = None;
                state.text_selecting = false;
                begin_connection_drag(state, connection_drag);
                return false;
            }
            if let Some(drag_mode) = selection_handle_hit(state, board, world, camera.zoom) {
                state.active_text_id = None;
                state.text_selecting = false;
                begin_transform_drag(state, board, drag_mode, world);
                return false;
            }
            if let Some(drag_mode) = selection_edge_hit(state, board, world, camera.zoom) {
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

            let candidate_ids = spatial.query(world, world);
            if let Some(id) = board.hit_test_filtered(world, Some(&candidate_ids)) {
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

                let already_selected = board.is_selected(id);
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
        Tool::Sticky => {
            clear_pending_drag(state);
            state.active_text_id = None;
            state.text_selecting = false;
            state.dragging_tool = true;
            state.drag_start_world = world;
            state.preview = None;
            state.last_click_id = None;
            state.last_click_at = None;
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
    spatial: &SpatialGrid,
    camera: &Camera,
    tool_style_defaults: &ToolStyleDefaults,
    active_tool: Tool,
    screen_size: Vec2,
    x: f32,
    y: f32,
    btn: miniquad::MouseButton,
) -> Option<Tool> {
    let was_panning = state.panning;
    let completed_drag_mode = state.drag_mode;
    state.mouse_pos = Vec2::new(x, y);
    let world = camera.screen_to_world(state.mouse_pos, screen_size);

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
                        let candidate_ids = spatial.query(bounds.min(), bounds.max());
                        board.select_intersecting_bounds_filtered(
                            bounds,
                            state.shift_held,
                            Some(&candidate_ids),
                        );
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
                let changes = move_transform_changes(state);
                if !changes.is_empty() {
                    board.apply_operation(BoardOperation::SetProperty {
                        changes,
                        sync_connected_lines: true,
                    });
                }

                if state.move_origin.len() <= 1 {
                    let line_connection_changes: Vec<LineConnectionChange> = state
                        .move_origin
                        .iter()
                        .map(|&(id, _, _, _, _, _)| id)
                        .filter_map(|id| {
                            line_connection_change_for_move_release(
                                board,
                                spatial,
                                id,
                                camera.zoom,
                                state.ctrl_held,
                            )
                        })
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
            DragMode::CreatingConnection => {
                if let Some(connection_drag) = state.connection_drag {
                    let snap_radius = snap_radius(camera.zoom);
                    let end_anchor = if state.ctrl_held {
                        None
                    } else {
                        find_line_anchor(
                            board,
                            spatial,
                            u64::MAX,
                            world,
                            connection_drag.start_world,
                            snap_radius,
                        )
                    };
                    let end_world = end_anchor
                        .as_ref()
                        .and_then(|anchor| anchored_position_from_anchor(board, anchor))
                        .unwrap_or(connection_drag.end_world);

                    if (end_world - connection_drag.start_world).length_squared()
                        >= MARQUEE_MIN_SIZE * MARQUEE_MIN_SIZE
                    {
                        let new_id = board.next_id();
                        board.apply_operation(BoardOperation::AddElement(Element {
                            id: new_id,
                            shape: ShapeType::Line,
                            kind: ElementKind::Generic,
                            pos: connection_drag.start_world,
                            size: end_world - connection_drag.start_world,
                            rotation: 0.0,
                            color: tool_style_defaults.line.color,
                            stroke_color: tool_style_defaults.line.color,
                            border_width: crate::board::default_border_width(),
                            stroke_width: tool_style_defaults.line.stroke_width,
                            line_arrow_start: tool_style_defaults.line.arrow_start,
                            line_arrow_end: tool_style_defaults.line.arrow_end,
                            line_bend: 0.0,
                            line_midpoint_shift: 0.0,
                            line_start_normal: None,
                            line_end_normal: None,
                            selected: false,
                            text: None,
                            image: None,
                            text_layout_generation: 0,
                        }));
                        board.deselect_all();
                        board.select_only(new_id);
                        board.apply_operation(BoardOperation::SetLineConnections {
                            changes: vec![connection_line_change(
                                new_id,
                                &connection_drag,
                                end_anchor,
                            )],
                        });
                        state.selection_bounds = None;
                    }
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
                let mut changes = board.selected_transform_changes(&state.move_origin);
                changes.extend(board.selected_line_curve_changes(&state.move_origin));
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
                if let Some(change) = line_connection_change_for_handle_release(
                    board,
                    spatial,
                    line_id,
                    dir,
                    camera.zoom,
                    state.ctrl_held,
                ) {
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
    state.connection_drag = None;
    state.move_origin.clear();
    state.marquee_bounds = None;
    state.drag_selection_bounds = None;
    state.transform_bounds_origin = None;

    if active_tool == Tool::Sticky {
        let should_create = state.dragging_tool && !was_panning;
        state.dragging_tool = false;
        state.preview = None;
        if should_create {
            let new_id = board.next_id();
            let mut element = sticky_note_element(tool_style_defaults, state.drag_start_world);
            element.id = new_id;
            board.apply_operation(BoardOperation::AddElement(element));
            board.deselect_all();
            board.select_only(new_id);
            state.selection_bounds = None;
            state.active_text_id = Some(new_id);
            state.text_cursor = 0;
            return Some(Tool::Select);
        }
    }

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
                if let Some(change) =
                    new_line_connections(board, spatial, new_id, camera.zoom, state.ctrl_held)
                {
                    board.apply_operation(BoardOperation::SetLineConnections {
                        changes: vec![change],
                    });
                }
                if matches!(active_tool, Tool::Rect | Tool::Text) {
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
    spatial: &SpatialGrid,
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
        if state.touchpad_mode {
            // In touchpad mode, pan-drag (Space+Drag) becomes zoom
            let factor = if delta_screen.y < 0.0 {
                1.05f32
            } else {
                1.0 / 1.05
            };
            camera.zoom_toward(state.mouse_pos, screen_size, factor);
            return;
        }

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
                    state
                        .pan_velocity
                        .lerp(instant_velocity, PAN_VELOCITY_SMOOTHING)
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
                    begin_transform_drag(
                        state,
                        board,
                        DragMode::MoveSelected,
                        state.pending_drag_start_world,
                    );
                }
                DragMode::MarqueeSelect => {
                    begin_marquee_drag(state, state.pending_drag_start_world);
                }
                DragMode::ResizingHandle(_)
                | DragMode::Rotating
                | DragMode::CreatingConnection
                | DragMode::None => {
                    clear_pending_drag(state);
                }
            }
        }
    }

    if state.drag_mode != DragMode::None {
        if state.drag_mode == DragMode::MarqueeSelect {
            state.marquee_bounds =
                Some(SelectionBounds::from_points(state.move_start_world, world));
            return;
        }

        if state.drag_mode == DragMode::CreatingConnection {
            if let Some(connection_drag) = state.connection_drag.as_mut() {
                let snap_radius = snap_radius(camera.zoom);
                let anchor = if state.ctrl_held {
                    None
                } else {
                    find_line_anchor(
                        board,
                        spatial,
                        u64::MAX,
                        world,
                        connection_drag.start_world,
                        snap_radius,
                    )
                };

                if let Some(anchor) = anchor {
                    connection_drag.end_world =
                        anchored_position_from_anchor(board, &anchor).unwrap_or(world);
                } else {
                    connection_drag.end_world = world;
                }
            }
            return;
        }

        state.move_delta = world - state.move_start_world;
        state.rotate_delta = 0.0;
        if state.drag_mode == DragMode::MoveSelected {
            if state.move_origin.len() > 1 {
                state.drag_selection_bounds = state
                    .transform_bounds_origin
                    .map(|bounds| bounds.with_position(bounds.pos + state.move_delta));
            } else {
                state.drag_selection_bounds = None;
            }
            return;
        }

        let is_group_transform = state.move_origin.len() > 1;

        if state.drag_mode == DragMode::Rotating && is_group_transform {
            let Some(bounds) = state.transform_bounds_origin else {
                return;
            };
            state.rotate_delta =
                rotation_angle_delta(state.move_start_world, world, bounds.center());
            state.drag_selection_bounds =
                Some(bounds.with_rotation(bounds.rotation + state.rotate_delta));
            return;
        }

        let snap_radius = snap_radius(camera.zoom);
        let origins = state.move_origin.clone();
        for (id, orig_pos, orig_size, orig_rot, _orig_bend, _orig_midpoint_shift) in origins {
            if !board.element(id).map(|e| e.selected).unwrap_or(false) {
                continue;
            }

            match state.drag_mode {
                DragMode::Rotating => {
                    if !is_group_transform {
                        let center = orig_pos + orig_size * 0.5;
                        let angle_diff =
                            rotation_angle_delta(state.move_start_world, world, center);
                        let element = board.element_mut(id).unwrap();
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

                        let element = board.element_mut(id).unwrap();
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
                                element, orig_pos, orig_size, orig_rot, anchor, scale_x, scale_y,
                                bounds,
                            );
                        }
                    } else {
                        let element_shape = board.element(id).unwrap().shape;
                        if element_shape == ShapeType::Line {
                            match dir {
                                HandleDir::LineStart => {
                                    let old_end = orig_pos + orig_size;
                                    let target_pos = orig_pos + state.move_delta;

                                    let snapped_pos = if state.ctrl_held {
                                        None
                                    } else {
                                        find_line_anchor(
                                            board,
                                            spatial,
                                            id,
                                            target_pos,
                                            old_end,
                                            snap_radius,
                                        )
                                        .and_then(
                                            |anchor| anchored_position_from_anchor(board, &anchor),
                                        )
                                    };

                                    let element = board.element_mut(id).unwrap();
                                    element.pos = snapped_pos.unwrap_or(target_pos);
                                    element.size = old_end - element.pos;
                                }
                                HandleDir::LineEnd => {
                                    let target_end = orig_pos + orig_size + state.move_delta;

                                    let snapped_end = if state.ctrl_held {
                                        None
                                    } else {
                                        find_line_anchor(
                                            board,
                                            spatial,
                                            id,
                                            target_end,
                                            orig_pos,
                                            snap_radius,
                                        )
                                        .and_then(
                                            |anchor| anchored_position_from_anchor(board, &anchor),
                                        )
                                    };

                                    let element = board.element_mut(id).unwrap();
                                    element.size = snapped_end.unwrap_or(target_end) - element.pos;
                                }
                                HandleDir::LineCurve => {
                                    let element = board.element_mut(id).unwrap();
                                    let curve_offset =
                                        line_curve_handle_offset_from_handle(element, world);
                                    element.line_midpoint_shift = curve_offset.x;
                                    element.line_bend = curve_offset.y;
                                }
                                _ => {}
                            }
                        } else {
                            let element = board.element_mut(id).unwrap();
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
                                HandleDir::Top => {
                                    new_pos.y += l_dy;
                                    new_size.y -= l_dy;
                                }
                                HandleDir::Right => {
                                    new_size.x += l_dx;
                                }
                                HandleDir::Bottom => {
                                    new_size.y += l_dy;
                                }
                                HandleDir::Left => {
                                    new_pos.x += l_dx;
                                    new_size.x -= l_dx;
                                }
                                _ => {}
                            }

                            let local_center = new_pos + new_size * 0.5;
                            let orig_local_center = orig_pos + orig_size * 0.5;
                            let d_cx = local_center.x - orig_local_center.x;
                            let d_cy = local_center.y - orig_local_center.y;
                            let w_dcx = d_cx * c - d_cy * s;
                            let w_dcy = d_cx * s + d_cy * c;
                            let w_center = orig_pos + orig_size * 0.5 + Vec2::new(w_dcx, w_dcy);

                            element.size = new_size;
                            element.pos = w_center - new_size * 0.5;
                        }
                    }
                    if board.element(id).unwrap().text.is_some() {
                        state.enqueue_resize_text_recompute(id);
                    }
                }
                DragMode::MoveSelected => {
                    let element = board.element_mut(id).unwrap();
                    element.pos = orig_pos + state.move_delta;
                }
                _ => {}
            }
        }
        if is_group_transform {
            if let DragMode::ResizingHandle(dir) = state.drag_mode {
                if let Some(bounds) = state.transform_bounds_origin {
                    state.drag_selection_bounds =
                        selection_bounds_from_selected_elements_in_frame(board, bounds.rotation)
                            .or_else(|| resized_selection_bounds(bounds, dir, world));
                }
            }
        } else {
            state.drag_selection_bounds = None;
        }
        return;
    }

    if state.dragging_tool {
        if active_tool == Tool::Sticky {
            state.preview = None;
            return;
        }

        let start = state.drag_start_world;
        let current = world;

        let shape = match active_tool {
            Tool::Rect => ShapeType::Rect,
            Tool::Ellipse => ShapeType::Ellipse,
            Tool::Line => ShapeType::Line,
            Tool::Text => ShapeType::Rect,
            Tool::Sticky => return,
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
            kind: ElementKind::Generic,
            pos,
            size,
            rotation: 0.0,
            color: preview_fill_color(&active_tool, tool_style_defaults),
            stroke_color: preview_stroke_color(&active_tool, tool_style_defaults),
            border_width: preview_border_width(&active_tool, tool_style_defaults),
            stroke_width: preview_line_stroke_width(&active_tool, tool_style_defaults),
            line_arrow_start: preview_line_arrow_start(&active_tool, tool_style_defaults),
            line_arrow_end: preview_line_arrow_end(&active_tool, tool_style_defaults),
            line_bend: 0.0,
            line_midpoint_shift: 0.0,
            line_start_normal: None,
            line_end_normal: None,
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
        Tool::Sticky => defaults.sticky.fill_color,
        Tool::Text => defaults.text.fill_color,
        Tool::Line => defaults.line.color,
        _ => crate::palette::PURE_BLACK,
    }
}

fn preview_stroke_color(tool: &Tool, defaults: &ToolStyleDefaults) -> [f32; 4] {
    match tool {
        Tool::Rect => defaults.rect.stroke_color,
        Tool::Ellipse => defaults.ellipse.stroke_color,
        Tool::Sticky => defaults.sticky.stroke_color,
        Tool::Text => defaults.text.stroke_color,
        Tool::Line => defaults.line.color,
        _ => crate::board::DEFAULT_STROKE_COLOR,
    }
}

fn preview_border_width(tool: &Tool, defaults: &ToolStyleDefaults) -> u8 {
    match tool {
        Tool::Rect => defaults.rect.border_width,
        Tool::Ellipse => defaults.ellipse.border_width,
        Tool::Sticky => defaults.sticky.border_width,
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

fn preview_line_arrow_start(tool: &Tool, defaults: &ToolStyleDefaults) -> bool {
    match tool {
        Tool::Line => defaults.line.arrow_start,
        _ => false,
    }
}

fn preview_line_arrow_end(tool: &Tool, defaults: &ToolStyleDefaults) -> bool {
    match tool {
        Tool::Line => defaults.line.arrow_end,
        _ => false,
    }
}

fn preview_text_color(tool: &Tool, defaults: &ToolStyleDefaults) -> [f32; 4] {
    match tool {
        Tool::Rect => defaults.rect.text_color,
        Tool::Ellipse => defaults.ellipse.text_color,
        Tool::Sticky => defaults.sticky.text_color,
        Tool::Text => defaults.text.text_color,
        _ => crate::board::DEFAULT_TEXT_COLOR,
    }
}

fn connection_line_change(
    line_id: u64,
    source: &ConnectionDrag,
    end: Option<LineAnchor>,
) -> LineConnectionChange {
    LineConnectionChange {
        id: line_id,
        before: LineEndpoints::default(),
        after: LineEndpoints {
            start: Some(LineAnchor {
                target_id: source.source_id,
                norm_pos: source.source_norm_pos,
            }),
            end,
        },
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec2;

    use super::*;
    use crate::spatial::SpatialGrid;

    fn rect_target(id: u64, pos: Vec2, size: Vec2) -> Element {
        Element {
            id,
            shape: ShapeType::Rect,
            kind: ElementKind::Generic,
            pos,
            size,
            rotation: 0.0,
            color: [1.0, 0.0, 0.0, 1.0],
            stroke_color: crate::board::default_stroke_color(),
            border_width: crate::board::default_border_width(),
            stroke_width: crate::board::default_line_stroke_width(),
            line_arrow_start: false,
            line_arrow_end: false,
            line_bend: 0.0,
            line_midpoint_shift: 0.0,
            line_start_normal: None,
            line_end_normal: None,
            selected: false,
            text: None,
            image: None,
            text_layout_generation: 0,
        }
    }

    #[test]
    fn line_hook_accepts_drop_inside_target_on_nearest_face() {
        let target = rect_target(1, Vec2::ZERO, Vec2::new(100.0, 80.0));

        let anchor =
            line_anchor_from_drop(&target, Vec2::new(75.0, 40.0), Vec2::new(-40.0, 40.0))
                .unwrap();

        assert_eq!(anchor.norm_pos, Vec2::new(1.0, 0.5));
    }

    #[test]
    fn line_hook_uses_nearest_face() {
        let target = rect_target(1, Vec2::ZERO, Vec2::new(100.0, 80.0));

        let anchor =
            line_anchor_from_drop(&target, Vec2::new(-4.0, 40.0), Vec2::new(-80.0, 40.0)).unwrap();

        assert_eq!(anchor.norm_pos, Vec2::new(0.0, 0.5));
    }

    #[test]
    fn line_hook_allows_opposite_facing_right_edge() {
        let target = rect_target(1, Vec2::ZERO, Vec2::new(100.0, 80.0));

        let anchor =
            line_anchor_from_drop(&target, Vec2::new(104.0, 40.0), Vec2::new(-80.0, 40.0))
                .unwrap();

        assert_eq!(anchor.norm_pos, Vec2::new(1.0, 0.5));
    }

    #[test]
    fn find_line_anchor_accepts_cursor_deep_inside_target() {
        let target = rect_target(1, Vec2::ZERO, Vec2::new(100.0, 80.0));
        let mut board = Board::new();
        board.elements.push(target.clone());

        let (aabb_min, aabb_max) = target.aabb();
        let mut spatial = SpatialGrid::new();
        spatial.insert(target.id, aabb_min, aabb_max);

        let anchor = find_line_anchor(
            &board,
            &spatial,
            u64::MAX,
            Vec2::new(75.0, 40.0),
            Vec2::new(-40.0, 40.0),
            12.0,
        )
        .unwrap();

        assert_eq!(anchor.norm_pos, Vec2::new(1.0, 0.5));
    }

    #[test]
    fn find_line_anchor_accepts_opposite_facing_right_edge() {
        let target = rect_target(1, Vec2::ZERO, Vec2::new(100.0, 80.0));
        let mut board = Board::new();
        board.elements.push(target.clone());

        let (aabb_min, aabb_max) = target.aabb();
        let mut spatial = SpatialGrid::new();
        spatial.insert(target.id, aabb_min, aabb_max);

        let anchor = find_line_anchor(
            &board,
            &spatial,
            u64::MAX,
            Vec2::new(104.0, 40.0),
            Vec2::new(-80.0, 40.0),
            12.0,
        )
        .unwrap();

        assert_eq!(anchor.norm_pos, Vec2::new(1.0, 0.5));
    }
}
