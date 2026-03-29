use glam::Vec2;
use crate::camera::Camera;
use crate::input::state::InputState;

// The pan velocity smoothing factor, between 0 and 1.
// Higher values make the velocity change more smoothly but also more slowly.
pub const PAN_VELOCITY_SMOOTHING: f32 = 0.45;

// Minimum initial pan velocity (in screen pixels per second) required to trigger pan glide on mouse release.
pub const PAN_GLIDE_MIN_LAUNCH_SPEED_SCREEN: f32 = 120.0;

pub const PAN_GLIDE_MAX_LAUNCH_SPEED_SCREEN: f32 = 4000.0;

// If the user releases the mouse after panning but has been idle for more than this duration, we won't trigger pan glide.
pub const PAN_GLIDE_MAX_IDLE_BEFORE_RELEASE_SECS: f64 = 0.075;

pub fn cancel_pan_glide(state: &mut InputState) {
    state.pan_velocity = Vec2::ZERO;
    state.pan_velocity_sample_time = None;
}

pub fn begin_pan(state: &mut InputState, camera: &Camera) {
    cancel_pan_glide(state);
    state.panning = true;
    state.pan_start_screen = state.mouse_pos;
    state.pan_start_world = camera.pan;
    state.pan_velocity_sample_time = Some(miniquad::date::now());
}

pub fn finalize_pan_glide(state: &mut InputState, zoom: f32) {
    let idle_before_release = state
        .pan_velocity_sample_time
        .map(|last_motion| miniquad::date::now() - last_motion)
        .unwrap_or(f64::INFINITY);
    let launch_speed_screen = state.pan_velocity.length() * zoom;
    if idle_before_release > PAN_GLIDE_MAX_IDLE_BEFORE_RELEASE_SECS
        || launch_speed_screen < PAN_GLIDE_MIN_LAUNCH_SPEED_SCREEN
    {
        state.pan_velocity = Vec2::ZERO;
    } else if launch_speed_screen > PAN_GLIDE_MAX_LAUNCH_SPEED_SCREEN {
        state.pan_velocity =
            state.pan_velocity.normalize() * (PAN_GLIDE_MAX_LAUNCH_SPEED_SCREEN / zoom);
    }
    state.pan_velocity_sample_time = None;
}
