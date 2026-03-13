use glam::Vec2;
use crate::board::{Board, Element, Op, ShapeType};
use crate::camera::Camera;
use crate::toolbar::{Tool, Toolbar, ToolbarAction, TOOLBAR_HEIGHT};
use crate::renderer::InstanceData;

pub struct InputState {
    pub mouse_pos: Vec2,
    mouse_down_left: bool,
    mouse_down_right: bool,
    mouse_down_middle: bool,
    pub space_held: bool,
    #[allow(dead_code)]
    ctrl_held: bool,

    // Camera pan state
    panning: bool,
    pan_start_screen: Vec2,
    pan_start_world: Vec2,

    // Tool drag state
    dragging_tool: bool,
    drag_start_world: Vec2,

    // Element dragging state
    drag_mode: DragMode,
    move_start_world: Vec2,
    /// Element positions at the start of the gesture
    move_origin: Vec<(u64, Vec2, Vec2, f32)>,

    /// Transient preview element shown while dragging a create tool.
    pub preview: Option<Element>,
    /// Live drag delta for selected elements (shown before commit).
    pub move_delta: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HandleDir {
    TL,
    TR,
    BR,
    BL,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DragMode {
    None,
    MoveSelected,
    ResizingHandle(HandleDir),
    Rotating,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            mouse_pos: Vec2::ZERO,
            mouse_down_left: false,
            mouse_down_right: false,
            mouse_down_middle: false,
            space_held: false,
            ctrl_held: false,
            panning: false,
            pan_start_screen: Vec2::ZERO,
            pan_start_world: Vec2::ZERO,
            dragging_tool: false,
            drag_start_world: Vec2::ZERO,
            drag_mode: DragMode::None,
            move_start_world: Vec2::ZERO,
            move_origin: Vec::new(),
            preview: None,
            move_delta: Vec2::ZERO,
        }
    }

    fn want_pan(&self) -> bool {
        self.space_held || self.mouse_down_middle
    }
}

// ── Public event handlers called from App ────────────────────────────────────

