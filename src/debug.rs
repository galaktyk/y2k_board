use glam::Vec2;
use crate::board::{Board, Element, ShapeType, TextData};
use crate::camera::Camera;

const LOREM_IPSUM: &str = "Lorem नमस्ते ipsum مرحبا dolor स्वास्थ sit amet, consectetur 你好 adipiscing elit, sed do eiusmod こんにちは tempor incididunt ut 労动 et dolore magna aliqua. Ut enim ad მინიმ veniam, उपयोग consequat จาก laboris nisi ut អត្ថបទ aliquip ex ea commodo consequat. Duis aute irure dolor في reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 非常好 しかし例えれば deserunt mollit anim id est laborum, 在這裡 そしてまた 生活 ดีมาก និងសន្តិភាព مرحباً بالعالم.";
const LOREM_EMOJI: &str = "😀😃😄😁😆😅😂🤣😊😇🙂🐶 🐱 🐭 🐹 🐰 🦊 🐻 🐼 🐨 🐯 🦁 🐮 🐷 🐽 🐸 🐵 🐔 🐧 🐦 🐤 🐣 🐥 🦆 🦅 🦉 🦇 🐺 🐗 🐴 🦄 🐝 🐛 🦋 🐌 🐞 🐜 🦗 🕷️ 🕸️ 🦂 🦟 🦠 🐢 🐍 🦎 🐙 🦑😎🥳";
const LOREM_ZWJ: &str = "👩‍💻 👨‍🚀 👩‍⚕️ 👨‍🍳 👩‍🏫 👨‍🔬 👩‍🚒 👨‍🎨 👩‍✈️ 👨‍💼 👩‍🔧 👨‍🏭 👩‍🌾 👨‍⚖️ 👩‍🚀 👨‍💻 👩‍🎤 👨‍🚒 👩‍🍳 👨‍✈️ 👩‍❤️‍👨 👨‍❤️‍👨 👩‍❤️‍👩 👩‍❤️‍💋‍👨 👨‍❤️‍💋‍👨 👩‍❤️‍💋‍👩 👨‍👩‍👧 👨‍👩‍👧‍👦 👩‍👩‍👧‍👦 👨‍👨‍👦 🏳️‍🌈 🏳️‍⚧️ 🧙‍♂️ 🧙‍♀️ 🧛‍♂️ 🧛‍♀️ 🧝‍♂️ 🧝‍♀️ 🧟‍♂️ 🧟‍♀️";


fn generate_lorem_text(size: Vec2, randomness: f32) -> String {
    let area = size.x * size.y;
    let word_count = (area / 1000.0).max(2.0).min(30.0) as usize;

    let base_words: Vec<&str> = LOREM_IPSUM.split_whitespace().collect();
    let emoji_words: Vec<&str> = LOREM_EMOJI.split_whitespace().collect();
    let zwj_words: Vec<&str> = LOREM_ZWJ.split_whitespace().collect();

    let mut result = String::new();
    let mut current_random = randomness;

    for i in 0..word_count {
        // Pseudorandom progression
        current_random = (current_random * 13.0 + 17.0).fract();
        
        // 15% emoji, 15% ZWJ sequence, 70% base text
        let word = if current_random < 0.15 {
            let idx = (current_random * 100.0) as usize % emoji_words.len();
            emoji_words[idx]
        } else if current_random < 0.3 {
            let idx = (current_random * 100.0) as usize % zwj_words.len();
            zwj_words[idx]
        } else {
            let idx = (current_random * 100.0) as usize % base_words.len();
            base_words[idx]
        };

        result.push_str(word);
        if i < word_count - 1 {
            result.push(' ');
        }
    }

    result
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
