use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView};
use miniquad::{FilterMode, RenderingBackend, TextureAccess, TextureFormat, TextureId, TextureParams, TextureSource, TextureWrap};

use crate::board::ImageData;
use crate::renderer::{ImageInstanceData, PreparedImageDraw};

pub const BASE_IMAGE_MAX_DIMENSION: u32 = 512;
pub const HIRES_IMAGE_MAX_DIMENSION: u32 = 2048;
pub const HIRES_SCREEN_FRACTION: f32 = 0.8;

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

#[derive(Clone, Copy)]
struct LoadedTexture {
    texture: TextureId,
}

pub struct ImageManager {
    asset_root: PathBuf,
    textures: HashMap<String, LoadedTexture>,
    missing_texture: TextureId,
}

impl ImageManager {
    pub fn new(ctx: &mut dyn RenderingBackend, asset_root: PathBuf) -> Self {
        Self {
            asset_root,
            textures: HashMap::new(),
            missing_texture: create_missing_texture(ctx),
        }
    }

    pub fn import_from_source(&self, element_id: u64, source_path: &Path) -> Result<ImportedImage, ImageImportError> {
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

            let hires_asset_path = if original_width.max(original_height) > BASE_IMAGE_MAX_DIMENSION {
                let hires_image = resize_to_limit(&decoded, HIRES_IMAGE_MAX_DIMENSION);
                let hires_asset_path = format!("images/image_{element_id}_hires.webp");
                write_webp(
                    &hires_image,
                    &self.asset_root.join(&hires_asset_path),
                    HIRES_WEBP_QUALITY,
                )?;
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
        screen_extent: [f32; 2],
        viewport_size: [f32; 2],
    ) -> PreparedImageDraw {
        let prefer_hires = image
            .hires_asset_path
            .as_ref()
            .filter(|_| screen_extent[0] > viewport_size[0] * HIRES_SCREEN_FRACTION || screen_extent[1] > viewport_size[1] * HIRES_SCREEN_FRACTION)
            .map(String::as_str);

        let texture = prefer_hires
            .and_then(|path| self.load_texture(ctx, path))
            .or_else(|| self.load_texture(ctx, &image.asset_path))
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

    fn load_texture(&mut self, ctx: &mut dyn RenderingBackend, relative_path: &str) -> Option<TextureId> {
        if let Some(texture) = self.textures.get(relative_path) {
            return Some(texture.texture);
        }

        let full_path = self.asset_root.join(relative_path);
        let decoded = image::open(&full_path).ok()?;
        let rgba = decoded.to_rgba8();
        let (width, height) = decoded.dimensions();
        let texture = ctx.new_texture(
            TextureAccess::Static,
            TextureSource::Bytes(rgba.as_raw()),
            TextureParams {
                width,
                height,
                format: TextureFormat::RGBA8,
                wrap: TextureWrap::Clamp,
                min_filter: FilterMode::Linear,
                mag_filter: FilterMode::Linear,
                ..Default::default()
            },
        );
        self.textures
            .insert(relative_path.to_string(), LoadedTexture { texture });
        Some(texture)
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