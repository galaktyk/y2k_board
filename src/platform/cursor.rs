use miniquad::{window, CursorIcon};

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicUsize, Ordering};

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
static ACTIVE_CUSTOM_CURSOR: AtomicUsize = AtomicUsize::new(0);
#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
static HOOKED_HWND: AtomicIsize = AtomicIsize::new(0);
#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
static ORIGINAL_WNDPROC: AtomicIsize = AtomicIsize::new(0);
#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
static LAST_WHEEL_CTRL: AtomicBool = AtomicBool::new(false);

pub(crate) fn prime_window_hook() {
    #[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
    {
        let _ = ensure_windows_cursor_hook();
    }
}

pub(crate) fn consume_last_wheel_ctrl() -> bool {
    #[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
    {
        return LAST_WHEEL_CTRL.swap(false, Ordering::AcqRel);
    }

    #[cfg(any(target_arch = "wasm32", not(target_os = "windows")))]
    {
        false
    }
}

pub(crate) fn set_cursor(cursor: CursorIcon) -> bool {
    match try_set_custom_cursor(cursor) {
        CustomCursorResult::Applied => true,
        CustomCursorResult::Pending => false,
        CustomCursorResult::NotHandled => {
            window::set_mouse_cursor(cursor);
            true
        }
    }
}

#[cfg(target_arch = "wasm32")]
const DEFAULT_CURSOR_CSS: &str = "url('cursor/default_cursor.png') 0 0, default";
#[cfg(target_arch = "wasm32")]
const POINTER_CURSOR_CSS: &str = "url('cursor/pointer_cursor.png') 12 0, pointer";
#[cfg(target_arch = "wasm32")]
const STICKY_CURSOR_CSS: &str = "url('cursor/sticky_cursor.png') 0 0, help";

