use glam::Vec2;

use crate::board::Element;

pub struct InputState {
    pub mouse_pos: Vec2,
    pub mouse_down_left: bool,
    pub mouse_down_right: bool,
    pub mouse_down_middle: bool,
    pub space_held: bool,
    #[allow(dead_code)]
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