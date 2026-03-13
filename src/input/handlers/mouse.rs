use glam::Vec2;
use std::time::{Duration, Instant};

use crate::board::{Board, BoardOperation, Element, ShapeType};
use crate::camera::Camera;
use crate::input::handles::get_element_handles;
use crate::input::preview::default_color;
use crate::input::state::{DragMode, HandleDir, InputState};
use crate::toolbar::{Tool, Toolbar, ToolbarAction, TOOLBAR_HEIGHT};

pub fn on_mouse_down(
    state: &mut InputState,
    board: &mut Board,
    camera: &Camera,
    toolbar: &mut Toolbar,
    screen_size: Vec2,
    x: f32,
    y: f32,
    btn: miniquad::MouseButton,
) -> Option<ToolbarAction> {
    const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(400);

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
        return None;
    }

    if y < TOOLBAR_HEIGHT {
        if let Some(action) = toolbar.hit_test(x, y) {
            return Some(action);
        }
        return None;
    }

    if state.want_pan() {
        state.panning = true;
        state.pan_start_screen = state.mouse_pos;
        state.pan_start_world = camera.pan;
        return None;
    }

    let world = camera.screen_to_world(state.mouse_pos, screen_size);

    match toolbar.active_tool {
        Tool::Select => {
            let now = Instant::now();
            let mut handle_hit = None;
            for e in board.elements.iter().filter(|e| e.selected).rev() {
                if let Some(handles) = get_element_handles(e) {
                    let hit_radius = 15.0f32;
                    for (index, &pt) in handles.iter().enumerate() {
                        let dx = world.x - pt.x;
                        let dy = world.y - pt.y;
                        if dx * dx + dy * dy < hit_radius * hit_radius {
                            handle_hit = Some((e.id, index));
                            break;
                        }
                    }
                }
                if handle_hit.is_some() {
                    break;
                }
            }

            if let Some((id, handle_index)) = handle_hit {
                let element = board.elements.iter().find(|e| e.id == id).unwrap();
                state.drag_mode = if element.shape == ShapeType::Line {
                    match handle_index {
                        0 => DragMode::ResizingHandle(HandleDir::LineStart),
                        1 => DragMode::ResizingHandle(HandleDir::LineEnd),
                        _ => unreachable!(),
                    }
                } else {
                    match handle_index {
                        0 => DragMode::ResizingHandle(HandleDir::TL),
                        1 => DragMode::ResizingHandle(HandleDir::TR),
                        2 => DragMode::ResizingHandle(HandleDir::BR),
                        3 => DragMode::ResizingHandle(HandleDir::BL),
                        4 => DragMode::Rotating,
                        _ => unreachable!(),
                    }
                };
                state.move_origin = board
                    .elements
                    .iter()
                    .filter(|e| e.selected)
                    .map(|e| (e.id, e.pos, e.size, e.rotation))
                    .collect();
                state.move_start_world = world;
                state.move_delta = Vec2::ZERO;
                return None;
            }

            if let Some(id) = board.hit_test(world) {
                if state.active_text_id.is_some() && state.active_text_id != Some(id) {
                    state.active_text_id = None;
                }

                let is_double_click = state.last_click_id == Some(id)
                    && state
                        .last_click_at
                        .map(|last| now.duration_since(last) <= DOUBLE_CLICK_WINDOW)
                        .unwrap_or(false);

                state.last_click_id = Some(id);
                state.last_click_at = Some(now);

                let already_selected = board
                    .elements
                    .iter()
                    .find(|e| e.id == id)
                    .map(|e| e.selected)
                    .unwrap_or(false);
                if !already_selected {
                    board.deselect_all();
                    board.select_only(id);
                }

                if is_double_click {
                    if board
                        .element(id)
                        .map(|element| element.can_host_text())
                        .unwrap_or(false)
                    {
                        state.active_text_id = Some(id);
                        state.text_cursor = board
                            .element(id)
                            .and_then(|element| element.text.as_ref())
                            .map(|text| text.content.chars().count())
                            .unwrap_or(0);
                        state.text_selecting = false;
                    }
                    state.drag_mode = DragMode::None;
                    state.move_origin.clear();
                    return None;
                }

                if state.active_text_id == Some(id) {
                    return None;
                }

                state.move_origin = board
                    .elements
                    .iter()
                    .filter(|e| e.selected)
                    .map(|e| (e.id, e.pos, e.size, e.rotation))
                    .collect();
                state.drag_mode = DragMode::MoveSelected;
                state.move_start_world = world;
                state.move_delta = Vec2::ZERO;
            } else {
                state.last_click_id = None;
                state.last_click_at = None;
                state.active_text_id = None;
                state.text_selecting = false;
                board.deselect_all();
            }
        }
        Tool::Rect | Tool::Ellipse | Tool::Line | Tool::Text => {
            state.active_text_id = None;
            state.text_selecting = false;
            state.dragging_tool = true;
            state.drag_start_world = world;
            state.preview = None;
        }
    }

    None
}

