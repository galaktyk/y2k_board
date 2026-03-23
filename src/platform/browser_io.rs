#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

#[cfg(target_arch = "wasm32")]
use std::mem;
#[cfg(target_arch = "wasm32")]
use std::sync::atomic::{AtomicBool, Ordering};
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
fn browser_clipboard_queue() -> &'static Mutex<Vec<String>> {
    static CLIPBOARD_QUEUE: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    CLIPBOARD_QUEUE.get_or_init(|| Mutex::new(Vec::new()))
}

#[cfg(target_arch = "wasm32")]
fn browser_app_ready() -> &'static AtomicBool {
    static APP_READY: AtomicBool = AtomicBool::new(false);
    &APP_READY
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
pub(crate) fn take_clipboard_pastes() -> Vec<String> {
    let mut queue = browser_clipboard_queue()
        .lock()
        .expect("browser clipboard queue mutex should not be poisoned");
    mem::take(&mut *queue)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn take_clipboard_pastes() -> Vec<String> {
    Vec::new()
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn mark_app_ready() {
    browser_app_ready().store(true, Ordering::Release);
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn mark_app_ready() {}

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

    if browser_app_ready().load(Ordering::Acquire) {
        miniquad::window::schedule_update();
    } else {
        println!("[font] app not ready yet; deferred redraw request for queued browser font");
    }
}

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn mg_browser_clipboard_paste(data_ptr: *mut u8, data_len: usize) {
    let text = if data_len == 0 {
        String::new()
    } else {
        let bytes = unsafe { Vec::from_raw_parts(data_ptr, data_len, data_len) };
        String::from_utf8(bytes)
            .unwrap_or_else(|err| String::from_utf8_lossy(&err.into_bytes()).into_owned())
    };

    if text.is_empty() {
        return;
    }

    let mut queue = browser_clipboard_queue()
        .lock()
        .expect("browser clipboard queue mutex should not be poisoned");
    queue.push(text);
    drop(queue);

    if browser_app_ready().load(Ordering::Acquire) {
        miniquad::window::schedule_update();
    }
}