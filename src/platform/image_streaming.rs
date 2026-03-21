use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

#[cfg(target_arch = "wasm32")]
use std::collections::HashMap;
#[cfg(target_arch = "wasm32")]
use std::io::Cursor;
#[cfg(target_arch = "wasm32")]
use std::sync::{Mutex, OnceLock};

use image::DynamicImage;

#[cfg(not(target_arch = "wasm32"))]
use image::GenericImageView;

use crate::images::{decoded_from_image, DecodedImage, ImageImportError};

#[cfg(target_arch = "wasm32")]
fn web_asset_store() -> &'static Mutex<HashMap<String, Vec<u8>>> {
    static WEB_ASSET_STORE: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();
    WEB_ASSET_STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(target_arch = "wasm32")]
fn encode_browser_asset(image: &DynamicImage) -> Result<Vec<u8>, ImageImportError> {
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .map_err(ImageImportError::from)?;
    Ok(bytes)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn collect_embedded_assets<I>(relative_paths: I) -> Vec<(String, Vec<u8>)>
where
    I: IntoIterator<Item = String>,
{
    let store = web_asset_store()
        .lock()
        .expect("web asset store mutex should not be poisoned");

    relative_paths
        .into_iter()
        .filter_map(|relative_path| {
            store
                .get(&relative_path)
                .cloned()
                .map(|bytes| (relative_path, bytes))
        })
        .collect()
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn replace_embedded_assets(assets: Vec<(String, Vec<u8>)>) {
    let mut store = web_asset_store()
        .lock()
        .expect("web asset store mutex should not be poisoned");
    store.clear();
    store.extend(assets);
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn clear_embedded_assets() {
    let mut store = web_asset_store()
        .lock()
        .expect("web asset store mutex should not be poisoned");
    store.clear();
}

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
        relative_path: &str,
        image: &DynamicImage,
        _quality: f32,
    ) -> Result<(), ImageImportError> {
        let encoded = encode_browser_asset(image)?;
        let mut store = web_asset_store()
            .lock()
            .expect("web asset store mutex should not be poisoned");
        store.insert(relative_path.to_string(), encoded);
        Ok(())
    }

    pub(crate) fn load_decoded(&self, _relative_path: &str) -> Option<DecodedImage> {
        let store = web_asset_store()
            .lock()
            .expect("web asset store mutex should not be poisoned");
        let bytes = store.get(_relative_path)?.clone();
        let decoded = image::load_from_memory(&bytes).ok()?;
        Some(decoded_from_image(&decoded))
    }

    pub(crate) fn asset_exists(&self, relative_path: &str) -> bool {
        let store = web_asset_store()
            .lock()
            .expect("web asset store mutex should not be poisoned");
        store.contains_key(relative_path)
    }
}
