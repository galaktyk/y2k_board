use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView};
use miniquad::{
    Bindings, BufferLayout, BufferSource, BufferType, BufferUsage, FilterMode,
    MipmapFilterMode, PassAction, Pipeline, PipelineParams, RenderPass, RenderingBackend,
    ShaderMeta, ShaderSource, TextureAccess, TextureFormat, TextureId, TextureParams,
    TextureSource, TextureWrap, UniformBlockLayout, VertexAttribute, VertexFormat,
};

use crate::board::ImageData;
use crate::platform::image_streaming::PlatformImageStreamingAdapter;
use crate::rendering::renderer::{ImageInstanceData, PreparedImageDraw};

pub const BASE_IMAGE_MAX_DIMENSION: u32 = 256;
pub const HIRES_IMAGE_MAX_DIMENSION: u32 = 1024;
pub const HIRES_SCREEN_FRACTION: f32 = 0.5;
pub const THUMB_ZOOM_THRESHOLD: f32 = 0.2;

const MAX_RAM_BYTES: usize = 128 * 1024 * 1024;
const MAX_GPU_BYTES: usize = 64 * 1024 * 1024;
const THUMB_ATLAS_SIZE: u32 = 1024;
const THUMB_SIZE: u32 = 32;
const THUMB_SLOTS_PER_ROW: usize = (THUMB_ATLAS_SIZE / THUMB_SIZE) as usize;
const THUMB_SLOT_COUNT: usize = THUMB_SLOTS_PER_ROW * THUMB_SLOTS_PER_ROW;

const BASE_WEBP_QUALITY: f32 = 40.0;
const HIRES_WEBP_QUALITY: f32 = 40.0;




#[derive(Debug)]
pub enum ImageImportError {
    #[cfg(target_arch = "wasm32")]
    UnsupportedPlatform,
    InvalidData(&'static str),
    Io(std::io::Error),
    Decode(image::ImageError),
}

impl fmt::Display for ImageImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(target_arch = "wasm32")]
            ImageImportError::UnsupportedPlatform => {
                write!(f, "web image streaming is TODO; image import is only implemented for native desktop builds")
            }
            ImageImportError::InvalidData(message) => write!(f, "{message}"),
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

#[derive(Clone, Copy, Debug, Default)]
pub struct ImageRamClearStats {
    pub entries_cleared: usize,
    pub bytes_freed: usize,
}

#[derive(Clone)]
pub(crate) struct DecodedImage {
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

#[repr(C)]
#[derive(Clone, Copy)]
struct AtlasBlitVertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

pub struct ImageManager {
    asset_root: PathBuf,
    streaming_adapter: PlatformImageStreamingAdapter,
    ram_cache: HashMap<String, RamEntry>,
    ram_lru: Vec<String>,
    ram_used_bytes: usize,
    gpu_cache: HashMap<String, GpuEntry>,
    gpu_lru: Vec<String>,
    gpu_used_bytes: usize,
    atlas_texture: TextureId,
    atlas_pass: RenderPass,
    atlas_blit_pipeline: Pipeline,
    atlas_blit_bindings: Bindings,
    atlas_entries: HashMap<String, AtlasEntry>,
    atlas_slot_owner: Vec<Option<String>>,
    atlas_next_slot: usize,
    thumb_placeholder_texture: TextureId,
    missing_texture: TextureId,
}

impl ImageManager {
    pub fn new(ctx: &mut dyn RenderingBackend, asset_root: PathBuf) -> Self {
        let missing_texture = create_missing_texture(ctx);
        let thumb_placeholder_texture = create_thumb_placeholder_texture(ctx);
        let atlas_texture = create_thumb_atlas(ctx);
        let atlas_pass = ctx.new_render_pass(atlas_texture, None);
        clear_atlas_texture(ctx, atlas_pass);
        let (atlas_blit_pipeline, atlas_blit_bindings) =
            create_atlas_blit_resources(ctx, missing_texture);

        Self {
            streaming_adapter: PlatformImageStreamingAdapter::new(asset_root.clone()),
            asset_root,
            ram_cache: HashMap::new(),
            ram_lru: Vec::new(),
            ram_used_bytes: 0,
            gpu_cache: HashMap::new(),
            gpu_lru: Vec::new(),
            gpu_used_bytes: 0,
            atlas_texture,
            atlas_pass,
            atlas_blit_pipeline,
            atlas_blit_bindings,
            atlas_entries: HashMap::new(),
            atlas_slot_owner: vec![None; THUMB_SLOT_COUNT],
            atlas_next_slot: 0,
            thumb_placeholder_texture,
            missing_texture,
        }
    }

