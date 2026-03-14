use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, RgbaImage};
use miniquad::{FilterMode, RenderingBackend, TextureAccess, TextureFormat, TextureId, TextureParams, TextureSource, TextureWrap};

use crate::board::ImageData;
use crate::renderer::{ImageInstanceData, PreparedImageDraw};

pub const BASE_IMAGE_MAX_DIMENSION: u32 = 512;
pub const HIRES_IMAGE_MAX_DIMENSION: u32 = 2048;
pub const HIRES_SCREEN_FRACTION: f32 = 0.8;
pub const THUMB_ZOOM_THRESHOLD: f32 = 0.1;

const MAX_RAM_BYTES: usize = 128 * 1024 * 1024;
const MAX_GPU_BYTES: usize = 64 * 1024 * 1024;
const THUMB_ATLAS_SIZE: u32 = 1024;
const THUMB_SIZE: u32 = 64;
const THUMB_SLOTS_PER_ROW: usize = (THUMB_ATLAS_SIZE / THUMB_SIZE) as usize;
const THUMB_SLOT_COUNT: usize = THUMB_SLOTS_PER_ROW * THUMB_SLOTS_PER_ROW;

const BASE_WEBP_QUALITY: f32 = 72.0;
const HIRES_WEBP_QUALITY: f32 = 84.0;

#[derive(Debug)]
pub enum ImageImportError {
    #[cfg(target_arch = "wasm32")]
    UnsupportedPlatform,
    Io(std::io::Error),
    Decode(image::ImageError),
}

