pub fn set_ime_candidate_pos(x: i32, y: i32) {
    #[cfg(target_os = "windows")]
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

    #[cfg(not(target_os = "windows"))]
    let _ = (x, y);
}