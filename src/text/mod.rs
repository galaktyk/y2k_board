mod edit;
mod atlas;
mod style;
mod fonts;
mod layout;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use cosmic_text::{
    Buffer, CacheKey, FontSystem, Motion, Shaping,
    SwashCache, SwashContent, Wrap, fontdb,
};
use glam::Vec2;
use miniquad::{RenderingBackend, TextureId};

use crate::board::{Board, Element, TextData};
use crate::palette;
use crate::platform::browser_io;
use crate::rendering::renderer::TextInstanceData;

pub use edit::{TextEditSession, TextEditSnapshot};
pub use atlas::{Atlas, AtlasEntry, TEXT_ATLAS_SIZE, EMOJI_ATLAS_SIZE, ATLAS_GAP, FALLBACK_GLYPH_SIZE};
pub use style::{
    text_metrics, default_text_attrs,
    cosmic_color_to_rgba, SELECTION_COLOR, CARET_COLOR
};
pub use fonts::{
    new_font_system, configure_bundled_font_defaults, decode_browser_font_bytes
};
pub use layout::{
    LineOffsets, global_byte_to_cursor, cursor_to_global_byte,
    selection_range, caret_geometry
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiTextAnchor {
    TopLeft,
    TopCenter,
}

#[derive(Clone, Debug)]
pub struct UiTextSpec {
    pub content: String,
    pub pos: Vec2,
    pub font_size: f32,
    pub line_height: Option<f32>,
    pub max_width: Option<f32>,
    pub color: [f32; 4],
    pub anchor: UiTextAnchor,
}

impl UiTextSpec {
    pub fn top_left(
        content: impl Into<String>,
        pos: Vec2,
        font_size: f32,
        color: [f32; 4],
    ) -> Self {
        Self {
            content: content.into(),
            pos,
            font_size,
            line_height: None,
            max_width: None,
            color,
            anchor: UiTextAnchor::TopLeft,
        }
    }

    pub fn top_center(
        content: impl Into<String>,
        pos: Vec2,
        font_size: f32,
        color: [f32; 4],
    ) -> Self {
        Self {
            content: content.into(),
            pos,
            font_size,
            line_height: None,
            max_width: None,
            color,
            anchor: UiTextAnchor::TopCenter,
        }
    }

    pub fn with_line_height(mut self, line_height: f32) -> Self {
        self.line_height = Some(line_height);
        self
    }

    pub fn with_max_width(mut self, max_width: f32) -> Self {
        self.max_width = Some(max_width.max(1.0));
        self
    }
}

#[derive(Clone, Copy)]
pub struct ActiveTextEdit<'a> {
    pub element_id: u64,
    pub content: &'a str,
}

#[derive(Clone, Debug, Default)]
pub struct TextElementRange {
    pub element_id: u64,
    pub generation: u64,
    pub was_active_edit: bool,
    pub element_pos: [f32; 2],
    pub element_rotation: f32,
    pub mono_start: u32,
    pub mono_end: u32,
    pub color_start: u32,
    pub color_end: u32,
}

#[derive(Default, Clone)]
pub struct PreparedTextDraw {
    pub mono_instances: Vec<TextInstanceData>,
    pub color_instances: Vec<TextInstanceData>,
    pub caret_pos: Option<Vec2>,
    pub element_ranges: Vec<TextElementRange>,
    element_range_index: HashMap<u64, u32>,
}

impl PreparedTextDraw {
    fn push_element_range(&mut self, range: TextElementRange) {
        let index = self.element_ranges.len() as u32;
        self.element_range_index.insert(range.element_id, index);
        self.element_ranges.push(range);
    }

    pub fn release_memory(&mut self) {
        self.mono_instances.clear();
        self.mono_instances.shrink_to_fit();
        self.color_instances.clear();
        self.color_instances.shrink_to_fit();
        self.element_ranges.clear();
        self.element_ranges.shrink_to_fit();
        self.element_range_index.clear();
        self.element_range_index.shrink_to_fit();
        self.caret_pos = None;
    }
}