    pub fn set_asset_root(&mut self, ctx: &mut dyn RenderingBackend, asset_root: PathBuf) {
        if self.asset_root == asset_root {
            return;
        }

        self.asset_root = asset_root;
        self.streaming_adapter.set_asset_root(self.asset_root.clone());
        self.clear_runtime_caches(ctx);
    }

    pub fn import_from_source(&mut self, source_path: &Path) -> Result<ImportedImage, ImageImportError> {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = source_path;
            return Err(ImageImportError::UnsupportedPlatform);
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let bytes = std::fs::read(source_path)?;
            let hash = fnv1a_hash(&bytes);
            let decoded = image::load_from_memory(&bytes)?;
            self.import_from_image(&hash, &decoded)
        }
    }

    pub fn import_from_bytes(&mut self, bytes: &[u8]) -> Result<ImportedImage, ImageImportError> {
        let hash = fnv1a_hash(bytes);
        let decoded = image::load_from_memory(bytes)?;
        self.import_from_image(&hash, &decoded)
    }

    pub fn import_from_rgba(
        &mut self,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    ) -> Result<ImportedImage, ImageImportError> {
        let hash = fnv1a_hash(&rgba);
        let image = image::RgbaImage::from_raw(width, height, rgba)
            .ok_or(ImageImportError::InvalidData("clipboard image data had an invalid RGBA size"))?;
        let decoded = DynamicImage::ImageRgba8(image);
        self.import_from_image(&hash, &decoded)
    }

