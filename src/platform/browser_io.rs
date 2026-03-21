#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

#[cfg(target_arch = "wasm32")]
use std::mem;
#[cfg(target_arch = "wasm32")]
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BrowserFileKind {
    Snapshot,
    Image,
}

#[derive(Debug)]
pub(crate) struct BrowserPickedFile {
    pub kind: BrowserFileKind,
    pub name: String,
    pub bytes: Vec<u8>,
}

#[cfg(target_arch = "wasm32")]
fn browser_file_queue() -> &'static Mutex<Vec<BrowserPickedFile>> {
    static FILE_QUEUE: OnceLock<Mutex<Vec<BrowserPickedFile>>> = OnceLock::new();
    FILE_QUEUE.get_or_init(|| Mutex::new(Vec::new()))
}

#[cfg(target_arch = "wasm32")]
fn browser_font_queue() -> &'static Mutex<Vec<Vec<u8>>> {
    static FONT_QUEUE: OnceLock<Mutex<Vec<Vec<u8>>>> = OnceLock::new();
    FONT_QUEUE.get_or_init(|| Mutex::new(Vec::new()))
}

#[cfg(target_arch = "wasm32")]
fn browser_file_kind_from_raw(value: u32) -> Option<BrowserFileKind> {
    match value {
        1 => Some(BrowserFileKind::Snapshot),
        2 => Some(BrowserFileKind::Image),
        _ => None,
    }
}

#[cfg(target_arch = "wasm32")]
unsafe extern "C" {
    fn mg_request_snapshot_load();
    fn mg_request_image_upload();
    fn mg_load_fonts_for_text(text_ptr: *const u8, text_len: usize);
    fn mg_download_bytes(
        name_ptr: *const u8,
        name_len: usize,
        mime_ptr: *const u8,
        mime_len: usize,
        data_ptr: *const u8,
        data_len: usize,
    );
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn request_snapshot_load() {
    unsafe {
        mg_request_snapshot_load();
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn request_snapshot_load() {}

#[cfg(target_arch = "wasm32")]
pub(crate) fn request_image_upload() {
    unsafe {
        mg_request_image_upload();
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn request_image_upload() {}

#[cfg(target_arch = "wasm32")]
fn browser_font_request_queue() -> &'static Mutex<String> {
    static REQUEST_QUEUE: OnceLock<Mutex<String>> = OnceLock::new();
    REQUEST_QUEUE.get_or_init(|| Mutex::new(String::new()))
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn request_fonts_for_text(text: &str) {
    if text.is_empty() {
        return;
    }

    let mut queue = browser_font_request_queue()
        .lock()
        .expect("font request queue mutex should not be poisoned");
    queue.push_str(text);
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn request_fonts_for_text(_text: &str) {}

#[cfg(target_arch = "wasm32")]
pub(crate) fn flush_font_requests() {
    let mut queue = browser_font_request_queue()
        .lock()
        .expect("font request queue mutex should not be poisoned");
    if queue.is_empty() {
        return;
    }

    let text = std::mem::take(&mut *queue);
    unsafe {
        mg_load_fonts_for_text(text.as_ptr(), text.len());
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn flush_font_requests() {}

#[cfg(target_arch = "wasm32")]
pub(crate) fn download_bytes(name: &str, mime: &str, data: &[u8]) {
    unsafe {
        mg_download_bytes(
            name.as_ptr(),
            name.len(),
            mime.as_ptr(),
            mime.len(),
            data.as_ptr(),
            data.len(),
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn download_bytes(_name: &str, _mime: &str, _data: &[u8]) {}

#[cfg(target_arch = "wasm32")]
pub(crate) fn take_picked_files() -> Vec<BrowserPickedFile> {
    let mut queue = browser_file_queue()
        .lock()
        .expect("browser file queue mutex should not be poisoned");
    mem::take(&mut *queue)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn take_picked_files() -> Vec<BrowserPickedFile> {
    Vec::new()
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn take_loaded_fonts() -> Vec<Vec<u8>> {
    let mut queue = browser_font_queue()
        .lock()
        .expect("browser font queue mutex should not be poisoned");
    mem::take(&mut *queue)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn take_loaded_fonts() -> Vec<Vec<u8>> {
    Vec::new()
}

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn mg_browser_file_selected(
    kind: u32,
    name_ptr: *mut u8,
    name_len: usize,
    data_ptr: *mut u8,
    data_len: usize,
) {
    let Some(kind) = browser_file_kind_from_raw(kind) else {
        if data_len > 0 {
            unsafe {
                drop(Vec::from_raw_parts(data_ptr, data_len, data_len));
            }
        }
        if name_len > 0 {
            unsafe {
                drop(Vec::from_raw_parts(name_ptr, name_len, name_len));
            }
        }
        return;
    };

    let name = if name_len == 0 {
        String::new()
    } else {
        let name_bytes = unsafe { Vec::from_raw_parts(name_ptr, name_len, name_len) };
        String::from_utf8(name_bytes).unwrap_or_else(|err| String::from_utf8_lossy(&err.into_bytes()).into_owned())
    };
    let bytes = if data_len == 0 {
        Vec::new()
    } else {
        unsafe { Vec::from_raw_parts(data_ptr, data_len, data_len) }
    };

    let mut queue = browser_file_queue()
        .lock()
        .expect("browser file queue mutex should not be poisoned");
    queue.push(BrowserPickedFile { kind, name, bytes });
    drop(queue);
    miniquad::window::schedule_update();
}

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn mg_browser_font_loaded(data_ptr: *mut u8, data_len: usize) {
    let bytes = if data_len == 0 {
        Vec::new()
    } else {
        unsafe { Vec::from_raw_parts(data_ptr, data_len, data_len) }
    };

    if bytes.is_empty() {
        println!("[font] browser delivered empty font payload");
        return;
    }

    println!("[font] browser delivered font payload bytes={}", bytes.len());

    let mut queue = browser_font_queue()
        .lock()
        .expect("browser font queue mutex should not be poisoned");
    queue.push(bytes);
    drop(queue);
    miniquad::window::schedule_update();
}