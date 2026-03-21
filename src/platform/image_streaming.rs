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
unsafe extern "C" {
    fn mg_store_webp_asset(
        relative_path_ptr: *const u8,
        relative_path_len: usize,
        rgba_ptr: *const u8,
        rgba_len: usize,
        width: u32,
        height: u32,
        quality: f32,
    );
}

#[cfg(target_arch = "wasm32")]
fn web_asset_store() -> &'static Mutex<HashMap<String, Vec<u8>>> {
    static WEB_ASSET_STORE: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();
    WEB_ASSET_STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(target_arch = "wasm32")]
fn encode_browser_fallback_asset(image: &DynamicImage) -> Result<Vec<u8>, ImageImportError> {
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .map_err(ImageImportError::from)?;
    Ok(bytes)
}

#[cfg(target_arch = "wasm32")]
fn store_web_asset(relative_path: String, bytes: Vec<u8>) {
    let mut store = web_asset_store()
        .lock()
        .expect("web asset store mutex should not be poisoned");
    store.insert(relative_path, bytes);
}

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn mg_embedded_asset_loaded(
    relative_path_ptr: *mut u8,
    relative_path_len: usize,
    data_ptr: *mut u8,
    data_len: usize,
) {
    let relative_path = if relative_path_len == 0 {
        String::new()
    } else {
        let bytes = unsafe {
            Vec::from_raw_parts(relative_path_ptr, relative_path_len, relative_path_len)
        };
        String::from_utf8(bytes)
            .unwrap_or_else(|err| String::from_utf8_lossy(&err.into_bytes()).into_owned())
    };

    let bytes = if data_len == 0 {
        Vec::new()
    } else {
        unsafe { Vec::from_raw_parts(data_ptr, data_len, data_len) }
    };

    store_web_asset(relative_path, bytes);
    miniquad::window::schedule_update();
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
        quality: f32,
    ) -> Result<(), ImageImportError> {
        let encoded = encode_browser_fallback_asset(image)?;
        store_web_asset(relative_path.to_string(), encoded);

        let rgba = image.to_rgba8();
        let rgba_bytes = rgba.as_raw();
        unsafe {
            mg_store_webp_asset(
                relative_path.as_ptr(),
                relative_path.len(),
                rgba_bytes.as_ptr(),
                rgba_bytes.len(),
                image.width(),
                image.height(),
                quality,
            );
        }
        Ok(())
    }

    pub(crate) fn load_decoded(&self, relative_path: &str) -> Option<DecodedImage> {
        let store = web_asset_store()
            .lock()
            .expect("web asset store mutex should not be poisoned");
        let bytes = store.get(relative_path)?.clone();
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
