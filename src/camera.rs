use glam::Vec2;

pub struct Camera {
    pub pan: Vec2,
    pub zoom: f32,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            pan: Vec2::ZERO,
            zoom: 1.0,
        }
    }

    /// World → screen pixel.
    #[allow(dead_code)]
    pub fn world_to_screen(&self, p: Vec2, screen_size: Vec2) -> Vec2 {
        let center = screen_size * 0.5;
        (p - self.pan) * self.zoom + center
    }

    /// Screen pixel → world.
    pub fn screen_to_world(&self, p: Vec2, screen_size: Vec2) -> Vec2 {
        let center = screen_size * 0.5;
        (p - center) / self.zoom + self.pan
    }

    /// Visible world-space rectangle given current screen size.
    pub fn visible_rect(&self, screen_size: Vec2) -> (Vec2, Vec2) {
        let half = screen_size * 0.5 / self.zoom;
        let min = self.pan - half;
        let max = self.pan + half;
        (min, max)
    }

    /// Zoom toward a screen-space anchor point (keep that point fixed in world space).
    pub fn zoom_toward(&mut self, anchor_screen: Vec2, screen_size: Vec2, factor: f32) {
        let world_before = self.screen_to_world(anchor_screen, screen_size);
        self.zoom = (self.zoom * factor).clamp(0.002, 20.0);
        let world_after = self.screen_to_world(anchor_screen, screen_size);
        self.pan -= world_after - world_before;
    }
}
