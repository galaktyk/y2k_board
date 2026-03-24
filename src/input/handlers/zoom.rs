use glam::Vec2;

use crate::camera::Camera;
use crate::input::state::InputState;

pub fn on_scroll(
    state: &mut InputState,
    camera: &mut Camera,
    screen_size: Vec2,
    dx: f32,
    dy: f32,
) {
    if state.touchpad_mode && !state.ctrl_held {
        // In touchpad mode, interpret scroll as pan
        // Normalize by zoom so movement follows fingers 1:1 in world space
        camera.pan -= Vec2::new(dx, dy) / camera.zoom;
    } else {
        // Default or forced zoom (Ctrl+Scroll)
        // Two-finger pinch on touchpads usually manifests as scroll+ctrl.
        // We use a magnitude-based factor for smoother touchpad zooming.
        let amount = (dy.abs() * 0.005).min(0.5);
        let factor = if dy > 0.0 { 1.0 + amount } else { 1.0 / (1.0 + amount) };
        camera.zoom_toward(state.mouse_pos, screen_size, factor);
    }
}