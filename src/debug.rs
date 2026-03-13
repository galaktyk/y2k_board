use glam::Vec2;
use crate::board::{Board, Element, ShapeType};
use crate::camera::Camera;

pub fn spawn_debug_shapes(board: &mut Board, camera: &Camera, screen_size: Vec2) {
    let (vis_min, vis_max) = camera.visible_rect(screen_size);
    let vis_size = vis_max - vis_min;
    
    let mut seed: u64 = (board.elements.len() as u64)
        .wrapping_mul(0x9e3779b97f4a7c15)
        ^ 0xdeadbeefcafe1234;
        
    let mut rng = |s: &mut u64| -> f32 {
        *s ^= *s << 13;
        *s ^= *s >> 7;
        *s ^= *s << 17;
        *s as u32 as f32 / u32::MAX as f32
    };
    
    let shapes = [ShapeType::Rect, ShapeType::Ellipse, ShapeType::Line];
    for _ in 0..500 {
        let rx  = rng(&mut seed);
        let ry  = rng(&mut seed);
        let rw  = rng(&mut seed);
        let rh  = rng(&mut seed);
        let rc0 = rng(&mut seed);
        let rc1 = rng(&mut seed);
        let rc2 = rng(&mut seed);
        
        let shape = shapes[(seed % 3) as usize];
        let pos   = vis_min + Vec2::new(rx * vis_size.x, ry * vis_size.y);
        let size  = Vec2::new(100.0 + rw * 300.0, 100.0 + rh * 300.0);
        let color = [rc0 * 0.7 + 0.3, rc1 * 0.7 + 0.3, rc2 * 0.7 + 0.3, 0.85];
        let id    = board.next_id();
        
        board.elements.push(Element { 
            id, 
            shape, 
            pos, 
            size, 
            rotation: 0.0, 
            color, 
            selected: false 
        });
    }
    println!("Alt+Ctrl+B: spawned 500 shapes | total elements: {}", board.elements.len());
}