/// Cached layout for a single element, keyed by element id.
struct CachedLayout {
    buffer: Buffer,
    world_min: Vec2,
    default_color: [f32; 4],
    /// Generation at which this layout was created.
    generation: u64,
}

pub struct TextSystem {
    font_system: FontSystem,
    swash_cache: SwashCache,
    mono_atlas: Atlas,
    emoji_atlas: Atlas,
    overlay_ready: bool,
    /// Single-entry layout cache for the actively-edited element.
    layout_cache: Option<(u64, CachedLayout)>,
    /// Monotonic counter for active-edit layout invalidation.
    edit_generation: u64,
}

pub struct ResolvedGlyph {
    pub kind: AtlasKind,
    pub entry: AtlasEntry,
}

#[derive(Clone, Copy)]
pub enum AtlasKind {
    Mono,
    Color,
}

fn inverse_rotate_point(element: &Element, point: Vec2) -> Vec2 {
    let center = element.pos + element.size * 0.5;
    let delta = point - center;
    let c = element.rotation.cos();
    let s = element.rotation.sin();
    center + Vec2::new(delta.x * c + delta.y * s, -delta.x * s + delta.y * c)
}

impl TextSystem {
    pub fn new() -> Self {
        let mut font_system = new_font_system();
        configure_bundled_font_defaults(&mut font_system);
        let mut text_system = Self {
            font_system,
            swash_cache: SwashCache::new(),
            mono_atlas: Atlas::new(TEXT_ATLAS_SIZE, true),
            emoji_atlas: Atlas::new(EMOJI_ATLAS_SIZE, false),
            overlay_ready: false,
            layout_cache: None,
            edit_generation: 0,
        };
        text_system.apply_browser_font_updates();
        text_system
    }

    /// Increment the edit generation counter, invalidating cached layouts
    /// for the actively-edited element.
    pub fn bump_edit_generation(&mut self) {
        self.edit_generation = self.edit_generation.wrapping_add(1);
    }

