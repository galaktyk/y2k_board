use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use image::DynamicImage;

#[cfg(not(target_arch = "wasm32"))]
use image::GenericImageView;

use crate::images::{decoded_from_image, DecodedImage, ImageImportError};

#[cfg(not(target_arch = "wasm32"))]
pub(crate) type PlatformImageStreamingAdapter = DiskImageStreamingAdapter;

#[cfg(target_arch = "wasm32")]
pub(crate) type PlatformImageStreamingAdapter = WebImageStreamingAdapter;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct DiskImageStreamingAdapter {
    asset_root: PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl DiskImageStreamingAdapter {
    pub(crate) fn new(asset_root: PathBuf) -> Self {
        Self { asset_root }
    }

    pub(crate) fn set_asset_root(&mut self, asset_root: PathBuf) {
        self.asset_root = asset_root;
    }

    pub(crate) fn persist_webp(
        &self,
        relative_path: &str,
        image: &DynamicImage,
        quality: f32,
    ) -> Result<(), ImageImportError> {
        let full_path = self.asset_root.join(relative_path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        write_webp(image, &full_path, quality)
    }

    pub(crate) fn load_decoded(&self, relative_path: &str) -> Option<DecodedImage> {
        let full_path = self.asset_root.join(relative_path);
        let decoded = image::open(&full_path).ok()?;
        Some(decoded_from_image(&decoded))
    }

    pub(crate) fn asset_exists(&self, relative_path: &str) -> bool {
        self.asset_root.join(relative_path).exists()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn write_webp(image: &DynamicImage, path: &Path, quality: f32) -> Result<(), ImageImportError> {
    let rgba = image.to_rgba8();
    let (width, height) = image.dimensions();
    let encoded = webp::Encoder::from_rgba(rgba.as_raw(), width, height).encode(quality);
    std::fs::write(path, encoded.as_ref())?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub(crate) struct WebImageStreamingAdapter;

#[cfg(target_arch = "wasm32")]
impl WebImageStreamingAdapter {
    pub(crate) fn new(_asset_root: PathBuf) -> Self {
        Self
    }

    pub(crate) fn set_asset_root(&mut self, _asset_root: PathBuf) {}

    pub(crate) fn persist_webp(
        &self,
        _relative_path: &str,
        _image: &DynamicImage,
        _quality: f32,
    ) -> Result<(), ImageImportError> {
        Err(ImageImportError::UnsupportedPlatform)
    }

    pub(crate) fn load_decoded(&self, _relative_path: &str) -> Option<DecodedImage> {
        None
    }

    pub(crate) fn asset_exists(&self, _relative_path: &str) -> bool {
        false
    }
}
