use glam::Vec2;

use crate::camera::Camera;
use crate::input::state::InputState;

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