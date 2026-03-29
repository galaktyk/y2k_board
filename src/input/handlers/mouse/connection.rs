use glam::Vec2;
use crate::board::{
    world_to_local_norm, Board, Element, LineAnchor, LineConnectionChange,
    LineEndpoints, ShapeType,
};
use crate::input::state::{ConnectionDrag, DragMode, HandleDir, InputState};
use crate::spatial::SpatialGrid;
use crate::input::handles::{get_connection_helpers, handle_hit_radius};

pub const SNAP_RADIUS_PX: f32 = 12.0;

pub fn world_units_per_screen_px(zoom: f32) -> f32 {
    1.0 / zoom.max(0.0001)
}

pub fn snap_radius(zoom: f32) -> f32 {
    SNAP_RADIUS_PX * world_units_per_screen_px(zoom)
}

pub fn begin_connection_drag(state: &mut InputState, connection_drag: ConnectionDrag) {
    state.pending_drag_mode = DragMode::None;
    state.drag_mode = DragMode::CreatingConnection;
    state.move_origin.clear();
    state.move_delta = Vec2::ZERO;
    state.rotate_delta = 0.0;
    state.marquee_bounds = None;
    state.drag_selection_bounds = None;
    state.transform_bounds_origin = None;
    state.connection_drag = Some(connection_drag);
}

pub fn connection_helper_hit(board: &Board, world: Vec2, zoom: f32) -> Option<ConnectionDrag> {
    if board.selected_count() != 1 {
        return None;
    }

    let hit_radius = handle_hit_radius(zoom);
    for element in board
        .elements
        .iter()
        .filter(|element| element.selected)
        .rev()
    {
        let Some(helpers) = get_connection_helpers(element, zoom) else {
            continue;
        };

        for helper in helpers {
            let delta = world - helper.point;
            if delta.length_squared() < hit_radius * hit_radius {
                let start_world = anchored_position_from_element(element, helper.norm_pos);
                return Some(ConnectionDrag {
                    source_id: element.id,
                    source_norm_pos: helper.norm_pos,
                    start_world,
                    end_world: start_world,
                });
            }
        }
    }

    None
}

pub fn ellipse_edge_anchor(norm_pos: Vec2) -> Vec2 {
    let centered = norm_pos * 2.0 - Vec2::ONE;
    if centered.length_squared() <= 0.0001 {
        return Vec2::new(1.0, 0.5);
    }

    let boundary = centered.normalize();
    (boundary + Vec2::ONE) * 0.5
}

pub fn point_inside_hook_target(target: &Element, world: Vec2) -> bool {
    let norm = world_to_local_norm(world, target);

    match target.shape {
        ShapeType::Rect | ShapeType::Image => {
            norm.x >= 0.0 && norm.x <= 1.0 && norm.y >= 0.0 && norm.y <= 1.0
        }
        ShapeType::Ellipse => {
            let centered = norm * 2.0 - Vec2::ONE;
            centered.length_squared() <= 1.0
        }
        ShapeType::Line => false,
    }
}

pub fn line_anchor_from_drop(
    target: &Element,
    world: Vec2,
    _reference_world: Vec2,
) -> Option<LineAnchor> {
    let mut best: Option<(f32, Vec2)> = None;

    match target.shape {
        ShapeType::Rect | ShapeType::Image => {
            let local = world_to_local_norm(world, target).clamp(Vec2::ZERO, Vec2::ONE);
            let candidates = [
                Vec2::new(0.0, local.y),
                Vec2::new(1.0, local.y),
                Vec2::new(local.x, 0.0),
                Vec2::new(local.x, 1.0),
            ];

            for norm_pos in candidates {
                let distance = (anchored_position_from_element(target, norm_pos) - world).length();

                match best {
                    Some((best_distance, _)) if distance >= best_distance => {}
                    _ => best = Some((distance, norm_pos)),
                }
            }
        }
        ShapeType::Ellipse => {
            let candidates = [
                ellipse_edge_anchor(world_to_local_norm(world, target)),
                ellipse_edge_anchor(world_to_local_norm(_reference_world, target)),
            ];

            for norm_pos in candidates {
                let distance = (anchored_position_from_element(target, norm_pos) - world).length();

                match best {
                    Some((best_distance, _)) if distance >= best_distance => {}
                    _ => best = Some((distance, norm_pos)),
                }
            }
        }
        ShapeType::Line => return None,
    }

    best.map(|(_, norm_pos)| LineAnchor {
        target_id: target.id,
        norm_pos,
    })
}