#[cfg(target_arch = "wasm32")]
fn try_set_custom_cursor(cursor: CursorIcon) -> CustomCursorResult {
    let css = match cursor {
        CursorIcon::Default => DEFAULT_CURSOR_CSS,
        CursorIcon::Pointer => POINTER_CURSOR_CSS,
        CursorIcon::Help => STICKY_CURSOR_CSS,
        _ => return CustomCursorResult::NotHandled,
    };

    crate::platform::browser_io::set_cursor_css(css);
    CustomCursorResult::Applied
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
fn try_set_custom_cursor(cursor: CursorIcon) -> CustomCursorResult {
    if !ensure_windows_cursor_hook() {
        return match cursor {
            CursorIcon::Default | CursorIcon::Pointer | CursorIcon::Help => {
                CustomCursorResult::Pending
            }
            _ => CustomCursorResult::NotHandled,
        };
    }

    let handle = match cursor {
        CursorIcon::Default => windows_cursor_handles().default as winapi::shared::windef::HCURSOR,
        CursorIcon::Pointer => windows_cursor_handles().pointer as winapi::shared::windef::HCURSOR,
        CursorIcon::Help => windows_cursor_handles().sticky as winapi::shared::windef::HCURSOR,
        _ => {
            set_active_custom_cursor(std::ptr::null_mut());
            return CustomCursorResult::NotHandled;
        }
    };

    if handle.is_null() {
        set_active_custom_cursor(std::ptr::null_mut());
        return CustomCursorResult::Pending;
    }

    set_active_custom_cursor(handle);
    unsafe {
        winapi::um::winuser::SetCursor(handle);
    }
    CustomCursorResult::Applied
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "windows")))]
fn try_set_custom_cursor(_cursor: CursorIcon) -> CustomCursorResult {
    CustomCursorResult::NotHandled
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
enum CustomCursorResult {
    Applied,
    Pending,
    NotHandled,
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
struct WindowsCursorHandles {
    default: usize,
    pointer: usize,
    sticky: usize,
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
fn windows_cursor_handles() -> &'static WindowsCursorHandles {
    use std::sync::OnceLock;

    static CURSORS: OnceLock<WindowsCursorHandles> = OnceLock::new();
    CURSORS.get_or_init(|| WindowsCursorHandles {
        default: load_png_cursor(
            include_bytes!("../../assets/cursor/default_cursor.png"),
            (0, 0),
        ) as usize,
        pointer: load_png_cursor(
            include_bytes!("../../assets/cursor/pointer_cursor.png"),
            (12, 0),
        ) as usize,
        sticky: load_png_cursor(
            include_bytes!("../../assets/cursor/sticky_cursor.png"),
            (0, 0),
        ) as usize,
    })
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
fn set_active_custom_cursor(cursor: winapi::shared::windef::HCURSOR) {
    ACTIVE_CUSTOM_CURSOR.store(cursor as usize, Ordering::Relaxed);
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
fn active_custom_cursor() -> winapi::shared::windef::HCURSOR {
    ACTIVE_CUSTOM_CURSOR.load(Ordering::Relaxed) as winapi::shared::windef::HCURSOR
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
fn ensure_windows_cursor_hook() -> bool {
    use winapi::um::winuser::{GetForegroundWindow, SetWindowLongPtrW, GWLP_WNDPROC};

    if HOOKED_HWND.load(Ordering::Acquire) != 0 {
        return true;
    }

    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_null() {
        return false;
    }

    if HOOKED_HWND
        .compare_exchange(0, hwnd as isize, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return true;
    }

    let original = unsafe {
        SetWindowLongPtrW(
            hwnd,
            GWLP_WNDPROC,
            windows_cursor_wndproc as *const () as isize,
        )
    };
    if original == 0 {
        HOOKED_HWND.store(0, Ordering::Release);
        return false;
    }

    ORIGINAL_WNDPROC.store(original, Ordering::Release);
    true
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
unsafe extern "system" fn windows_cursor_wndproc(
    hwnd: winapi::shared::windef::HWND,
    msg: winapi::shared::minwindef::UINT,
    wparam: winapi::shared::minwindef::WPARAM,
    lparam: winapi::shared::minwindef::LPARAM,
) -> winapi::shared::minwindef::LRESULT {
    use std::mem::transmute;
    use winapi::um::winuser::{
        CallWindowProcW, DefWindowProcW, SetCursor, MK_CONTROL, WM_MOUSEHWHEEL,
        WM_MOUSEWHEEL, WM_SETCURSOR, WNDPROC,
    };

    if msg == WM_MOUSEWHEEL || msg == WM_MOUSEHWHEEL {
        LAST_WHEEL_CTRL.store((wparam & MK_CONTROL as usize) != 0, Ordering::Release);
    }

    if msg == WM_SETCURSOR {
        let cursor = active_custom_cursor();
        if !cursor.is_null() {
            SetCursor(cursor);
            return 1;
        }
    }

    let original = ORIGINAL_WNDPROC.load(Ordering::Acquire);
    if original == 0 {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    let original_proc: WNDPROC = transmute(original);
    CallWindowProcW(original_proc, hwnd, msg, wparam, lparam)
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
fn load_png_cursor(bytes: &[u8], hotspot: (u32, u32)) -> winapi::shared::windef::HCURSOR {
    let Ok(image) = image::load_from_memory_with_format(bytes, image::ImageFormat::Png) else {
        return std::ptr::null_mut();
    };

    let rgba = image.to_rgba8();
    unsafe { create_rgba_cursor(&rgba, hotspot) }
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
unsafe fn create_rgba_cursor(
    rgba: &image::RgbaImage,
    hotspot: (u32, u32),
) -> winapi::shared::windef::HCURSOR {
    use std::mem::{size_of, zeroed};
    use std::ptr::{copy_nonoverlapping, null, null_mut};
    use winapi::ctypes::c_void;
    use winapi::shared::minwindef::FALSE;
    use winapi::shared::windef::HBITMAP;
    use winapi::um::wingdi::{
        CreateBitmap, CreateDIBSection, DeleteObject, BITMAPINFO, BITMAPV5HEADER, BI_BITFIELDS,
        DIB_RGB_COLORS,
    };
    use winapi::um::winuser::{CreateIconIndirect, GetDC, ReleaseDC, ICONINFO};

    let width = rgba.width() as i32;
    let height = rgba.height() as i32;
    if width <= 0 || height <= 0 {
        return null_mut();
    }

    let mut bgra = rgba.as_raw().clone();
    for pixel in bgra.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }

    let mut bitmap_header: BITMAPV5HEADER = zeroed();
    bitmap_header.bV5Size = size_of::<BITMAPV5HEADER>() as u32;
    bitmap_header.bV5Width = width;
    bitmap_header.bV5Height = -height;
    bitmap_header.bV5Planes = 1;
    bitmap_header.bV5BitCount = 32;
    bitmap_header.bV5Compression = BI_BITFIELDS;
    bitmap_header.bV5RedMask = 0x00FF_0000;
    bitmap_header.bV5GreenMask = 0x0000_FF00;
    bitmap_header.bV5BlueMask = 0x0000_00FF;
    bitmap_header.bV5AlphaMask = 0xFF00_0000;

    let screen_dc = GetDC(null_mut());
    if screen_dc.is_null() {
        return null_mut();
    }

    let mut bits: *mut c_void = null_mut();
    let color_bitmap = CreateDIBSection(
        screen_dc,
        &bitmap_header as *const _ as *const BITMAPINFO,
        DIB_RGB_COLORS,
        &mut bits,
        null_mut(),
        0,
    );
    ReleaseDC(null_mut(), screen_dc);

    if color_bitmap.is_null() || bits.is_null() {
        return null_mut();
    }

    copy_nonoverlapping(bgra.as_ptr(), bits as *mut u8, bgra.len());

    let mask_bitmap: HBITMAP = CreateBitmap(width, height, 1, 1, null());
    if mask_bitmap.is_null() {
        DeleteObject(color_bitmap as _);
        return null_mut();
    }

    let mut icon_info = ICONINFO {
        fIcon: FALSE,
        xHotspot: hotspot.0,
        yHotspot: hotspot.1,
        hbmMask: mask_bitmap,
        hbmColor: color_bitmap,
    };
    let cursor = CreateIconIndirect(&mut icon_info) as winapi::shared::windef::HCURSOR;

    DeleteObject(color_bitmap as _);
    DeleteObject(mask_bitmap as _);

    cursor
}