    pub fn import_from_image(
        &mut self,
        hash: &str,
        decoded: &DynamicImage,
    ) -> Result<ImportedImage, ImageImportError> {
        let (original_width, original_height) = decoded.dimensions();
        let base_image = resize_to_limit(decoded, BASE_IMAGE_MAX_DIMENSION);
        let (base_width, base_height) = base_image.dimensions();

        let asset_path = format!("images/image_{hash}.webp");
        if !self.streaming_adapter.asset_exists(&asset_path) {
            println!("[image] encode new: {asset_path}");
            self.streaming_adapter
                .persist_webp(&asset_path, &base_image, BASE_WEBP_QUALITY)?;
        } else {
            println!("[image] hash hit: {asset_path}");
        }
        self.seed_ram(asset_path.clone(), decoded_from_image(&base_image));

        let hires_asset_path = if original_width.max(original_height) > BASE_IMAGE_MAX_DIMENSION {
            let hires_image = resize_to_limit(decoded, HIRES_IMAGE_MAX_DIMENSION);
            let hires_asset_path = format!("images/image_{hash}_hires.webp");
            if !self.streaming_adapter.asset_exists(&hires_asset_path) {
                println!("[image] encode new: {hires_asset_path}");
                self.streaming_adapter
                    .persist_webp(&hires_asset_path, &hires_image, HIRES_WEBP_QUALITY)?;
            } else {
                println!("[image] hash hit: {hires_asset_path}");
            }
            self.seed_ram(hires_asset_path.clone(), decoded_from_image(&hires_image));
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
        selected: bool,
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
                        [1.0, 1.0, 1.0, 1.0], selected,
                    ),
                };
            }

            return PreparedImageDraw {
                texture: self.thumb_placeholder_texture,
                instance: ImageInstanceData::new(
                    pos,
                    size,
                    [pos[0] + size[0] * 0.5, pos[1] + size[1] * 0.5],
                    rotation,
                    [0.0, 0.0],
                    [1.0, 1.0],
                    [1.0, 1.0, 1.0, 1.0], selected,
                ),
            };
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
                self.load_gpu_texture(ctx, path, true)
            })
            .or_else(|| self.load_gpu_texture(ctx, &image.asset_path, false))
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
                [1.0, 1.0, 1.0, 1.0], selected,
            ),
        }
    }

    pub fn preload_thumb(&mut self, ctx: &mut dyn RenderingBackend, relative_path: &str) {
        let _ = self.ensure_thumb_entry(ctx, relative_path);
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

    pub fn clear_ram_cache(&mut self) -> ImageRamClearStats {
        let stats = ImageRamClearStats {
            entries_cleared: self.ram_cache.len(),
            bytes_freed: self.ram_used_bytes,
        };
        self.ram_cache.clear();
        self.ram_lru.clear();
        self.ram_used_bytes = 0;
        stats
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
        clear_atlas_texture(ctx, self.atlas_pass);
    }

    fn load_gpu_texture(&mut self, ctx: &mut dyn RenderingBackend, relative_path: &str, is_hires: bool) -> Option<TextureId> {
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
                mipmap_filter: if is_hires {
                    MipmapFilterMode::None
                } else {
                    MipmapFilterMode::Linear
                },
                allocate_mipmaps: !is_hires,
                ..Default::default()
            },
        );
        if is_hires {
            ctx.texture_set_filter(texture, FilterMode::Linear, MipmapFilterMode::None);
        } else {
            ctx.texture_generate_mipmaps(texture);
            ctx.texture_set_filter(texture, FilterMode::Linear, MipmapFilterMode::Linear);
        }
        if is_hires {
            println!("[image] HIRES resident {} {}x{} no-mipmap", relative_path, decoded.width, decoded.height);
        }
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

        let decoded = self.streaming_adapter.load_decoded(relative_path)?;
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

        let source_texture = self.load_gpu_texture(ctx, relative_path, false)?;
        let source_size = self
            .ram_cache
            .get(relative_path)
            .map(|entry| (entry.image.width, entry.image.height))
            .or_else(|| {
                let (width, height) = ctx.texture_size(source_texture);
                (width > 0 && height > 0).then_some((width, height))
            })?;

        let slot = self.atlas_next_slot;
        self.atlas_next_slot = (self.atlas_next_slot + 1) % THUMB_SLOT_COUNT;
        if let Some(previous_owner) = self.atlas_slot_owner[slot].replace(relative_path.to_string()) {
            self.atlas_entries.remove(&previous_owner);
        }

        let col = (slot % THUMB_SLOTS_PER_ROW) as u32;
        let row = (slot / THUMB_SLOTS_PER_ROW) as u32;
        let x = col * THUMB_SIZE;
        let y = row * THUMB_SIZE;
        let (content_offset, content_size) = fit_thumbnail_rect(source_size.0, source_size.1)?;
        blit_texture_into_atlas(
            ctx,
            self.atlas_pass,
            &self.atlas_blit_pipeline,
            &mut self.atlas_blit_bindings,
            source_texture,
            [x, y],
            content_offset,
            content_size,
        );

        let atlas_size = THUMB_ATLAS_SIZE as f32;
        let uv_x = x + content_offset[0];
        let uv_y = y + content_offset[1];
        let entry = AtlasEntry {
            uv_min: [uv_x as f32 / atlas_size, uv_y as f32 / atlas_size],
            uv_max: [
                (uv_x + content_size[0]) as f32 / atlas_size,
                (uv_y + content_size[1]) as f32 / atlas_size,
            ],
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

pub(crate) fn decoded_from_image(image: &DynamicImage) -> DecodedImage {
    let rgba = image.to_rgba8();
    DecodedImage {
        width: image.width(),
        height: image.height(),
        rgba: rgba.into_raw(),
    }
}

fn fit_thumbnail_rect(width: u32, height: u32) -> Option<([u32; 2], [u32; 2])> {
    if width == 0 || height == 0 {
        return None;
    }

    let scale = (THUMB_SIZE as f32 / width as f32).min(THUMB_SIZE as f32 / height as f32);
    let draw_width = (width as f32 * scale).round().clamp(1.0, THUMB_SIZE as f32) as u32;
    let draw_height = (height as f32 * scale).round().clamp(1.0, THUMB_SIZE as f32) as u32;
    let offset_x = (THUMB_SIZE - draw_width) / 2;
    let offset_y = (THUMB_SIZE - draw_height) / 2;
    Some(([offset_x, offset_y], [draw_width, draw_height]))
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

fn fnv1a_hash(data: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)
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
            mipmap_filter: MipmapFilterMode::None,
            allocate_mipmaps: false,
            ..Default::default()
        },
    )
}

fn create_thumb_placeholder_texture(ctx: &mut dyn RenderingBackend) -> TextureId {
    let pixels: [u8; 16] = [92, 98, 108, 255, 92, 98, 108, 255, 92, 98, 108, 255, 92, 98, 108, 255];

    ctx.new_texture(
        TextureAccess::Static,
        TextureSource::Bytes(&pixels),
        TextureParams {
            width: 2,
            height: 2,
            format: TextureFormat::RGBA8,
            wrap: TextureWrap::Clamp,
            min_filter: FilterMode::Nearest,
            mag_filter: FilterMode::Nearest,
            mipmap_filter: MipmapFilterMode::None,
            allocate_mipmaps: false,
            ..Default::default()
        },
    )
}

