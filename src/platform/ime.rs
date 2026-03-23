#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

#[cfg(target_arch = "wasm32")]
use std::mem;
#[cfg(target_arch = "wasm32")]
use std::sync::{Mutex, OnceLock};

#[cfg(target_arch = "wasm32")]
#[derive(Debug)]
pub(crate) enum BrowserTextInputEvent {
    Insert(String),
    DeleteBackward,
    DeleteForward,
}

#[cfg(target_arch = "wasm32")]
fn browser_text_input_queue() -> &'static Mutex<Vec<BrowserTextInputEvent>> {
    static TEXT_INPUT_QUEUE: OnceLock<Mutex<Vec<BrowserTextInputEvent>>> = OnceLock::new();
    TEXT_INPUT_QUEUE.get_or_init(|| Mutex::new(Vec::new()))
}

#[cfg(target_arch = "wasm32")]
unsafe extern "C" {
    fn mg_set_text_input_active(active: u32);
    fn mg_set_ime_candidate_pos(x: i32, y: i32);
}

pub fn set_text_input_active(active: bool) {
    #[cfg(target_arch = "wasm32")]
    unsafe {
        mg_set_text_input_active(u32::from(active));
    }

    #[cfg(not(target_arch = "wasm32"))]
    let _ = active;
}

pub fn set_ime_candidate_pos(x: i32, y: i32) {
    #[cfg(target_arch = "wasm32")]
    unsafe {
        mg_set_ime_candidate_pos(x, y);
    }

    #[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
    unsafe {
        use winapi::shared::windef::POINT;
        use winapi::um::imm::{
            CFS_POINT, COMPOSITIONFORM, ImmGetContext, ImmReleaseContext,
            ImmSetCompositionWindow,
        };
        use winapi::um::winuser::GetForegroundWindow;

        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return;
        }
        let himc = ImmGetContext(hwnd);
        if himc.is_null() {
            return;
        }
        let mut form = COMPOSITIONFORM {
            dwStyle: CFS_POINT,
            ptCurrentPos: POINT { x, y },
            rcArea: std::mem::zeroed(),
        };
        ImmSetCompositionWindow(himc, &mut form);
        ImmReleaseContext(hwnd, himc);
    }

    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "windows")))]
    let _ = (x, y);
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn take_text_input_events() -> Vec<BrowserTextInputEvent> {
    let mut queue = browser_text_input_queue()
        .lock()
        .expect("browser text input queue mutex should not be poisoned");
    mem::take(&mut *queue)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn take_text_input_events() -> Vec<()> {
    Vec::new()
}

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn mg_browser_text_input_insert(data_ptr: *mut u8, data_len: usize) {
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

    let mut queue = browser_text_input_queue()
        .lock()
        .expect("browser text input queue mutex should not be poisoned");
    queue.push(BrowserTextInputEvent::Insert(text));
    drop(queue);

    miniquad::window::schedule_update();
}

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn mg_browser_text_input_delete_backward() {
    let mut queue = browser_text_input_queue()
        .lock()
        .expect("browser text input queue mutex should not be poisoned");
    queue.push(BrowserTextInputEvent::DeleteBackward);
    drop(queue);

    miniquad::window::schedule_update();
}

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn mg_browser_text_input_delete_forward() {
    let mut queue = browser_text_input_queue()
        .lock()
        .expect("browser text input queue mutex should not be poisoned");
    queue.push(BrowserTextInputEvent::DeleteForward);
    drop(queue);

    miniquad::window::schedule_update();
}