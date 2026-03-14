use glam::Vec2;
use crate::board::{Board, Element, ShapeType, TextData};
use crate::camera::Camera;

const LOREM_IPSUM: &str = "Lorem नमस्ते ipsum مرحبا dolor สวัสดี sit amet, consectetur 你好 adipiscing elit, sed do eiusmod こんにちは tempor incididunt ut 劳动 et dolore magna aliqua. Ut enim ad მინიმ veniam, उपयोग consequat จาก laboris nisi ut អត្ថបទ aliquip ex ea commodo consequat. Duis aute irure dolor في reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 非常好 しかし例えれば deserunt mollit anim id est laborum, 在這裡 そしてまた 生活 ดีมาก និងសន្តិភាព مرحباً بالعالم.";

fn generate_lorem_text(size: Vec2, randomness: f32) -> String {
    // Length based on element size - larger elements get more text
    let area = size.x * size.y;
    let max_chars = (area / 100.0).min(LOREM_IPSUM.len() as f32) as usize;
    let max_chars = max_chars.max(10).min(LOREM_IPSUM.len());

    // Ensure we don't overflow when slicing
    let max_start = LOREM_IPSUM.len().saturating_sub(max_chars);
    let start = (randomness.clamp(0.0, 0.999_999_9) * (max_start as f32 + 1.0)) as usize;

    LOREM_IPSUM
        .chars()
        .skip(start)
        .take(max_chars)
        .collect()
}

pub fn spawn_debug_shapes(board: &mut Board, camera: &Camera, screen_size: Vec2) {
    let (vis_min, vis_max) = camera.visible_rect(screen_size);
    let vis_size = vis_max - vis_min;
    
    let mut seed: u64 = (board.elements.len() as u64)
        .wrapping_mul(0x9e3779b97f4a7c15)
        ^ 0xdeadbeefcafe1234;
        
    let rng = |s: &mut u64| -> f32 {
        *s ^= *s << 13;
        *s ^= *s >> 7;
        *s ^= *s << 17;
        *s as u32 as f32 / u32::MAX as f32
    };
    
    let mut spawn = |shape: ShapeType, with_text: bool| {
        let rx  = rng(&mut seed);
        let ry  = rng(&mut seed);
        let rw  = rng(&mut seed);
        let rh  = rng(&mut seed);
        let rc0 = rng(&mut seed);
        let rc1 = rng(&mut seed);
        let rc2 = rng(&mut seed);
        let r_text = rng(&mut seed);

        let pos   = vis_min + Vec2::new(rx * vis_size.x, ry * vis_size.y);
        let size  = Vec2::new(100.0 + rw * 300.0, 100.0 + rh * 300.0);
        let color = [rc0 * 0.7 + 0.3, rc1 * 0.7 + 0.3, rc2 * 0.7 + 0.3, 0.85];
        let id    = board.next_id();

        let text = if with_text {
            Some(TextData {
                content: generate_lorem_text(size, r_text),
                font_size: 24.0,
                color: [1.0, 1.0, 1.0, 1.0],
            })
        } else {
            None
        };

        board.insert_element_untracked(Element {
            id,
            shape,
            pos,
            size,
            rotation: 0.0,
            color,
            selected: false,
            text,
            text_layout_generation: 0,
        });
    };

    for _ in 0..100 { spawn(ShapeType::Rect, false); }
    for _ in 0..100 { spawn(ShapeType::Ellipse, false); }
    for _ in 0..100 { spawn(ShapeType::Line, false); }
    for _ in 0..100 { spawn(ShapeType::Text, true); }

    println!(
        "Alt+Ctrl+B: spawned 100 rect, 100 ellipse, 100 lines, 100 text | total elements: {}",
        board.elements.len()
    );
}