pub fn on_mouse_down(
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
        miniquad::MouseButton::Left   => state.mouse_down_left   = true,
        miniquad::MouseButton::Right  => state.mouse_down_right  = true,
        miniquad::MouseButton::Middle => { state.mouse_down_middle = true; state.panning = true; state.pan_start_screen = state.mouse_pos; state.pan_start_world = camera.pan; }
        _ => {}
    }

    if btn != miniquad::MouseButton::Left {
        return;
    }

    // ── Toolbar hit ────────────────────────────────────────────────────────
    if y < TOOLBAR_HEIGHT {
        if let Some(action) = toolbar.hit_test(x, y) {
            match action {
                ToolbarAction::SetTool(t) => toolbar.active_tool = t,
                ToolbarAction::Undo       => board.undo(),
                ToolbarAction::Redo       => board.redo(),
            }
        }
        return;
    }

    // ── Canvas area ────────────────────────────────────────────────────────
    if state.want_pan() {
        state.panning = true;
        state.pan_start_screen = state.mouse_pos;
        state.pan_start_world = camera.pan;
        return;
    }

    let world = camera.screen_to_world(state.mouse_pos, screen_size);

    match toolbar.active_tool {
        Tool::Select => {
            // Check handles first
            let mut handle_hit = None;
            for e in board.elements.iter().filter(|e| e.selected).rev() {
                if let Some(handles) = get_element_handles(e) {
                    let hit_radius = 15.0f32; // a bit larger than 5.0 for easier grabbing
                    for (i, &pt) in handles.iter().enumerate() {
                        let dx = world.x - pt.x;
                        let dy = world.y - pt.y;
                        if dx*dx + dy*dy < hit_radius*hit_radius {
                            handle_hit = Some((e.id, i));
                            break;
                        }
                    }
                }
                if handle_hit.is_some() { break; }
            }

            if let Some((_id, h_idx)) = handle_hit {
                state.drag_mode = match h_idx {
                    0 => DragMode::ResizingHandle(HandleDir::TL),
                    1 => DragMode::ResizingHandle(HandleDir::TR),
                    2 => DragMode::ResizingHandle(HandleDir::BR),
                    3 => DragMode::ResizingHandle(HandleDir::BL),
                    4 => DragMode::Rotating,
                    _ => unreachable!(),
                };
                state.move_origin = board.elements.iter()
                    .filter(|e| e.selected)
                    .map(|e| (e.id, e.pos, e.size, e.rotation))
                    .collect();
                state.move_start_world = world;
                state.move_delta = Vec2::ZERO;
                return;
            }

            if let Some(id) = board.hit_test(world) {
                let already_selected = board.elements.iter().find(|e| e.id == id).map(|e| e.selected).unwrap_or(false);
                if !already_selected {
                    board.deselect_all();
                    board.select_only(id);
                }
                // Track positions for a potential move
                state.move_origin = board.elements.iter()
                    .filter(|e| e.selected)
                    .map(|e| (e.id, e.pos, e.size, e.rotation))
                    .collect();
                state.drag_mode = DragMode::MoveSelected;
                state.move_start_world = world;
                state.move_delta = Vec2::ZERO;
            } else {
                board.deselect_all();
            }
        }
        Tool::Rect | Tool::Ellipse | Tool::Line => {
            state.dragging_tool = true;
            state.drag_start_world = world;
            state.preview = None;
        }
    }
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
        miniquad::MouseButton::Left   => state.mouse_down_left   = false,
        miniquad::MouseButton::Right  => state.mouse_down_right  = false,
        miniquad::MouseButton::Middle => { state.mouse_down_middle = false; state.panning = false; }
        _ => {}
    }

    if btn != miniquad::MouseButton::Left {
        return;
    }

    // Commit move, resize, rotate
    if state.drag_mode != DragMode::None {
        let mut updates = Vec::new();
        for e in &board.elements {
            if e.selected {
                if let Some(&(id, old_pos, old_size, old_rot)) = state.move_origin.iter().find(|&&(id, _, _, _)| id == e.id) {
                    if e.pos != old_pos || e.size != old_size || e.rotation != old_rot {
                        updates.push((id, (old_pos, old_size, old_rot), (e.pos, e.size, e.rotation)));
                    }
                }
            }
        }
        if !updates.is_empty() {
            board.apply_op(Op::UpdateElements { updates });
        }
    }
    state.drag_mode = DragMode::None;
    state.move_delta = Vec2::ZERO;
    state.move_origin.clear();

    // Commit shape creation
    if state.dragging_tool {
        state.dragging_tool = false;
        if let Some(prev) = state.preview.take() {
            let min_size = 4.0f32;
            if prev.size.x.abs() >= min_size || prev.size.y.abs() >= min_size {
                let mut elem = prev;
                // Normalise so size is always positive, except for lines
                if elem.shape != ShapeType::Line {
                    if elem.size.x < 0.0 {
                        elem.pos.x += elem.size.x;
                        elem.size.x = -elem.size.x;
                    }
                    if elem.size.y < 0.0 {
                        elem.pos.y += elem.size.y;
                        elem.size.y = -elem.size.y;
                    }
                }
                let _ = camera; let _ = screen_size;
                elem.id = board.next_id();
                board.apply_op(Op::AddElement(elem));
                // Auto-switch back to Select tool after creating a shape
                toolbar.active_tool = Tool::Select;
            }
        }
    }

    // End pan
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

    // Camera pan
    if state.panning {
        camera.pan -= delta_screen / camera.zoom;
        return;
    }

    let world = camera.screen_to_world(state.mouse_pos, screen_size);

    // Live update elements
    if state.drag_mode != DragMode::None {
        state.move_delta = world - state.move_start_world;
        for e in &mut board.elements {
            if e.selected {
                if let Some(&(_, orig_pos, orig_size, orig_rot)) = state.move_origin.iter().find(|&&(id, _, _, _)| id == e.id) {
                    match state.drag_mode {
                        DragMode::MoveSelected => {
                            e.pos = orig_pos + state.move_delta;
                        }
                        DragMode::Rotating => {
                            let center = orig_pos + orig_size * 0.5;
                            let start_vec = state.move_start_world - center;
                            let current_vec = world - center;
                            let angle_diff = current_vec.y.atan2(current_vec.x) - start_vec.y.atan2(start_vec.x);
                            e.rotation = orig_rot + angle_diff;
                        }
                        DragMode::ResizingHandle(dir) => {
                            // First map world diff back into local unrotated space
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
                            }

                            // Keep pos in world space: the new_pos we computed is as if the top-left moved in local space 
                            // *relative to the original rotation*.
                            // It's actually easier to compute the new local center, and rotate it into world space.
                            
                            let local_center = new_pos + new_size * 0.5;
                            let orig_local_center = orig_pos + orig_size * 0.5;
                            let d_cx = local_center.x - orig_local_center.x;
                            let d_cy = local_center.y - orig_local_center.y;
                            
                            let w_dcx = d_cx * c - d_cy * s;
                            let w_dcy = d_cx * s + d_cy * c;
                            
                            let w_center = orig_pos + orig_size * 0.5 + Vec2::new(w_dcx, w_dcy);
                            
                            e.size = new_size;
                            e.pos = w_center - new_size * 0.5;
                        }
                        DragMode::None => {}
                    }
                }
            }
        }
        return;
    }

    // Preview shape creation
    if state.dragging_tool {
        let start = state.drag_start_world;
        let current = world;

        let shape = match toolbar.active_tool {
            Tool::Rect    => ShapeType::Rect,
            Tool::Ellipse => ShapeType::Ellipse,
            Tool::Line    => ShapeType::Line,
            Tool::Select  => return,
        };

        let (pos, size) = match shape {
            ShapeType::Line => (start, current - start),
            _ => (
                Vec2::new(start.x.min(current.x), start.y.min(current.y)),
                Vec2::new((current.x - start.x).abs(), (current.y - start.y).abs()),
            ),
        };

        let id = 0; // preview id, never committed
        state.preview = Some(Element {
            id,
            shape,
            pos,
            size,
            rotation: 0.0,
            color: default_color(shape),
            selected: false,
        });
    }
}