fn create_thumb_atlas(ctx: &mut dyn RenderingBackend) -> TextureId {
    let texture = ctx.new_render_texture(TextureParams {
        width: THUMB_ATLAS_SIZE,
        height: THUMB_ATLAS_SIZE,
        format: TextureFormat::RGBA8,
        wrap: TextureWrap::Clamp,
        min_filter: FilterMode::Linear,
        mag_filter: FilterMode::Linear,
        mipmap_filter: MipmapFilterMode::None,
        allocate_mipmaps: false,
        ..Default::default()
    });
    ctx.texture_set_filter(texture, FilterMode::Linear, MipmapFilterMode::None);
    texture
}

fn clear_atlas_texture(ctx: &mut dyn RenderingBackend, atlas_pass: RenderPass) {
    ctx.begin_pass(Some(atlas_pass), PassAction::clear_color(0.0, 0.0, 0.0, 0.0));
    ctx.end_render_pass();
}

fn create_atlas_blit_resources(
    ctx: &mut dyn RenderingBackend,
    placeholder_texture: TextureId,
) -> (Pipeline, Bindings) {
    let vertices: [AtlasBlitVertex; 4] = [
        AtlasBlitVertex {
            pos: [-1.0, -1.0],
            uv: [0.0, 0.0],
        },
        AtlasBlitVertex {
            pos: [1.0, -1.0],
            uv: [1.0, 0.0],
        },
        AtlasBlitVertex {
            pos: [1.0, 1.0],
            uv: [1.0, 1.0],
        },
        AtlasBlitVertex {
            pos: [-1.0, 1.0],
            uv: [0.0, 1.0],
        },
    ];
    let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];

    let vertex_buffer = ctx.new_buffer(
        BufferType::VertexBuffer,
        BufferUsage::Immutable,
        BufferSource::slice(&vertices),
    );
    let index_buffer = ctx.new_buffer(
        BufferType::IndexBuffer,
        BufferUsage::Immutable,
        BufferSource::slice(&indices),
    );
    let shader = ctx
        .new_shader(
            ShaderSource::Glsl {
                vertex: ATLAS_BLIT_VERTEX_SRC,
                fragment: ATLAS_BLIT_FRAGMENT_SRC,
            },
            ShaderMeta {
                images: vec!["u_source".to_string()],
                uniforms: UniformBlockLayout { uniforms: vec![] },
            },
        )
        .expect("atlas blit shader compile failed");
    let pipeline = ctx.new_pipeline(
        &[BufferLayout::default()],
        &[
            VertexAttribute::new("a_pos", VertexFormat::Float2),
            VertexAttribute::new("a_uv", VertexFormat::Float2),
        ],
        shader,
        PipelineParams::default(),
    );
    let bindings = Bindings {
        vertex_buffers: vec![vertex_buffer],
        index_buffer,
        images: vec![placeholder_texture],
    };
    (pipeline, bindings)
}

fn blit_texture_into_atlas(
    ctx: &mut dyn RenderingBackend,
    atlas_pass: RenderPass,
    atlas_blit_pipeline: &Pipeline,
    atlas_blit_bindings: &mut Bindings,
    source_texture: TextureId,
    slot_origin: [u32; 2],
    content_offset: [u32; 2],
    content_size: [u32; 2],
) {
    atlas_blit_bindings.images[0] = source_texture;

    let slot_x = slot_origin[0] as i32;
    let slot_y = slot_origin[1] as i32;
    let content_x = (slot_origin[0] + content_offset[0]) as i32;
    let content_y = (slot_origin[1] + content_offset[1]) as i32;

    ctx.begin_pass(Some(atlas_pass), PassAction::Nothing);
    ctx.apply_scissor_rect(slot_x, slot_y, THUMB_SIZE as i32, THUMB_SIZE as i32);
    ctx.clear(Some((0.0, 0.0, 0.0, 0.0)), None, None);
    ctx.apply_pipeline(atlas_blit_pipeline);
    ctx.apply_bindings(atlas_blit_bindings);
    ctx.apply_viewport(content_x, content_y, content_size[0] as i32, content_size[1] as i32);
    ctx.apply_scissor_rect(content_x, content_y, content_size[0] as i32, content_size[1] as i32);
    ctx.draw(0, 6, 1);
    ctx.end_render_pass();
}

const ATLAS_BLIT_VERTEX_SRC: &str = r#"#version 100
attribute vec2 a_pos;
attribute vec2 a_uv;

varying vec2 v_uv;

void main() {
    gl_Position = vec4(a_pos, 0.0, 1.0);
    v_uv = a_uv;
}
"#;

const ATLAS_BLIT_FRAGMENT_SRC: &str = r#"#version 100
precision highp float;

varying vec2 v_uv;

uniform sampler2D u_source;

void main() {
    gl_FragColor = texture2D(u_source, v_uv);
}
"#;