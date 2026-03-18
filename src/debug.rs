use std::fmt;
use std::path::{Path, PathBuf};

use glam::Vec2;

use crate::board::{
    default_border_width, default_line_stroke_width, default_stroke_color, Board, Element,
    ShapeType, TextData,
};
use crate::camera::Camera;
use crate::images::{ImageImportError, ImageManager};
use crate::palette;



const LOREM_IPSUM: &str = 
    // Thai
    "สวัสดีครับ ยินดีต้อนรับสู่โลกแห่งภาษา ฉันชื่อลอเรม และฉันอาศัยอยู่ในเมืองที่สวยงาม \
    การเรียนรู้ภาษาต่างๆ ช่วยให้เราเข้าใจวัฒนธรรมที่หลากหลาย ประเทศไทยมีประวัติศาสตร์อันยาวนาน \
    อาหารไทยเป็นที่นิยมทั่วโลก เช่น ผัดไทย ต้มยำกุ้ง และแกงเขียวหวาน \
    \
    // Traditional Chinese (繁體中文)
    繁體中文主要使用於台灣、香港及澳門等地區，保留了漢字的傳統書寫形式。 \
    台灣擁有豐富的自然景觀與多元文化，故宮博物院收藏了無數珍貴的中華文物。 \
    傳統節慶如農曆新年、中秋節與端午節，至今仍在華人社會中廣泛慶祝與傳承。 \
    \
    // Simplified Chinese (简体中文)
    简体中文是中国大陆、新加坡和马来西亚等地区的官方书写系统，由繁体字简化而来。 \
    万里长城是中国最著名的历史遗迹，北京故宫是世界上保存最完好的古代宫殿建筑群之一。 \
    中国的四大发明——造纸术、印刷术、火药和指南针，对世界文明的发展产生了深远影响。 \
    \
    // Japanese (half kanji)
    日本語は漢字とひらがなとカタカナを組み合わせた言語です。東京は日本の首都であり世界最大の都市圏です。 \
    桜の季節には花見をする文化があり、春になると公園は美しい花でいっぱいになります。 \
    日本の伝統文化には茶道、書道、剣道などがあり、これらは何世紀にもわたって受け継がれてきました。 \
    \
    // English
    Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore. \
    The quick brown fox jumps over the lazy dog near the riverbank on a warm summer afternoon. \
    Language is the foundation of culture, and learning new scripts opens doors to entirely new worlds of thought. \
    \
    // Hindi
    हिंदी भारत की राजभाषा है और यह देवनागरी लिपि में लिखी जाती है। भारत एक विविधताओं से भरा देश है। \
    यहाँ अनेक भाषाएँ, धर्म और संस्कृतियाँ एक साथ फलती-फूलती हैं। ताजमहल विश्व के सात अजूबों में से एक है। \
    भारतीय खाना अपने मसालों और स्वाद के लिए पूरी दुनिया में प्रसिद्ध है जैसे बिरयानी और दाल मखनी। \
    \
    // Arabic
    اللغة العربية من أقدم اللغات وأكثرها انتشاراً في العالم. تُكتب من اليمين إلى اليسار وتتميز بجمال خطها. \
    الحضارة العربية أسهمت إسهاماً كبيراً في تطور العلوم والفلسفة والأدب عبر التاريخ. \
    من أشهر المعالم العربية برج خليفة في دبي والأهرامات في مصر والمدينة القديمة في مراكش.";


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
            let normalized = current_random / 0.15;
            let idx = (normalized * emoji_words.len() as f32) as usize % emoji_words.len();
            emoji_words[idx]
        } else if current_random < 0.3 {
            let normalized = (current_random - 0.15) / 0.15;
            let idx = (normalized * zwj_words.len() as f32) as usize % zwj_words.len();
            zwj_words[idx]
        } else {
            let normalized = (current_random - 0.3) / 0.7;
            let idx = (normalized * base_words.len() as f32) as usize % base_words.len();
            base_words[idx]
        };

        result.push_str(word);
        if i < word_count - 1 {
            result.push(' ');
        }
    }

    result
}

#[derive(Debug)]
pub enum DebugImageSpawnError {
    ExecutablePath(std::io::Error),
    ReadDir {
        path: PathBuf,
        source: std::io::Error,
    },
    FolderMissing(PathBuf),
    NoImages(PathBuf),
    Import {
        path: PathBuf,
        source: ImageImportError,
    },
}

impl fmt::Display for DebugImageSpawnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DebugImageSpawnError::ExecutablePath(err) => write!(f, "failed to resolve executable path: {err}"),
            DebugImageSpawnError::ReadDir { path, source } => {
                write!(f, "failed to read debug image folder {}: {source}", path.display())
            }
            DebugImageSpawnError::FolderMissing(path) => {
                write!(f, "debug image folder not found: {}", path.display())
            }
            DebugImageSpawnError::NoImages(path) => {
                write!(f, "no supported images found in {}", path.display())
            }
            DebugImageSpawnError::Import { path, source } => {
                write!(f, "failed to import debug image {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for DebugImageSpawnError {}

fn rng(seed: &mut u64) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    (*seed as u32) as f32 / u32::MAX as f32
}

fn viewport_image_size(display_size: [f32; 2], camera: &Camera, screen_size: Vec2) -> Vec2 {
    let mut size = Vec2::from_array(display_size);
    let viewport_world = Vec2::new(
        screen_size.x / camera.zoom.max(0.0001),
        screen_size.y / camera.zoom.max(0.0001),
    ) * 0.6;
    let scale = (viewport_world.x / size.x)
        .min(viewport_world.y / size.y)
        .min(1.0);
    size *= scale.max(0.01);
    size
}

fn debug_images_dir() -> Result<PathBuf, DebugImageSpawnError> {
    let exe_path = std::env::current_exe().map_err(DebugImageSpawnError::ExecutablePath)?;
    let Some(exe_dir) = exe_path.parent() else {
        return Err(DebugImageSpawnError::FolderMissing(PathBuf::from("debug_images")));
    };

    Ok(exe_dir.join("debug_images"))
}

fn is_supported_debug_image(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some(ext)
            if ext.eq_ignore_ascii_case("png")
                || ext.eq_ignore_ascii_case("jpg")
                || ext.eq_ignore_ascii_case("jpeg")
                || ext.eq_ignore_ascii_case("webp")
                || ext.eq_ignore_ascii_case("bmp")
                || ext.eq_ignore_ascii_case("gif")
    )
}