pub fn anchored_position_from_element(element: &Element, norm_pos: Vec2) -> Vec2 {
    let center = element.pos + element.size * 0.5;
    let local = (norm_pos - Vec2::splat(0.5)) * element.size;
    let c = element.rotation.cos();
    let s = element.rotation.sin();
    center + Vec2::new(local.x * c - local.y * s, local.x * s + local.y * c)
}

pub fn anchored_position_from_anchor(board: &Board, anchor: &LineAnchor) -> Option<Vec2> {
    let element = board.element(anchor.target_id)?;
    Some(anchored_position_from_element(element, anchor.norm_pos))
}

pub fn find_line_anchor(
    board: &Board,
    spatial: &SpatialGrid,
    line_id: u64,
    world: Vec2,
    reference_world: Vec2,
    snap_radius: f32,
) -> Option<LineAnchor> {
    let query_min = world - Vec2::splat(snap_radius);
    let query_max = world + Vec2::splat(snap_radius);
    let candidate_ids = spatial.query(query_min, query_max);

    let mut best_anchor = None;
    let mut min_dist = snap_radius;

    let ordered_indices = board.ordered_candidate_indices(Some(&candidate_ids));

    for &index in ordered_indices.iter().rev() {
        let target = &board.elements[index];
        if target.id == line_id || target.shape == ShapeType::Line {
            continue;
        }

        if let Some(norm_pos) = line_anchor_from_drop(target, world, reference_world) {
            let dist = if point_inside_hook_target(target, world) {
                0.0
            } else {
                (anchored_position_from_element(target, norm_pos.norm_pos) - world).length()
            };
            if dist < min_dist {
                min_dist = dist;
                best_anchor = Some(norm_pos);
            }
        }
    }

    best_anchor
}

pub fn resolved_line_endpoints(
    board: &Board,
    spatial: &SpatialGrid,
    line_id: u64,
    zoom: f32,
    ctrl_held: bool,
) -> Option<LineEndpoints> {
    let line = board.element(line_id)?;
    if line.shape != ShapeType::Line {
        return None;
    }

    if ctrl_held {
        return Some(LineEndpoints::default());
    }

    let snap_radius = snap_radius(zoom);

    Some(LineEndpoints {
        start: find_line_anchor(
            board,
            spatial,
            line_id,
            line.pos,
            line.pos + line.size,
            snap_radius,
        ),
        end: find_line_anchor(
            board,
            spatial,
            line_id,
            line.pos + line.size,
            line.pos,
            snap_radius,
        ),
    })
}

pub fn line_connection_change_for_handle_release(
    board: &Board,
    spatial: &SpatialGrid,
    line_id: u64,
    dir: HandleDir,
    zoom: f32,
    ctrl_held: bool,
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

    let snap_radius = snap_radius(zoom);

    match dir {
        HandleDir::LineStart => {
            after.start = if ctrl_held {
                None
            } else {
                find_line_anchor(board, spatial, line_id, start_world, end_world, snap_radius)
            }
        }
        HandleDir::LineEnd => {
            after.end = if ctrl_held {
                None
            } else {
                find_line_anchor(board, spatial, line_id, end_world, start_world, snap_radius)
            }
        }
        _ => return None,
    }

    (after != before).then_some(LineConnectionChange {
        id: line_id,
        before,
        after,
    })
}

pub fn line_connection_change_for_move_release(
    board: &Board,
    spatial: &SpatialGrid,
    line_id: u64,
    zoom: f32,
    ctrl_held: bool,
) -> Option<LineConnectionChange> {
    let before = board
        .line_attachments
        .get(&line_id)
        .cloned()
        .unwrap_or_default();
    let after = resolved_line_endpoints(board, spatial, line_id, zoom, ctrl_held)?;

    (after != before).then_some(LineConnectionChange {
        id: line_id,
        before,
        after,
    })
}

pub fn new_line_connections(
    board: &Board,
    spatial: &SpatialGrid,
    line_id: u64,
    zoom: f32,
    ctrl_held: bool,
) -> Option<LineConnectionChange> {
    let after = resolved_line_endpoints(board, spatial, line_id, zoom, ctrl_held)?;

    (!matches!(
        after,
        LineEndpoints {
            start: None,
            end: None
        }
    ))
    .then_some(LineConnectionChange {
        id: line_id,
        before: LineEndpoints::default(),
        after,
    })
}