pub fn on_mouse_up(
    state: &mut InputState,
    board: &mut Board,
    camera: &Camera,
    toolbar: &mut Toolbar,
    screen_size: Vec2,
    x: f32,
    y: f32,
    btn: miniquad::MouseButton,
) {
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
        return;
    }

    state.text_selecting = false;

    if state.drag_mode != DragMode::None {
        let changes = board.selected_transform_changes(&state.move_origin);
        if !changes.is_empty() {
            board.apply_operation(BoardOperation::SetProperty { changes });
        }
    }
    state.drag_mode = DragMode::None;
    state.move_delta = Vec2::ZERO;
    state.move_origin.clear();

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
                element.id = board.next_id();
                if element.shape == ShapeType::Text {
                    element.text = Some(crate::board::TextData {
                        content: String::new(),
                        font_size: 24.0,
                        color: [1.0, 1.0, 1.0, 1.0],
                    });
                }
                board.apply_operation(BoardOperation::AddElement(element));
                if matches!(toolbar.active_tool, Tool::Text) {
                    state.active_text_id = Some(board.next_available_id().saturating_sub(1));
                    state.text_cursor = 0;
                }
                toolbar.active_tool = Tool::Select;
            }
        }
    }

    if btn == miniquad::MouseButton::Left && !state.mouse_down_middle {
        state.panning = false;
    }
}

pub fn on_mouse_move(
    state: &mut InputState,
    board: &mut Board,
    camera: &mut Camera,
    toolbar: &Toolbar,
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

    if state.drag_mode != DragMode::None {
        state.move_delta = world - state.move_start_world;
        for element in &mut board.elements {
            if element.selected {
                if let Some(&(_, orig_pos, orig_size, orig_rot)) = state
                    .move_origin
                    .iter()
                    .find(|&&(id, _, _, _)| id == element.id)
                {
                    match state.drag_mode {
                        DragMode::MoveSelected => {
                            element.pos = orig_pos + state.move_delta;
                        }
                        DragMode::Rotating => {
                            let center = orig_pos + orig_size * 0.5;
                            let start_vec = state.move_start_world - center;
                            let current_vec = world - center;
                            let angle_diff =
                                current_vec.y.atan2(current_vec.x) - start_vec.y.atan2(start_vec.x);
                            element.rotation = orig_rot + angle_diff;
                        }
                        DragMode::ResizingHandle(dir) => {
                            if element.shape == ShapeType::Line {
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
                        }
                        DragMode::None => {}
                    }
                }
            }
        }
        return;
    }

    if state.dragging_tool {
        let start = state.drag_start_world;
        let current = world;

        let shape = match toolbar.active_tool {
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
                    color: [1.0, 1.0, 1.0, 1.0],
                })
            } else {
                None
            },
        });
    }
}