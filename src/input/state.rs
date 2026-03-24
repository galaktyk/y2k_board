use std::collections::{HashMap, VecDeque};

use glam::Vec2;

use crate::board::Element;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConnectionDrag {
    pub source_id: u64,
    pub source_norm_pos: Vec2,
    pub start_world: Vec2,
    pub end_world: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionBounds {
    pub pos: Vec2,
    pub size: Vec2,
    pub rotation: f32,
}

impl SelectionBounds {
    pub fn new(pos: Vec2, size: Vec2) -> Self {
        Self {
            pos,
            size,
            rotation: 0.0,
        }
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

    pub fn with_rotation(mut self, rotation: f32) -> Self {
        self.rotation = rotation;
        self
    }

    pub fn with_position(mut self, pos: Vec2) -> Self {
        self.pos = pos;
        self
    }

    pub fn rotate_point(&self, point: Vec2) -> Vec2 {
        let center = self.center();
        let offset = point - center;
        let c = self.rotation.cos();
        let s = self.rotation.sin();
        center + Vec2::new(offset.x * c - offset.y * s, offset.x * s + offset.y * c)
    }

    pub fn corners(&self) -> [Vec2; 4] {
        let min = self.min();
        let max = self.max();
        [
            self.rotate_point(min),
            self.rotate_point(Vec2::new(max.x, min.y)),
            self.rotate_point(max),
            self.rotate_point(Vec2::new(min.x, max.y)),
        ]
    }

    pub fn contains(&self, point: Vec2) -> bool {
        let center = self.center();
        let offset = point - center;
        let c = self.rotation.cos();
        let s = self.rotation.sin();
        let local = center + Vec2::new(offset.x * c + offset.y * s, -offset.x * s + offset.y * c);
        let min = self.min();
        let max = self.max();
        local.x >= min.x && local.x <= max.x && local.y >= min.y && local.y <= max.y
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
    pub pan_velocity: Vec2,
    pub pan_velocity_sample_time: Option<f64>,
    pub dragging_tool: bool,
    pub drag_start_world: Vec2,
    pub drag_mode: DragMode,
    pub pending_drag_mode: DragMode,
    pub pending_drag_start_screen: Vec2,
    pub pending_drag_start_world: Vec2,
    pub move_start_world: Vec2,
    pub move_origin: Vec<(u64, Vec2, Vec2, f32)>,
    pub preview: Option<Element>,
    pub move_delta: Vec2,
    pub rotate_delta: f32,
    pub connection_drag: Option<ConnectionDrag>,
    pub marquee_bounds: Option<SelectionBounds>,
    pub selection_bounds: Option<SelectionBounds>,
    pub drag_selection_bounds: Option<SelectionBounds>,
    pub transform_bounds_origin: Option<SelectionBounds>,
    pub active_text_id: Option<u64>,
    pub text_cursor: usize,
    pub text_selecting: bool,
    pub last_click_id: Option<u64>,
    pub last_click_at: Option<f64>,
    pub touchpad_mode: bool,
    pending_resize_text_recompute: VecDeque<(u64, u64)>,
    pending_resize_text_recompute_latest: HashMap<u64, u64>,
    pending_resize_text_recompute_seq: u64,
    pub last_resize_text_bump: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HandleDir {
    TL,
    TR,
    BR,
    BL,
    Top,
    Right,
    Bottom,
    Left,
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
    CreatingConnection,
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
            pan_velocity: Vec2::ZERO,
            pan_velocity_sample_time: None,
            dragging_tool: false,
            drag_start_world: Vec2::ZERO,
            drag_mode: DragMode::None,
            pending_drag_mode: DragMode::None,
            pending_drag_start_screen: Vec2::ZERO,
            pending_drag_start_world: Vec2::ZERO,
            move_start_world: Vec2::ZERO,
            move_origin: Vec::new(),
            preview: None,
            move_delta: Vec2::ZERO,
            rotate_delta: 0.0,
            connection_drag: None,
            marquee_bounds: None,
            selection_bounds: None,
            drag_selection_bounds: None,
            transform_bounds_origin: None,
            active_text_id: None,
            text_cursor: 0,
            text_selecting: false,
            last_click_id: None,
            last_click_at: None,
            touchpad_mode: false,
            pending_resize_text_recompute: VecDeque::new(),
            pending_resize_text_recompute_latest: HashMap::new(),
            pending_resize_text_recompute_seq: 0,
            last_resize_text_bump: 0.0,
        }
    }

    pub fn want_pan(&self) -> bool {
        self.space_held || self.mouse_down_middle
    }

    pub fn has_pan_glide(&self) -> bool {
        self.pan_velocity.length_squared() > 0.0
    }


    /** IMPORTANT: Enqueue a text element for resize recomputation.
     * This should fix the lag when resizing multiple text elements
     * might took some time to complete all in the queue, but worth it.
     * No one gonna notice this unless they are resizing like 100+ text elements at once.😈
    */
    pub fn enqueue_resize_text_recompute(&mut self, id: u64) -> bool {
        self.pending_resize_text_recompute_seq =
            self.pending_resize_text_recompute_seq.wrapping_add(1);
        let seq = self.pending_resize_text_recompute_seq;
        let is_new = self
            .pending_resize_text_recompute_latest
            .insert(id, seq)
            .is_none();
        self.pending_resize_text_recompute.push_back((id, seq));
        is_new
    }

    pub fn pop_resize_text_recompute(&mut self) -> Option<u64> {
        while let Some((id, seq)) = self.pending_resize_text_recompute.pop_front() {
            match self.pending_resize_text_recompute_latest.get(&id).copied() {
                Some(latest_seq) if latest_seq == seq => {
                    self.pending_resize_text_recompute_latest.remove(&id);
                    return Some(id);
                }
                _ => continue,
            }
        }
        None
    }

    pub fn has_pending_resize_text_recompute(&self) -> bool {
        !self.pending_resize_text_recompute_latest.is_empty()
    }
}