pub fn on_scroll(
    state: &mut InputState,
    camera: &mut Camera,
    screen_size: Vec2,
    _dx: f32,
    dy: f32,
) {
    let factor = if dy > 0.0 { 1.1f32 } else { 1.0 / 1.1 };
    camera.zoom_toward(state.mouse_pos, screen_size, factor);
}

pub fn on_key_down(
    _state: &mut InputState,
    board: &mut Board,
    keycode: miniquad::KeyCode,
    modifiers: miniquad::KeyMods,
) {
    match keycode {
        miniquad::KeyCode::Z if modifiers.ctrl => {
            if modifiers.shift {
                board.redo();
            } else {
                board.undo();
            }
        }
        miniquad::KeyCode::Y if modifiers.ctrl => {
            board.redo();
        }
        miniquad::KeyCode::Delete | miniquad::KeyCode::Backspace => {
            board.delete_selected();
        }
        miniquad::KeyCode::Space => {}  // handled via key_down_event on App
        _ => {}
    }
}

fn default_color(shape: ShapeType) -> [f32; 4] {
    match shape {
        ShapeType::Rect    => [0.30, 0.56, 0.90, 0.85],
        ShapeType::Ellipse => [0.34, 0.80, 0.65, 0.85],
        ShapeType::Line    => [0.95, 0.75, 0.30, 1.00],
    }
}

pub fn get_element_handles(e: &Element) -> Option<[Vec2; 5]> {
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
    let th = -hh - 30.0;

    Some([
        rot(-hw, -hh), // TL
        rot(hw, -hh),  // TR
        rot(hw, hh),   // BR
        rot(-hw, hh),  // BL
        rot(0.0, th),  // Top-mid rotation
    ])
}

pub fn handles_to_instances(e: &Element) -> Vec<InstanceData> {
    let mut out = Vec::new();
    let handles = match get_element_handles(e) {
        Some(h) => h,
        None => return out,
    };

    let center = e.pos + e.size * 0.5;
    let c = e.rotation.cos();
    let s = e.rotation.sin();
    let rot = |rx: f32, ry: f32| -> Vec2 {
        center + Vec2::new(rx * c - ry * s, rx * s + ry * c)
    };
    
    let stick_center = rot(0.0, -e.size.y * 0.5 - 15.0);

    out.push(InstanceData {
        pos: [stick_center.x - 0.5, stick_center.y - 15.0],
        size: [1.0, 30.0],
        rotation: e.rotation,
        color: [1.0, 1.0, 1.0, 1.0],
        shape_type: 0.0, // Rect
        alpha: 1.0,
    });

    let s = 10.0;
    for i in 0..4 {
        out.push(InstanceData {
            pos: [handles[i].x - s*0.5, handles[i].y - s*0.5],
            size: [s, s],
            rotation: e.rotation,
            color: [1.0, 1.0, 1.0, 1.0],
            shape_type: 0.0,
            alpha: 1.0,
        });
    }

    out.push(InstanceData {
        pos: [handles[4].x - s*0.5, handles[4].y - s*0.5],
        size: [s, s],
        rotation: e.rotation,
        color: [1.0, 1.0, 1.0, 1.0],
        shape_type: 1.0,
        alpha: 1.0,
    });

    out
}
