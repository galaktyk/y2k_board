use std::collections::HashMap;
use cosmic_text::{CacheKey, SwashImage, SwashContent};
use miniquad::{RenderingBackend, TextureId};

pub const TEXT_ATLAS_SIZE: usize = 2048;
pub const EMOJI_ATLAS_SIZE: usize = 1024;
pub const ATLAS_GAP: usize = 2;
pub const FALLBACK_GLYPH_SIZE: usize = 8;

#[derive(Clone, Copy)]
pub struct AtlasEntry {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub left: i32,
    pub top: i32,
}

impl AtlasEntry {
    pub fn uv_min(&self, atlas_size: f32) -> [f32; 2] {
        [self.x as f32 / atlas_size, self.y as f32 / atlas_size]
    }

    pub fn uv_max(&self, atlas_size: f32) -> [f32; 2] {
        [
            (self.x + self.width) as f32 / atlas_size,
            (self.y + self.height) as f32 / atlas_size,
        ]
    }
}

pub struct Atlas {
    pub size: usize,
    next_x: usize,
    next_y: usize,
    row_h: usize,
    pub entries: HashMap<CacheKey, AtlasEntry>,
    /// Pre-reserved ■ glyph for use when the atlas overflows.
    fallback: Option<AtlasEntry>,
}

impl Atlas {
    pub fn new(size: usize, reserve_overlay_pixel: bool) -> Self {
        let (next_x, row_h, fallback) = if reserve_overlay_pixel {
            // x=0: 1×1 overlay pixel (selection/caret), then ATLAS_GAP, then
            // FALLBACK_GLYPH_SIZE×FALLBACK_GLYPH_SIZE filled square (■ for overflow).
            let fb_x = 1 + ATLAS_GAP;
            let fb_size = FALLBACK_GLYPH_SIZE;
            let entry = AtlasEntry {
                x: fb_x,
                y: 0,
                width: fb_size,
                height: fb_size,
                left: 0,
                top: fb_size as i32,
            };
            (fb_x + fb_size + ATLAS_GAP, fb_size, Some(entry))
        } else {
            (0, 0, None)
        };
        Self {
            size,
            next_x,
            next_y: 0,
            row_h,
            entries: HashMap::new(),
            fallback,
        }
    }

    pub fn insert(
        &mut self,
        ctx: &mut dyn RenderingBackend,
        texture: TextureId,
        cache_key: CacheKey,
        image: &SwashImage,
    ) -> Option<AtlasEntry> {
        if let Some(entry) = self.entries.get(&cache_key) {
            return Some(*entry);
        }

        let width = image.placement.width as usize;
        let height = image.placement.height as usize;
        if width == 0 || height == 0 {
            return None; // whitespace / zero-size glyph — nothing to draw
        }

        let Some((x, y)) = self.pack(width, height) else {
            // Atlas is full — show ■ so the overflow is visually apparent.
            return self.fallback;
        };
        let bytes = atlas_bytes(image)?;
        ctx.texture_update_part(
            texture,
            x as i32,
            y as i32,
            width as i32,
            height as i32,
            &bytes,
        );

        let entry = AtlasEntry {
            x,
            y,
            width,
            height,
            left: image.placement.left,
            top: image.placement.top,
        };
        self.entries.insert(cache_key, entry);
        Some(entry)
    }

    fn pack(&mut self, width: usize, height: usize) -> Option<(usize, usize)> {
        if width > self.size || height > self.size {
            return None;
        }
        if self.next_x + width > self.size {
            self.next_x = 0;
            self.next_y += self.row_h + ATLAS_GAP;
            self.row_h = 0;
        }
        if self.next_y + height > self.size {
            return None;
        }

        let out = (self.next_x, self.next_y);
        self.next_x += width + ATLAS_GAP;
        self.row_h = self.row_h.max(height);
        Some(out)
    }
}

fn atlas_bytes(image: &SwashImage) -> Option<Vec<u8>> {
    match image.content {
        SwashContent::Mask => Some(image.data.clone()),
        SwashContent::Color => Some(image.data.clone()),
        SwashContent::SubpixelMask => {
            let mut bytes =
                Vec::with_capacity((image.placement.width * image.placement.height) as usize);
            for chunk in image.data.chunks_exact(3) {
                let alpha =
                    ((u16::from(chunk[0]) + u16::from(chunk[1]) + u16::from(chunk[2])) / 3) as u8;
                bytes.push(alpha);
            }
            Some(bytes)
        }
    }
}
