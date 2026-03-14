use glam::Vec2;

use crate::board::Element;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionBounds {
    pub pos: Vec2,
    pub size: Vec2,
}

impl SelectionBounds {
    pub fn new(pos: Vec2, size: Vec2) -> Self {
        Self { pos, size }
    }

    pub fn from_points(a: Vec2, b: Vec2) -> Self {
        let min = a.min(b);
        let max = a.max(b);
        Self::new(min, max - min)
    }

    pub fn min(&self) -> Vec2 {
        self.pos
    }

    pub fn max(&self) -> Vec2 {
        self.pos + self.size
    }

    pub fn center(&self) -> Vec2 {
        self.pos + self.size * 0.5
    }

    pub fn contains(&self, point: Vec2) -> bool {
        let min = self.min();
        let max = self.max();
        point.x >= min.x && point.x <= max.x && point.y >= min.y && point.y <= max.y
    }
}

pub struct InputState {
    pub mouse_pos: Vec2,
    pub mouse_down_left: bool,
    pub mouse_down_right: bool,
    pub mouse_down_middle: bool,
    pub space_held: bool,
    pub shift_held: bool,
    pub ctrl_held: bool,
    pub panning: bool,
    pub pan_start_screen: Vec2,
    pub pan_start_world: Vec2,
    pub dragging_tool: bool,
    pub drag_start_world: Vec2,
    pub drag_mode: DragMode,
    pub move_start_world: Vec2,
    pub move_origin: Vec<(u64, Vec2, Vec2, f32)>,
    pub preview: Option<Element>,
    pub move_delta: Vec2,
    pub marquee_bounds: Option<SelectionBounds>,
    pub drag_selection_bounds: Option<SelectionBounds>,
    pub transform_bounds_origin: Option<SelectionBounds>,
    pub active_text_id: Option<u64>,
    pub text_cursor: usize,
    pub text_selecting: bool,
    pub last_click_id: Option<u64>,
    pub last_click_at: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HandleDir {
    TL,
    TR,
    BR,
    BL,
    LineStart,
    LineEnd,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DragMode {
    None,
    MoveSelected,
    MarqueeSelect,
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
            shift_held: false,
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
            marquee_bounds: None,
            drag_selection_bounds: None,
            transform_bounds_origin: None,
            active_text_id: None,
            text_cursor: 0,
            text_selecting: false,
            last_click_id: None,
            last_click_at: None,
        }
    }

    pub fn want_pan(&self) -> bool {
        self.space_held || self.mouse_down_middle
    }
}