fn collect_debug_images(folder: &Path) -> Result<Vec<PathBuf>, DebugImageSpawnError> {
    if !folder.is_dir() {
        return Err(DebugImageSpawnError::FolderMissing(folder.to_path_buf()));
    }

    let entries = std::fs::read_dir(folder).map_err(|source| DebugImageSpawnError::ReadDir {
        path: folder.to_path_buf(),
        source,
    })?;

    let mut images = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| DebugImageSpawnError::ReadDir {
            path: folder.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_file() && is_supported_debug_image(&path) {
            images.push(path);
        }
    }

    if images.is_empty() {
        return Err(DebugImageSpawnError::NoImages(folder.to_path_buf()));
    }

    Ok(images)
}

pub fn spawn_debug_shapes(board: &mut Board, camera: &Camera, screen_size: Vec2) {
    let (vis_min, vis_max) = camera.visible_rect(screen_size);
    let vis_size = vis_max - vis_min;
    
    let mut seed: u64 = (board.elements.len() as u64)
        .wrapping_mul(0x9e3779b97f4a7c15)
        ^ 0xdeadbeefcafe1234;
        
    let mut spawn = |shape: ShapeType, with_text: bool| {
        let rx  = rng(&mut seed);
        let ry  = rng(&mut seed);
        let rw  = rng(&mut seed);
        let rh  = rng(&mut seed);
  let r_border_idx = rng(&mut seed);
        let r_text_idx = rng(&mut seed);
        let r_text = rng(&mut seed);

        let pos   = vis_min + Vec2::new(rx * vis_size.x, ry * vis_size.y);
        let size  = Vec2::new(100.0 + rw * 300.0, 100.0 + rh * 300.0);
        
        let border_color_idx = (r_border_idx * palette::PALETTE.len() as f32) as usize % palette::PALETTE.len();
        let text_color_idx = (r_text_idx * palette::PALETTE.len() as f32) as usize % palette::PALETTE.len();
        let border_color = palette::PALETTE[border_color_idx];
        let text_color = palette::PALETTE[text_color_idx];
        
        let id    = board.next_id();

        let text = if with_text {
            Some(TextData {
                content: generate_lorem_text(size, r_text),
                font_size: 24.0,
                color: text_color,
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
            color: border_color,
            stroke_color: if shape == ShapeType::Line {
                border_color
            } else {
                palette::BLACK
            },
            border_width: 1,
            stroke_width: 1,
            selected: false,
            text,
            image: None,
            text_layout_generation: 0,
        });
    };

    for _ in 0..100 { spawn(ShapeType::Rect, false); }
    for _ in 0..100 { spawn(ShapeType::Ellipse, false); }
    for _ in 0..100 { spawn(ShapeType::Line, false); }
    for _ in 0..100 { spawn(ShapeType::Rect, true); }

    println!(
        "Alt+Ctrl+B: spawned 100 rect, 100 ellipse, 100 lines, 100 text | total elements: {}",
        board.elements.len()
    );
}

pub fn spawn_debug_images(
    board: &mut Board,
    camera: &Camera,
    screen_size: Vec2,
    image_manager: &mut ImageManager,
) -> Result<usize, DebugImageSpawnError> {
    let debug_dir = debug_images_dir()?;
    let image_paths = collect_debug_images(&debug_dir)?;
    let (vis_min, vis_max) = camera.visible_rect(screen_size);
    let vis_size = vis_max - vis_min;

    let mut seed: u64 = (board.elements.len() as u64)
        .wrapping_mul(0x517cc1b727220a95)
        ^ 0xa5a5f0f0deadbeef;
    let mut spawned = 0usize;

    for _ in 0..20 {
        let pick = (rng(&mut seed) * image_paths.len() as f32) as usize;
        let source_path = &image_paths[pick.min(image_paths.len() - 1)];
        let asset_id = board.next_available_id();
        let imported = image_manager.import_from_source(asset_id, source_path).map_err(|source| {
            DebugImageSpawnError::Import {
                path: source_path.clone(),
                source,
            }
        })?;

        let size = viewport_image_size(imported.display_size, camera, screen_size);
        let max_x = (vis_size.x - size.x).max(0.0);
        let max_y = (vis_size.y - size.y).max(0.0);
        let pos = vis_min + Vec2::new(rng(&mut seed) * max_x, rng(&mut seed) * max_y);
        let id = board.next_id();

        board.insert_element_untracked(Element {
            id,
            shape: ShapeType::Image,
            pos,
            size,
            rotation: 0.0,
            color: [1.0, 1.0, 1.0, 1.0],
            stroke_color: default_stroke_color(),
            border_width: default_border_width(),
            stroke_width: default_line_stroke_width(),
            selected: false,
            text: None,
            image: Some(imported.data),
            text_layout_generation: 0,
        });
        spawned += 1;
    }

    println!(
        "Alt+F8: spawned {spawned} images from {} | total elements: {}",
        debug_dir.display(),
        board.elements.len()
    );

    Ok(spawned)
}
