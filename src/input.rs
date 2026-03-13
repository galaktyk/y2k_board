use glam::Vec2;
use crate::board::{Board, Element, Op, ShapeType};
use crate::camera::Camera;
use crate::toolbar::{Tool, Toolbar, ToolbarAction, TOOLBAR_HEIGHT};

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

    // Move-selected state
    moving_elements: bool,
    move_start_world: Vec2,
    /// Element positions at the start of the move gesture
    move_origin: Vec<(u64, Vec2)>,

    /// Transient preview element shown while dragging a create tool.
    pub preview: Option<Element>,
    /// Live drag delta for selected elements (shown before commit).
    pub move_delta: Vec2,
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
            moving_elements: false,
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
            if let Some(id) = board.hit_test(world) {
                let already_selected = board.elements.iter().find(|e| e.id == id).map(|e| e.selected).unwrap_or(false);
                if !already_selected {
                    board.deselect_all();
                    board.select_only(id);
                }
                // Track positions for a potential move
                state.move_origin = board.elements.iter()
                    .filter(|e| e.selected)
                    .map(|e| (e.id, e.pos))
                    .collect();
                state.moving_elements = true;
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
    toolbar: &Toolbar,
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

    // Commit move
    if state.moving_elements && state.move_delta != Vec2::ZERO {
        let moves: Vec<(u64, Vec2, Vec2)> = state.move_origin.iter()
            .map(|&(id, old)| (id, old, old + state.move_delta))
            .collect();
        if !moves.is_empty() {
            board.apply_op(Op::MoveElements { moves });
        }
    }
    state.moving_elements = false;
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
                let _ = camera; let _ = screen_size; let _ = toolbar;
                elem.id = board.next_id();
                board.apply_op(Op::AddElement(elem));
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

    // Move selected elements (live preview via move_delta)
    if state.moving_elements {
        state.move_delta = world - state.move_start_world;
        // Apply live delta to element positions temporarily
        for e in &mut board.elements {
            if e.selected {
                if let Some(&(_, orig)) = state.move_origin.iter().find(|&&(id, _)| id == e.id) {
                    e.pos = orig + state.move_delta;
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