    /// Remove the cached layout if its element no longer exists.
    pub fn evict_stale_layouts(&mut self, live_ids: &HashSet<u64>) {
        if let Some((id, _)) = &self.layout_cache {
            if !live_ids.contains(id) {
                self.layout_cache = None;
            }
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn apply_browser_font_updates(&mut self) -> bool {
        let loaded_fonts = browser_io::take_loaded_fonts();
        if loaded_fonts.is_empty() {
            return false;
        }

        let db = self.font_system.db_mut();
        println!(
            "[font] applying {} browser-loaded fonts",
            loaded_fonts.len()
        );
        for bytes in loaded_fonts {
            let decoded = decode_browser_font_bytes(bytes);
            let Some(decoded) = decoded else {
                continue;
            };
            println!("[font] loading into fontdb bytes={}", decoded.len());
            db.load_font_source(fontdb::Source::Binary(Arc::new(decoded)));
        }

        self.reset_runtime_caches();
        println!("[font] fontdb updated; text runtime caches reset");
        true
    }

    fn reset_runtime_caches(&mut self) {
        self.swash_cache = SwashCache::new();
        self.mono_atlas = Atlas::new(TEXT_ATLAS_SIZE, true);
        self.emoji_atlas = Atlas::new(EMOJI_ATLAS_SIZE, false);
        self.overlay_ready = false;
        self.layout_cache = None;
    }

    pub fn hard_reset_runtime_caches(&mut self) {
        self.reset_runtime_caches();
    }

    pub fn measure_text_box(&mut self, content: &str, text: &TextData, max_width: f32) -> Vec2 {
        let padding = Vec2::splat(12.0);
        let usable_width = (max_width - padding.x * 2.0).max(1.0);
        let metrics = text_metrics(text.font_size, None);
        let attrs = default_text_attrs(content, text.color);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_size(&mut self.font_system, Some(usable_width), None);
        buffer.set_wrap(&mut self.font_system, Wrap::WordOrGlyph);
        buffer.set_text(
            &mut self.font_system,
            content,
            &attrs,
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut self.font_system, true);

        let mut widest_line = 0.0f32;
        let mut total_height = 0.0f32;
        let mut had_runs = false;
        for run in buffer.layout_runs() {
            had_runs = true;
            widest_line = widest_line.max(run.line_w);
            total_height = total_height.max(run.line_top + run.line_height);
        }

        if !had_runs {
            total_height = metrics.line_height.max(text.font_size + 4.0);
        }

        Vec2::new(
            (widest_line + padding.x * 2.0).clamp(96.0, max_width.max(96.0)),
            (total_height + padding.y * 2.0).max(text.font_size + padding.y * 2.0 + 4.0),
        )
    }

    pub fn measure_ui_text(&mut self, spec: &UiTextSpec) -> Vec2 {
        if spec.content.is_empty() {
            return Vec2::ZERO;
        }

        let metrics = text_metrics(spec.font_size, spec.line_height);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_size(&mut self.font_system, spec.max_width, None);
        buffer.set_wrap(
            &mut self.font_system,
            if spec.max_width.is_some() {
                Wrap::WordOrGlyph
            } else {
                Wrap::None
            },
        );
        let attrs = default_text_attrs(&spec.content, spec.color);
        buffer.set_text(
            &mut self.font_system,
            &spec.content,
            &attrs,
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut self.font_system, true);

        let mut size = Vec2::new(0.0, metrics.line_height);
        let mut had_runs = false;
        for run in buffer.layout_runs() {
            had_runs = true;
            size.x = size.x.max(run.line_w);
            size.y = size.y.max(run.line_top + run.line_height);
        }

        if had_runs {
            size
        } else {
            Vec2::new(0.0, metrics.line_height)
        }
    }

    pub fn build_ui_text_instances(
        &mut self,
        ctx: &mut dyn RenderingBackend,
        text_atlas: TextureId,
        emoji_atlas: TextureId,
        specs: &[UiTextSpec],
    ) -> PreparedTextDraw {
        self.ensure_overlay_pixel(ctx, text_atlas);

        let mut prepared = PreparedTextDraw::default();
        for spec in specs {
            if spec.content.is_empty() {
                continue;
            }

            let metrics = text_metrics(spec.font_size, spec.line_height);
            let attrs = default_text_attrs(&spec.content, spec.color);
            let mut buffer = Buffer::new(&mut self.font_system, metrics);
            buffer.set_size(&mut self.font_system, spec.max_width, None);
            buffer.set_wrap(
                &mut self.font_system,
                if spec.max_width.is_some() {
                    Wrap::WordOrGlyph
                } else {
                    Wrap::None
                },
            );
            buffer.set_text(
                &mut self.font_system,
                &spec.content,
                &attrs,
                Shaping::Advanced,
                None,
            );
            buffer.shape_until_scroll(&mut self.font_system, true);

            let glyph_data: Vec<(CacheKey, i32, i32, [f32; 4], f32)> = buffer
                .layout_runs()
                .flat_map(|run| {
                    let line_x = match spec.anchor {
                        UiTextAnchor::TopLeft => spec.pos.x,
                        UiTextAnchor::TopCenter => spec.pos.x - run.line_w * 0.5,
                    };
                    run.glyphs.iter().map(move |glyph| {
                        let physical = glyph.physical((0.0, run.line_y), 1.0);
                        let glyph_color = glyph
                            .color_opt
                            .map(cosmic_color_to_rgba)
                            .unwrap_or(spec.color);
                        (
                            physical.cache_key,
                            physical.x,
                            physical.y,
                            glyph_color,
                            line_x,
                        )
                    })
                })
                .collect();

            for (cache_key, phys_x, phys_y, glyph_color, line_x) in glyph_data {
                let Some(resolved) = self.resolve_glyph(ctx, text_atlas, emoji_atlas, cache_key)
                else {
                    continue;
                };

                let instance_color = match resolved.kind {
                    AtlasKind::Mono => glyph_color,
                    AtlasKind::Color => [1.0, 1.0, 1.0, glyph_color[3]],
                };
                let pos = Vec2::new(
                    line_x + (phys_x + resolved.entry.left) as f32,
                    spec.pos.y + (phys_y - resolved.entry.top) as f32,
                );
                let atlas_size = match resolved.kind {
                    AtlasKind::Mono => TEXT_ATLAS_SIZE,
                    AtlasKind::Color => EMOJI_ATLAS_SIZE,
                };
                let instance = TextInstanceData::new(
                    pos.to_array(),
                    [resolved.entry.width as f32, resolved.entry.height as f32],
                    spec.pos.to_array(),
                    0.0,
                    resolved.entry.uv_min(atlas_size as f32),
                    resolved.entry.uv_max(atlas_size as f32),
                    instance_color,
                    false,
                );

                match resolved.kind {
                    AtlasKind::Mono => prepared.mono_instances.push(instance),
                    AtlasKind::Color => prepared.color_instances.push(instance),
                }
            }
        }

        prepared
    }

    pub fn build_text_instances(
        &mut self,
        ctx: &mut dyn RenderingBackend,
        text_atlas: TextureId,
        emoji_atlas: TextureId,
        board: &Board,
        active_edit: Option<ActiveTextEdit<'_>>,
        mut prepared: PreparedTextDraw,
    ) -> PreparedTextDraw {
        self.ensure_overlay_pixel(ctx, text_atlas);

        let prev_mono_instances = std::mem::take(&mut prepared.mono_instances);
        let prev_color_instances = std::mem::take(&mut prepared.color_instances);
        let prev_element_ranges = std::mem::take(&mut prepared.element_ranges);
        let prev_element_range_index = std::mem::take(&mut prepared.element_range_index);

        prepared
            .mono_instances
            .reserve(prev_mono_instances.capacity());
        prepared
            .color_instances
            .reserve(prev_color_instances.capacity());
        prepared
            .element_ranges
            .reserve(prev_element_ranges.capacity());
        prepared
            .element_range_index
            .reserve(prev_element_range_index.len());
        prepared.caret_pos = None;
        for element in &board.elements {
            let content =
                if let Some(edit) = active_edit.filter(|edit| edit.element_id == element.id) {
                    edit.content
                } else {
                    element
                        .text
                        .as_ref()
                        .map(|text| text.content.as_str())
                        .unwrap_or_default()
                };

            let is_active_edit = active_edit
                .as_ref()
                .map(|edit| edit.element_id == element.id)
                .unwrap_or(false);

            if content.is_empty() && !is_active_edit {
                continue;
            }

            let mono_start = prepared.mono_instances.len();
            let color_start = prepared.color_instances.len();
            let is_active_edit = active_edit
                .as_ref()
                .map(|edit| edit.element_id == element.id)
                .unwrap_or(false);

            // Attempt to reuse from previous draw
            if !is_active_edit {
                if let Some(&prev_range_index) = prev_element_range_index.get(&element.id) {
                    let prev_range = &prev_element_ranges[prev_range_index as usize];
                    if prev_range.generation == element.text_layout_generation
                        && !prev_range.was_active_edit
                    {
                        let pos_diff = element.pos - Vec2::from(prev_range.element_pos);
                        let rot_diff = element.rotation - prev_range.element_rotation;

                        let new_selected = if element.selected { 1 } else { 0 };
                        let text_color = element
                            .text
                            .as_ref()
                            .map(|t| t.color)
                            .unwrap_or(palette::BLACK);
                        let new_color_u8 = [
                            (text_color[0] * 255.0) as u8,
                            (text_color[1] * 255.0) as u8,
                            (text_color[2] * 255.0) as u8,
                            (text_color[3] * 255.0) as u8,
                        ];

                        let origin_f32 = (element.pos + element.size * 0.5).to_array();
                        let origin_i16 = [origin_f32[0] as i16, origin_f32[1] as i16];

                        prepared.mono_instances.extend_from_slice(
                            &prev_mono_instances
                                [prev_range.mono_start as usize..prev_range.mono_end as usize],
                        );
                        prepared.color_instances.extend_from_slice(
                            &prev_color_instances
                                [prev_range.color_start as usize..prev_range.color_end as usize],
                        );

                        let mono_end = prepared.mono_instances.len();
                        let color_end = prepared.color_instances.len();

                        for inst in &mut prepared.mono_instances[mono_start..mono_end] {
                            if pos_diff != Vec2::ZERO || rot_diff != 0.0 {
                                inst.pos[0] += pos_diff.x;
                                inst.pos[1] += pos_diff.y;
                                inst.origin = origin_i16;
                                inst.rotation = element.rotation;
                            }
                            inst.selected = new_selected;
                            inst.color = new_color_u8;
                        }

                        for inst in &mut prepared.color_instances[color_start..color_end] {
                            if pos_diff != Vec2::ZERO || rot_diff != 0.0 {
                                inst.pos[0] += pos_diff.x;
                                inst.pos[1] += pos_diff.y;
                                inst.origin = origin_i16;
                                inst.rotation = element.rotation;
                            }
                            inst.selected = new_selected;
                            inst.color[3] = new_color_u8[3];
                        }

                        prepared.push_element_range(TextElementRange {
                            element_id: element.id,
                            generation: element.text_layout_generation,
                            was_active_edit: false,
                            element_pos: element.pos.to_array(),
                            element_rotation: element.rotation,
                            mono_start: mono_start as u32,
                            mono_end: mono_end as u32,
                            color_start: color_start as u32,
                            color_end: color_end as u32,
                        });
                        continue;
                    }
                }
            }

            let origin = (element.pos + element.size * 0.5).to_array();

            // Active edit element: use the persistent single-entry layout cache.
            // Idle elements: shape a temporary buffer and drop it immediately to
            // avoid keeping a Buffer per element in RAM.
            let (glyph_data, world_min): (Vec<(CacheKey, i32, i32, [f32; 4])>, Vec2) =
                if is_active_edit {
                    let generation = self.edit_generation;
                    if !self.ensure_layout_cached(element, content, generation) {
                        continue;
                    }
                    let cached = match &self.layout_cache {
                        Some((id, c)) if *id == element.id => c,
                        _ => continue,
                    };
                    let world_min = cached.world_min;
                    let default_color = cached.default_color;
                    let glyph_data = cached
                        .buffer
                        .layout_runs()
                        .flat_map(|run| {
                            run.glyphs.iter().map(move |glyph| {
                                let physical = glyph.physical((0.0, run.line_y), 1.0);
                                let glyph_color = glyph
                                    .color_opt
                                    .map(cosmic_color_to_rgba)
                                    .unwrap_or(default_color);
                                (physical.cache_key, physical.x, physical.y, glyph_color)
                            })
                        })
                        .collect();
                    (glyph_data, world_min)
                } else {
                    // Shape a temporary buffer — no cache write.
                    let Some((world_min, world_max)) = element.text_bounds() else {
                        continue;
                    };
                    let default_text = TextData::default();
                    let text = element.text.as_ref().unwrap_or(&default_text);
                    let width = (world_max.x - world_min.x).max(1.0);
                    let height = (world_max.y - world_min.y).max(1.0);
                    let metrics = text_metrics(text.font_size, None);
                    let attrs = default_text_attrs(content, text.color);
                    let default_color = text.color;
                    let mut buffer = Buffer::new(&mut self.font_system, metrics);
                    buffer.set_size(&mut self.font_system, Some(width), Some(height));
                    buffer.set_wrap(&mut self.font_system, Wrap::WordOrGlyph);
                    buffer.set_text(
                        &mut self.font_system,
                        content,
                        &attrs,
                        Shaping::Advanced,
                        None,
                    );
                    buffer.shape_until_scroll(&mut self.font_system, true);
                    for run in buffer.layout_runs() {
                        for glyph in run.glyphs.iter() {
                            let physical = glyph.physical((0.0, run.line_y), 1.0);
                            let glyph_color = glyph
                                .color_opt
                                .map(cosmic_color_to_rgba)
                                .unwrap_or(default_color);
                            let Some(resolved) = self.resolve_glyph(
                                ctx,
                                text_atlas,
                                emoji_atlas,
                                physical.cache_key,
                            ) else {
                                continue;
                            };

                            let instance_color = match resolved.kind {
                                AtlasKind::Mono => glyph_color,
                                AtlasKind::Color => [1.0, 1.0, 1.0, glyph_color[3]],
                            };

                            let pos = world_min
                                + Vec2::new(
                                    (physical.x + resolved.entry.left) as f32,
                                    (physical.y - resolved.entry.top) as f32,
                                );

                            let atlas_size = match resolved.kind {
                                AtlasKind::Mono => TEXT_ATLAS_SIZE,
                                AtlasKind::Color => EMOJI_ATLAS_SIZE,
                            };

                            let instance = TextInstanceData::new(
                                pos.to_array(),
                                [resolved.entry.width as f32, resolved.entry.height as f32],
                                origin,
                                element.rotation,
                                resolved.entry.uv_min(atlas_size as f32),
                                resolved.entry.uv_max(atlas_size as f32),
                                instance_color,
                                element.selected,
                            );

                            match resolved.kind {
                                AtlasKind::Mono => prepared.mono_instances.push(instance),
                                AtlasKind::Color => prepared.color_instances.push(instance),
                            }
                        }
                    }
                    prepared.push_element_range(TextElementRange {
                        element_id: element.id,
                        generation: element.text_layout_generation,
                        was_active_edit: false,
                        element_pos: element.pos.to_array(),
                        element_rotation: element.rotation,
                        mono_start: mono_start as u32,
                        mono_end: prepared.mono_instances.len() as u32,
                        color_start: color_start as u32,
                        color_end: prepared.color_instances.len() as u32,
                    });
                    continue;
                };

            for (cache_key, phys_x, phys_y, glyph_color) in glyph_data {
                let resolved = self.resolve_glyph(ctx, text_atlas, emoji_atlas, cache_key);
                let Some(resolved) = resolved else {
                    continue;
                };

                let instance_color = match resolved.kind {
                    AtlasKind::Mono => glyph_color,
                    AtlasKind::Color => [1.0, 1.0, 1.0, glyph_color[3]],
                };

                let pos = world_min
                    + Vec2::new(
                        (phys_x + resolved.entry.left) as f32,
                        (phys_y - resolved.entry.top) as f32,
                    );

                let atlas_size = match resolved.kind {
                    AtlasKind::Mono => TEXT_ATLAS_SIZE,
                    AtlasKind::Color => EMOJI_ATLAS_SIZE,
                };

                let instance = TextInstanceData::new(
                    pos.to_array(),
                    [resolved.entry.width as f32, resolved.entry.height as f32],
                    origin,
                    element.rotation,
                    resolved.entry.uv_min(atlas_size as f32),
                    resolved.entry.uv_max(atlas_size as f32),
                    instance_color,
                    element.selected,
                );

                match resolved.kind {
                    AtlasKind::Mono => prepared.mono_instances.push(instance),
                    AtlasKind::Color => prepared.color_instances.push(instance),
                }
            }

            prepared.push_element_range(TextElementRange {
                element_id: element.id,
                generation: element.text_layout_generation,
                was_active_edit: is_active_edit,
                element_pos: element.pos.to_array(),
                element_rotation: element.rotation,
                mono_start: mono_start as u32,
                mono_end: prepared.mono_instances.len() as u32,
                color_start: color_start as u32,
                color_end: prepared.color_instances.len() as u32,
            });
        }

        prepared.caret_pos = None;
        prepared
    }

    pub fn hit_test_cursor(
        &mut self,
        element: &Element,
        is_active_edit: bool,
        content: &str,
        line_offsets: Option<&LineOffsets>,
        world_pos: Vec2,
    ) -> Option<usize> {
        let generation = if is_active_edit {
            self.edit_generation
        } else {
            element.text_layout_generation
        };
        if !self.ensure_layout_cached(element, content, generation) {
            return None;
        }
        let (_, cached) = self.layout_cache.as_ref()?;
        let local = inverse_rotate_point(element, world_pos) - cached.world_min;
        let cursor = cached.buffer.hit(local.x, local.y)?;
        Some(match line_offsets {
            Some(offsets) => offsets.cursor_to_byte(content, cursor),
            None => cursor_to_global_byte(content, cursor),
        })
    }

    pub fn move_cursor(
        &mut self,
        element: &Element,
        content: &str,
        line_offsets: Option<&LineOffsets>,
        cursor_byte: usize,
        preferred_x: Option<i32>,
        motion: Motion,
    ) -> Option<(usize, Option<i32>)> {
        let generation = self.edit_generation;
        if !self.ensure_layout_cached(element, content, generation) {
            return None;
        }
        let (_, cached) = self.layout_cache.as_mut()?;
        let cursor = match line_offsets {
            Some(offsets) => offsets.byte_to_cursor(content, cursor_byte),
            None => global_byte_to_cursor(content, cursor_byte),
        };
        let (next, next_preferred_x) =
            cached
                .buffer
                .cursor_motion(&mut self.font_system, cursor, preferred_x, motion)?;
        let next_byte = match line_offsets {
            Some(offsets) => offsets.cursor_to_byte(content, next),
            None => cursor_to_global_byte(content, next),
        };
        Some((next_byte, next_preferred_x))
    }

    pub fn build_edit_overlay_instances(
        &mut self,
        element: &Element,
        content: &str,
        line_offsets: &LineOffsets,
        cursor_byte: usize,
        selection_anchor_byte: Option<usize>,
    ) -> (Vec<TextInstanceData>, Option<Vec2>) {
        let generation = self.edit_generation;
        if !self.ensure_layout_cached(element, content, generation) {
            return (Vec::new(), None);
        }

        let uv_min = [0.0, 0.0];
        let uv_max = [1.0 / TEXT_ATLAS_SIZE as f32, 1.0 / TEXT_ATLAS_SIZE as f32];
        let origin = (element.pos + element.size * 0.5).to_array();
        let mut instances = Vec::new();
        let mut caret_pos = None;

        let (_, cached) = self.layout_cache.as_ref().unwrap();

        if let Some((start_byte, end_byte)) = selection_range(cursor_byte, selection_anchor_byte) {
            let start = line_offsets.byte_to_cursor(content, start_byte);
            let end = line_offsets.byte_to_cursor(content, end_byte);
            for run in cached.buffer.layout_runs() {
                if let Some((x, width)) = run.highlight(start, end) {
                    if width <= 0.0 {
                        continue;
                    }
                    instances.push(TextInstanceData::new(
                        (cached.world_min + Vec2::new(x, run.line_top)).to_array(),
                        [width, run.line_height],
                        origin,
                        element.rotation,
                        uv_min,
                        uv_max,
                        SELECTION_COLOR,
                        element.selected,
                    ));
                }
            }
        }

        let cursor = line_offsets.byte_to_cursor(content, cursor_byte);
        if let Some((x, line_top, line_height)) = caret_geometry(&cached.buffer, cursor) {
            let world_pos = cached.world_min + Vec2::new((x - 1.0).max(0.0), line_top);
            instances.push(TextInstanceData::new(
                world_pos.to_array(),
                [2.0, line_height.max(1.0)],
                origin,
                element.rotation,
                uv_min,
                uv_max,
                CARET_COLOR,
                element.selected,
            ));
            caret_pos = Some(world_pos);
        }

        (instances, caret_pos)
    }

    /// Ensure a layout is cached for the given element. Returns true if the
    /// cache entry exists (hit or freshly inserted), false if layout failed.
    /// Overwrites the single cache slot when a miss occurs.
    fn ensure_layout_cached(&mut self, element: &Element, content: &str, generation: u64) -> bool {
        // Check cache hit
        if let Some((id, ref cached)) = self.layout_cache {
            if id == element.id && cached.generation == generation {
                return true;
            }
        }

        // Cache miss — do full shaping
        // println!("[text] Case 2 Compute layout for element id={}", element.id);
        let Some((world_min, world_max)) = element.text_bounds() else {
            return false;
        };
        let default_text = TextData::default();
        let text = element.text.as_ref().unwrap_or(&default_text);
        let width = (world_max.x - world_min.x).max(1.0);
        let height = (world_max.y - world_min.y).max(1.0);

        let metrics = text_metrics(text.font_size, None);
        let attrs = default_text_attrs(content, text.color);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_size(&mut self.font_system, Some(width), Some(height));
        buffer.set_wrap(&mut self.font_system, Wrap::WordOrGlyph);

        buffer.set_text(
            &mut self.font_system,
            content,
            &attrs,
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut self.font_system, true);

        // Overwrite the single cache slot
        self.layout_cache = Some((
            element.id,
            CachedLayout {
                buffer,
                world_min,
                default_color: text.color,
                generation,
            },
        ));

        true
    }

    fn resolve_glyph(
        &mut self,
        ctx: &mut dyn RenderingBackend,
        mono_texture: TextureId,
        emoji_texture: TextureId,
        cache_key: CacheKey,
    ) -> Option<ResolvedGlyph> {
        if let Some(entry) = self.mono_atlas.entries.get(&cache_key) {
            return Some(ResolvedGlyph {
                kind: AtlasKind::Mono,
                entry: *entry,
            });
        }
        if let Some(entry) = self.emoji_atlas.entries.get(&cache_key) {
            return Some(ResolvedGlyph {
                kind: AtlasKind::Color,
                entry: *entry,
            });
        }

        let image = self
            .swash_cache
            .get_image(&mut self.font_system, cache_key)
            .as_ref()?
            .clone();

        match image.content {
            SwashContent::Mask | SwashContent::SubpixelMask => {
                let entry = self
                    .mono_atlas
                    .insert(ctx, mono_texture, cache_key, &image)?;
                Some(ResolvedGlyph {
                    kind: AtlasKind::Mono,
                    entry,
                })
            }
            SwashContent::Color => {
                let entry = self
                    .emoji_atlas
                    .insert(ctx, emoji_texture, cache_key, &image)?;
                Some(ResolvedGlyph {
                    kind: AtlasKind::Color,
                    entry,
                })
            }
        }
    }

    fn ensure_overlay_pixel(&mut self, ctx: &mut dyn RenderingBackend, text_atlas: TextureId) {
        if self.overlay_ready {
            return;
        }

        // 1×1 solid pixel at (0, 0) — used for selection highlights and the caret.
        ctx.texture_update_part(text_atlas, 0, 0, 1, 1, &[255]);

        // Solid FALLBACK_GLYPH_SIZE×FALLBACK_GLYPH_SIZE block — the ■ shown when
        // the atlas overflows and a glyph cannot be cached.
        let fb = FALLBACK_GLYPH_SIZE;
        let fb_data = vec![255u8; fb * fb];
        ctx.texture_update_part(
            text_atlas,
            (1 + ATLAS_GAP) as i32,
            0,
            fb as i32,
            fb as i32,
            &fb_data,
        );

        self.overlay_ready = true;
    }
}
