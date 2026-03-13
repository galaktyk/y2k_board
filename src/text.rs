use std::cmp::Ordering;
use std::path::Path;

use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, Shaping, SwashCache, Wrap};
use glam::Vec2;
use miniquad::{RenderingBackend, TextureId};

use crate::board::{Board, Element};
use crate::camera::Camera;
use crate::renderer::TextInstanceData;

const ATLAS_SIZE: usize = 1024;
const ATLAS_GAP: usize = 2;

pub struct TextSystem {
    font_system: FontSystem,
    swash_cache: SwashCache,
    clear_bytes: Vec<u8>,
}

impl TextSystem {
    pub fn new() -> Self {
        let mut font_system = FontSystem::new();
        font_system.db_mut().load_fonts_dir(Path::new("fonts"));

        Self {
            font_system,
            swash_cache: SwashCache::new(),
            clear_bytes: vec![0; ATLAS_SIZE * ATLAS_SIZE],
        }
    }

    pub fn build_visible_text_instances(
        &mut self,
        ctx: &mut dyn RenderingBackend,
        atlas: TextureId,
        board: &Board,
        visible_indices: &[usize],
        camera: &Camera,
    ) -> Vec<TextInstanceData> {
        ctx.texture_update(atlas, &self.clear_bytes);

        let mut candidates: Vec<&Element> = visible_indices
            .iter()
            .filter_map(|&index| board.elements.get(index))
            .filter(|element| element.text.as_ref().map(|text| !text.content.is_empty()).unwrap_or(false))
            .collect();

        candidates.sort_by(|left, right| {
            let left_dist = text_host_distance(left, camera.pan);
            let right_dist = text_host_distance(right, camera.pan);
            left_dist.partial_cmp(&right_dist).unwrap_or(Ordering::Equal)
        });

        let mut atlas_x = 0usize;
        let mut atlas_y = 0usize;
        let mut row_h = 0usize;
        let mut instances = Vec::new();

        for element in candidates {
            let Some(text) = element.text.as_ref() else {
                continue;
            };
            let mut surface = self.rasterize_text_surface(element, &text.content, text.font_size, text.color);
            let mut draw_size = surface.size;
            if !surface.bytes.is_empty() {
                if let Some((x, y)) = pack_rect(&mut atlas_x, &mut atlas_y, &mut row_h, draw_size.0, draw_size.1) {
                    ctx.texture_update_part(atlas, x as i32, y as i32, draw_size.0 as i32, draw_size.1 as i32, &surface.bytes);
                    instances.push(build_instance(element, surface.world_pos, draw_size, x, y, text.color));
                    continue;
                }
            }

            surface = self.rasterize_text_surface(element, "■", text.font_size, text.color);
            draw_size = surface.size;
            if !surface.bytes.is_empty() {
                if let Some((x, y)) = pack_rect(&mut atlas_x, &mut atlas_y, &mut row_h, draw_size.0, draw_size.1) {
                    ctx.texture_update_part(atlas, x as i32, y as i32, draw_size.0 as i32, draw_size.1 as i32, &surface.bytes);
                    instances.push(build_instance(element, surface.world_pos, draw_size, x, y, text.color));
                }
            }
        }

        instances
    }

    fn rasterize_text_surface(
        &mut self,
        element: &Element,
        text: &str,
        font_size: f32,
        color: [f32; 4],
    ) -> TextSurface {
        let Some((world_pos, max_pos)) = element.text_bounds() else {
            return TextSurface::empty();
        };

        let width = (max_pos.x - world_pos.x).max(1.0).ceil() as usize;
        let height = (max_pos.y - world_pos.y).max(1.0).ceil() as usize;
        if width == 0 || height == 0 || width > ATLAS_SIZE || height > ATLAS_SIZE {
            return TextSurface::empty();
        }

        let metrics = Metrics::new(font_size.max(8.0), (font_size * 1.35).max(font_size + 4.0));
        let attrs = Attrs::new();
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_size(&mut self.font_system, Some(width as f32), Some(height as f32));
        buffer.set_wrap(&mut self.font_system, Wrap::WordOrGlyph);
        buffer.set_text(&mut self.font_system, text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.font_system, true);

        let mut pixels = vec![0u8; width * height];
        let text_color = Color::rgba(
            (color[0].clamp(0.0, 1.0) * 255.0) as u8,
            (color[1].clamp(0.0, 1.0) * 255.0) as u8,
            (color[2].clamp(0.0, 1.0) * 255.0) as u8,
            (color[3].clamp(0.0, 1.0) * 255.0) as u8,
        );
        buffer.draw(&mut self.font_system, &mut self.swash_cache, text_color, |x, y, w, h, draw_color| {
            let alpha = draw_color.a();
            for iy in 0..h as i32 {
                let py = y + iy;
                if py < 0 || py >= height as i32 {
                    continue;
                }
                for ix in 0..w as i32 {
                    let px = x + ix;
                    if px < 0 || px >= width as i32 {
                        continue;
                    }
                    pixels[py as usize * width + px as usize] = alpha;
                }
            }
        });

        TextSurface {
            bytes: pixels,
            size: (width, height),
            world_pos,
        }
    }
}

struct TextSurface {
    bytes: Vec<u8>,
    size: (usize, usize),
    world_pos: Vec2,
}

impl TextSurface {
    fn empty() -> Self {
        Self {
            bytes: Vec::new(),
            size: (0, 0),
            world_pos: Vec2::ZERO,
        }
    }
}

fn text_host_distance(element: &Element, screen_center: Vec2) -> f32 {
    let center = element.pos + element.size * 0.5;
    center.distance_squared(screen_center)
}

fn pack_rect(
    atlas_x: &mut usize,
    atlas_y: &mut usize,
    row_h: &mut usize,
    width: usize,
    height: usize,
) -> Option<(usize, usize)> {
    if width == 0 || height == 0 || width > ATLAS_SIZE || height > ATLAS_SIZE {
        return None;
    }
    if *atlas_x + width > ATLAS_SIZE {
        *atlas_x = 0;
        *atlas_y += *row_h + ATLAS_GAP;
        *row_h = 0;
    }
    if *atlas_y + height > ATLAS_SIZE {
        return None;
    }

    let out = (*atlas_x, *atlas_y);
    *atlas_x += width + ATLAS_GAP;
    *row_h = (*row_h).max(height);
    Some(out)
}

fn build_instance(
    element: &Element,
    world_pos: Vec2,
    size: (usize, usize),
    atlas_x: usize,
    atlas_y: usize,
    color: [f32; 4],
) -> TextInstanceData {
    let uv_min = [atlas_x as f32 / ATLAS_SIZE as f32, atlas_y as f32 / ATLAS_SIZE as f32];
    let uv_max = [
        (atlas_x + size.0) as f32 / ATLAS_SIZE as f32,
        (atlas_y + size.1) as f32 / ATLAS_SIZE as f32,
    ];

    TextInstanceData {
        pos: world_pos.to_array(),
        size: [size.0 as f32, size.1 as f32],
        origin: (element.pos + element.size * 0.5).to_array(),
        rotation: element.rotation,
        uv_min,
        uv_max,
        color,
    }
}