impl fmt::Display for ImageImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(target_arch = "wasm32")]
            ImageImportError::UnsupportedPlatform => {
                write!(f, "image import is only implemented for native desktop builds")
            }
            ImageImportError::Io(err) => write!(f, "{err}"),
            ImageImportError::Decode(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for ImageImportError {}

impl From<std::io::Error> for ImageImportError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<image::ImageError> for ImageImportError {
    fn from(value: image::ImageError) -> Self {
        Self::Decode(value)
    }
}

pub struct ImportedImage {
    pub data: ImageData,
    pub display_size: [f32; 2],
}

#[derive(Clone)]
struct DecodedImage {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl DecodedImage {
    fn byte_len(&self) -> usize {
        self.rgba.len()
    }
}

struct RamEntry {
    image: DecodedImage,
    bytes: usize,
}

#[derive(Clone, Copy)]
struct GpuEntry {
    texture: TextureId,
    bytes: usize,
}

#[derive(Clone, Copy)]
struct AtlasEntry {
    uv_min: [f32; 2],
    uv_max: [f32; 2],
}

pub struct ImageManager {
    asset_root: PathBuf,
    ram_cache: HashMap<String, RamEntry>,
    ram_lru: Vec<String>,
    ram_used_bytes: usize,
    gpu_cache: HashMap<String, GpuEntry>,
    gpu_lru: Vec<String>,
    gpu_used_bytes: usize,
    atlas_texture: TextureId,
    atlas_entries: HashMap<String, AtlasEntry>,
    atlas_slot_owner: Vec<Option<String>>,
    atlas_next_slot: usize,
    missing_texture: TextureId,
}

impl ImageManager {
    pub fn new(ctx: &mut dyn RenderingBackend, asset_root: PathBuf) -> Self {
        Self {
            asset_root,
            ram_cache: HashMap::new(),
            ram_lru: Vec::new(),
            ram_used_bytes: 0,
            gpu_cache: HashMap::new(),
            gpu_lru: Vec::new(),
            gpu_used_bytes: 0,
            atlas_texture: create_thumb_atlas(ctx),
            atlas_entries: HashMap::new(),
            atlas_slot_owner: vec![None; THUMB_SLOT_COUNT],
            atlas_next_slot: 0,
            missing_texture: create_missing_texture(ctx),
        }
    }

    pub fn set_asset_root(&mut self, ctx: &mut dyn RenderingBackend, asset_root: PathBuf) {
        if self.asset_root == asset_root {
            return;
        }

        self.asset_root = asset_root;
        self.clear_runtime_caches(ctx);
    }

    pub fn import_from_source(&mut self, element_id: u64, source_path: &Path) -> Result<ImportedImage, ImageImportError> {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = element_id;
            let _ = source_path;
            return Err(ImageImportError::UnsupportedPlatform);
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let decoded = image::open(source_path)?;
            let (original_width, original_height) = decoded.dimensions();
            let base_image = resize_to_limit(&decoded, BASE_IMAGE_MAX_DIMENSION);
            let (base_width, base_height) = base_image.dimensions();

            let images_dir = self.asset_root.join("images");
            std::fs::create_dir_all(&images_dir)?;

            let asset_path = format!("images/image_{element_id}.webp");
            write_webp(&base_image, &self.asset_root.join(&asset_path), BASE_WEBP_QUALITY)?;
            self.seed_ram(
                asset_path.clone(),
                decoded_from_image(&base_image),
            );

            let hires_asset_path = if original_width.max(original_height) > BASE_IMAGE_MAX_DIMENSION {
                let hires_image = resize_to_limit(&decoded, HIRES_IMAGE_MAX_DIMENSION);
                let hires_asset_path = format!("images/image_{element_id}_hires.webp");
                write_webp(
                    &hires_image,
                    &self.asset_root.join(&hires_asset_path),
                    HIRES_WEBP_QUALITY,
                )?;
                self.seed_ram(
                    hires_asset_path.clone(),
                    decoded_from_image(&hires_image),
                );
                Some(hires_asset_path)
            } else {
                None
            };

            Ok(ImportedImage {
                data: ImageData {
                    asset_path,
                    hires_asset_path,
                    original_width,
                    original_height,
                    base_width,
                    base_height,
                },
                display_size: [base_width as f32, base_height as f32],
            })
        }
    }

    pub fn prepare_draw(
        &mut self,
        ctx: &mut dyn RenderingBackend,
        image: &ImageData,
        pos: [f32; 2],
        size: [f32; 2],
        rotation: f32,
        zoom: f32,
        screen_extent: [f32; 2],
        viewport_size: [f32; 2],
    ) -> PreparedImageDraw {
        if zoom < THUMB_ZOOM_THRESHOLD {
            if let Some(entry) = self.ensure_thumb_entry(ctx, &image.asset_path) {
                return PreparedImageDraw {
                    texture: self.atlas_texture,
                    instance: ImageInstanceData::new(
                        pos,
                        size,
                        [pos[0] + size[0] * 0.5, pos[1] + size[1] * 0.5],
                        rotation,
                        entry.uv_min,
                        entry.uv_max,
                        [1.0, 1.0, 1.0, 1.0],
                    ),
                };
            }
        }

        let prefer_hires = image
            .hires_asset_path
            .as_ref()
            .filter(|_| screen_extent[0] > viewport_size[0] * HIRES_SCREEN_FRACTION || screen_extent[1] > viewport_size[1] * HIRES_SCREEN_FRACTION)
            .map(String::as_str);

        let texture = prefer_hires
            .and_then(|path| {
                if !self.gpu_cache.contains_key(path) {
                    println!("[image] HIRES trigger {} screen=({:.0},{:.0}) viewport=({:.0},{:.0})", path, screen_extent[0], screen_extent[1], viewport_size[0], viewport_size[1]);
                }
                self.load_gpu_texture(ctx, path)
            })
            .or_else(|| self.load_gpu_texture(ctx, &image.asset_path))
            .unwrap_or(self.missing_texture);

        PreparedImageDraw {
            texture,
            instance: ImageInstanceData::new(
                pos,
                size,
                [pos[0] + size[0] * 0.5, pos[1] + size[1] * 0.5],
                rotation,
                [0.0, 0.0],
                [1.0, 1.0],
                [1.0, 1.0, 1.0, 1.0],
            ),
        }
    }

    pub fn atlas_count(&self) -> usize {
        self.atlas_entries.len()
    }

    pub fn atlas_capacity(&self) -> usize {
        THUMB_SLOT_COUNT
    }

    pub fn ram_used_bytes(&self) -> usize {
        self.ram_used_bytes
    }

    pub fn ram_capacity_bytes(&self) -> usize {
        MAX_RAM_BYTES
    }

    pub fn gpu_used_bytes(&self) -> usize {
        self.gpu_used_bytes
    }

    pub fn gpu_capacity_bytes(&self) -> usize {
        MAX_GPU_BYTES
    }

    fn clear_runtime_caches(&mut self, ctx: &mut dyn RenderingBackend) {
        self.ram_cache.clear();
        self.ram_lru.clear();
        self.ram_used_bytes = 0;

        let textures: Vec<TextureId> = self.gpu_cache.values().map(|entry| entry.texture).collect();
        for texture in textures {
            ctx.delete_texture(texture);
        }
        self.gpu_cache.clear();
        self.gpu_lru.clear();
        self.gpu_used_bytes = 0;

        self.atlas_entries.clear();
        self.atlas_slot_owner.fill(None);
        self.atlas_next_slot = 0;
    }

    fn load_gpu_texture(&mut self, ctx: &mut dyn RenderingBackend, relative_path: &str) -> Option<TextureId> {
        if let Some(entry) = self.gpu_cache.get(relative_path).copied() {
            touch_lru(&mut self.gpu_lru, relative_path);
            return Some(entry.texture);
        }

        let decoded = self.load_ram_image(relative_path)?;
        let bytes = decoded.byte_len();
        self.evict_gpu_if_needed(ctx, bytes);
        let texture = ctx.new_texture(
            TextureAccess::Static,
            TextureSource::Bytes(&decoded.rgba),
            TextureParams {
                width: decoded.width,
                height: decoded.height,
                format: TextureFormat::RGBA8,
                wrap: TextureWrap::Clamp,
                min_filter: FilterMode::Linear,
                mag_filter: FilterMode::Linear,
                ..Default::default()
            },
        );
        self.gpu_cache
            .insert(relative_path.to_string(), GpuEntry { texture, bytes });
        touch_lru(&mut self.gpu_lru, relative_path);
        self.gpu_used_bytes += bytes;
        Some(texture)
    }

    fn load_ram_image(&mut self, relative_path: &str) -> Option<DecodedImage> {
        if let Some(entry) = self.ram_cache.get(relative_path) {
            touch_lru(&mut self.ram_lru, relative_path);
            return Some(entry.image.clone());
        }

        let full_path = self.asset_root.join(relative_path);
        let decoded = image::open(&full_path).ok()?;
        let decoded = decoded_from_image(&decoded);
        self.seed_ram(relative_path.to_string(), decoded.clone());
        Some(decoded)
    }

    fn seed_ram(&mut self, key: String, image: DecodedImage) {
        if let Some(previous) = self.ram_cache.remove(&key) {
            self.ram_used_bytes = self.ram_used_bytes.saturating_sub(previous.bytes);
            remove_from_lru(&mut self.ram_lru, &key);
        }

        let bytes = image.byte_len();
        while !self.ram_lru.is_empty() && self.ram_used_bytes + bytes > MAX_RAM_BYTES {
            let evict_key = self.ram_lru.pop().unwrap();
            if let Some(entry) = self.ram_cache.remove(&evict_key) {
                self.ram_used_bytes = self.ram_used_bytes.saturating_sub(entry.bytes);
            }
        }

        self.ram_used_bytes += bytes;
        self.ram_cache.insert(key.clone(), RamEntry { image, bytes });
        touch_lru(&mut self.ram_lru, &key);
    }

    fn evict_gpu_if_needed(&mut self, ctx: &mut dyn RenderingBackend, new_bytes: usize) {
        while !self.gpu_lru.is_empty() && self.gpu_used_bytes + new_bytes > MAX_GPU_BYTES {
            let evict_key = self.gpu_lru.pop().unwrap();
            if let Some(entry) = self.gpu_cache.remove(&evict_key) {
                self.gpu_used_bytes = self.gpu_used_bytes.saturating_sub(entry.bytes);
                ctx.delete_texture(entry.texture);
            }
        }
    }

    fn ensure_thumb_entry(&mut self, ctx: &mut dyn RenderingBackend, relative_path: &str) -> Option<AtlasEntry> {
        if let Some(entry) = self.atlas_entries.get(relative_path).copied() {
            return Some(entry);
        }

        let decoded = self.load_ram_image(relative_path)?;
        let thumb = build_thumbnail_rgba(&decoded)?;

        let slot = self.atlas_next_slot;
        self.atlas_next_slot = (self.atlas_next_slot + 1) % THUMB_SLOT_COUNT;
        if let Some(previous_owner) = self.atlas_slot_owner[slot].replace(relative_path.to_string()) {
            self.atlas_entries.remove(&previous_owner);
        }

        let col = (slot % THUMB_SLOTS_PER_ROW) as u32;
        let row = (slot / THUMB_SLOTS_PER_ROW) as u32;
        let x = col * THUMB_SIZE;
        let y = row * THUMB_SIZE;
        ctx.texture_update_part(
            self.atlas_texture,
            x as i32,
            y as i32,
            THUMB_SIZE as i32,
            THUMB_SIZE as i32,
            &thumb,
        );

        let atlas_size = THUMB_ATLAS_SIZE as f32;
        let entry = AtlasEntry {
            uv_min: [x as f32 / atlas_size, y as f32 / atlas_size],
            uv_max: [(x + THUMB_SIZE) as f32 / atlas_size, (y + THUMB_SIZE) as f32 / atlas_size],
        };
        self.atlas_entries.insert(relative_path.to_string(), entry);
        Some(entry)
    }
}

fn resize_to_limit(image: &DynamicImage, max_dimension: u32) -> DynamicImage {
    let (width, height) = image.dimensions();
    if width.max(height) <= max_dimension {
        return image.clone();
    }

    image.resize(max_dimension, max_dimension, FilterType::CatmullRom)
}

fn write_webp(image: &DynamicImage, path: &Path, quality: f32) -> Result<(), ImageImportError> {
    let rgba = image.to_rgba8();
    let (width, height) = image.dimensions();
    let encoded = webp::Encoder::from_rgba(rgba.as_raw(), width, height).encode(quality);
    std::fs::write(path, encoded.as_ref())?;
    Ok(())
}

fn decoded_from_image(image: &DynamicImage) -> DecodedImage {
    let rgba = image.to_rgba8();
    DecodedImage {
        width: image.width(),
        height: image.height(),
        rgba: rgba.into_raw(),
    }
}

fn build_thumbnail_rgba(decoded: &DecodedImage) -> Option<Vec<u8>> {
    let source = RgbaImage::from_raw(decoded.width, decoded.height, decoded.rgba.clone())?;
    let scaled = DynamicImage::ImageRgba8(source)
        .resize(THUMB_SIZE, THUMB_SIZE, FilterType::Triangle)
        .to_rgba8();
    let mut canvas = vec![0u8; (THUMB_SIZE * THUMB_SIZE * 4) as usize];
    let offset_x = ((THUMB_SIZE - scaled.width()) / 2) as usize;
    let offset_y = ((THUMB_SIZE - scaled.height()) / 2) as usize;
    let scaled_width = scaled.width() as usize;
    let scaled_height = scaled.height() as usize;
    let scaled_raw = scaled.into_raw();

    for row in 0..scaled_height {
        let dst_row = offset_y + row;
        let src_start = row * scaled_width * 4;
        let src_end = src_start + scaled_width * 4;
        let dst_start = (dst_row * THUMB_SIZE as usize + offset_x) * 4;
        let dst_end = dst_start + scaled_width * 4;
        if dst_end <= canvas.len() && src_end <= scaled_raw.len() {
            canvas[dst_start..dst_end].copy_from_slice(&scaled_raw[src_start..src_end]);
        }
    }

    Some(canvas)
}

fn touch_lru(order: &mut Vec<String>, key: &str) {
    remove_from_lru(order, key);
    order.insert(0, key.to_string());
}

fn remove_from_lru(order: &mut Vec<String>, key: &str) {
    if let Some(index) = order.iter().position(|entry| entry == key) {
        order.remove(index);
    }
}

fn create_missing_texture(ctx: &mut dyn RenderingBackend) -> TextureId {
    let pixels: [u8; 64] = [
        255, 0, 255, 255, 40, 40, 40, 255, 255, 0, 255, 255, 40, 40, 40, 255,
        40, 40, 40, 255, 255, 0, 255, 255, 40, 40, 40, 255, 255, 0, 255, 255,
        255, 0, 255, 255, 40, 40, 40, 255, 255, 0, 255, 255, 40, 40, 40, 255,
        40, 40, 40, 255, 255, 0, 255, 255, 40, 40, 40, 255, 255, 0, 255, 255,
    ];

    ctx.new_texture(
        TextureAccess::Static,
        TextureSource::Bytes(&pixels),
        TextureParams {
            width: 4,
            height: 4,
            format: TextureFormat::RGBA8,
            wrap: TextureWrap::Clamp,
            min_filter: FilterMode::Nearest,
            mag_filter: FilterMode::Nearest,
            ..Default::default()
        },
    )
}

fn create_thumb_atlas(ctx: &mut dyn RenderingBackend) -> TextureId {
    ctx.new_texture(
        TextureAccess::Static,
        TextureSource::Bytes(&vec![0u8; (THUMB_ATLAS_SIZE * THUMB_ATLAS_SIZE * 4) as usize]),
        TextureParams {
            width: THUMB_ATLAS_SIZE,
            height: THUMB_ATLAS_SIZE,
            format: TextureFormat::RGBA8,
            wrap: TextureWrap::Clamp,
            min_filter: FilterMode::Linear,
            mag_filter: FilterMode::Linear,
            ..Default::default()
        },
    )
}