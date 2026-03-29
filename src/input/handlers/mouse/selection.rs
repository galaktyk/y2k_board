use glam::Vec2;
use crate::board::{Board, ShapeType};
use crate::input::state::{DragMode, HandleDir, InputState, SelectionBounds};
use crate::input::handles::{
    edge_handle_hit, element_resize_bounds, get_element_handles, get_selection_bounds_handles,
    handle_hit_radius,
};

pub const MARQUEE_MIN_SIZE: f32 = 4.0;
pub const DRAG_START_DISTANCE: f32 = 3.0;

pub fn begin_marquee_drag(state: &mut InputState, world: Vec2) {
    state.pending_drag_mode = DragMode::None;
    state.drag_mode = DragMode::MarqueeSelect;
    state.move_start_world = world;
    state.move_delta = Vec2::ZERO;
    state.marquee_bounds = Some(SelectionBounds::from_points(world, world));
    state.selection_bounds = None;
    state.drag_selection_bounds = None;
    state.transform_bounds_origin = None;
}

pub fn current_multi_selection_bounds(state: &InputState, board: &Board) -> Option<SelectionBounds> {
    if board.selected_count() <= 1 {
        return None;
    }

    state.selection_bounds.or_else(|| board.selected_bounds())
}

pub fn sync_multi_selection_bounds(state: &mut InputState, board: &Board) {
    state.selection_bounds = if board.selected_count() > 1 {
        board.selected_bounds()
    } else {
        None
    };
}

pub fn selection_handle_hit(
    state: &InputState,
    board: &Board,
    world: Vec2,
    zoom: f32,
) -> Option<DragMode> {
    let hit_radius = handle_hit_radius(zoom);
    if board.selected_count() > 1 {
        let bounds = current_multi_selection_bounds(state, board)?;
        for (index, point) in get_selection_bounds_handles(bounds, zoom)
            .iter()
            .enumerate()
        {
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

    for element in board
        .elements
        .iter()
        .filter(|element| element.selected)
        .rev()
    {
        if let Some(handles) = get_element_handles(element, zoom) {
            for (index, point) in handles.iter().enumerate() {
                let delta = world - *point;
                if delta.length_squared() < hit_radius * hit_radius {
                    return Some(if element.shape == ShapeType::Line {
                        match index {
                            0 => DragMode::ResizingHandle(HandleDir::LineStart),
                            1 => DragMode::ResizingHandle(HandleDir::LineEnd),
                            2 => DragMode::ResizingHandle(HandleDir::LineCurve),
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

pub fn selection_edge_hit(
    state: &InputState,
    board: &Board,
    world: Vec2,
    zoom: f32,
) -> Option<DragMode> {
    if board.selected_count() > 1 {
        let bounds = current_multi_selection_bounds(state, board)?;
        return edge_handle_hit(bounds, world, zoom).map(DragMode::ResizingHandle);
    }

    for element in board
        .elements
        .iter()
        .filter(|element| element.selected)
        .rev()
    {
        let Some(bounds) = element_resize_bounds(element) else {
            continue;
        };

        if let Some(dir) = edge_handle_hit(bounds, world, zoom) {
            return Some(DragMode::ResizingHandle(dir));
        }
    }

    None
}
