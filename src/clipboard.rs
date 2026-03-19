use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::board::{Element, LineEndpoints};

// ── System clipboard helpers ─────────────────────────────────────────────────

pub struct ClipboardImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub enum ClipboardPaste {
    Image(ClipboardImage),
    Text(String),
}

#[cfg(all(target_os = "windows", not(target_arch = "wasm32")))]
pub fn preferred_paste_contents() -> Result<Option<ClipboardPaste>, String> {
    use std::convert::TryFrom;

    let mut clipboard = arboard::Clipboard::new().map_err(|err| err.to_string())?;

    match clipboard.get_image() {
        Ok(image) => {
            let width = u32::try_from(image.width)
                .map_err(|_| "clipboard image width was too large".to_string())?;
            let height = u32::try_from(image.height)
                .map_err(|_| "clipboard image height was too large".to_string())?;
            let rgba = image.bytes.into_owned();
            return Ok(Some(ClipboardPaste::Image(ClipboardImage {
                width,
                height,
                rgba,
            })));
        }
        Err(err) if matches!(err, arboard::Error::ContentNotAvailable) => {}
        Err(err) => return Err(err.to_string()),
    }

    match clipboard.get_text() {
        Ok(text) if !text.is_empty() => Ok(Some(ClipboardPaste::Text(text))),
        Ok(_) => Ok(None),
        Err(err) if matches!(err, arboard::Error::ContentNotAvailable) => Ok(None),
        Err(err) => Err(err.to_string()),
    }
}

#[cfg(not(all(target_os = "windows", not(target_arch = "wasm32"))))]
pub fn preferred_paste_contents() -> Result<Option<ClipboardPaste>, String> {
    Ok(None)
}

// ── Board object clipboard ────────────────────────────────────────────────────

const BOARD_CLIP_TYPE: &str = "miniGalaktyk/clipboard/v1";

/// JSON payload written to the system clipboard when copying board elements.
///
/// Line connection keys are string-encoded u64 IDs (JSON only allows string
/// map keys). Image bytes are base64-encoded WebP.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoardClipboardData {
    #[serde(rename = "type")]
    pub clip_type: String,
    /// Bounding-box centroid of all copied elements in world space.
    pub centroid: [f32; 2],
    /// Copied elements with `selected = false` and original IDs.
    pub elements: Vec<Element>,
    /// Keyed by string-encoded line element ID. Only contains anchors whose
    /// `target_id` is also in `elements` (external anchors are dropped).
    pub line_connections: HashMap<String, LineEndpoints>,
    /// Keyed by `asset_path` / `hires_asset_path`. Values are standard
    /// base64-encoded WebP bytes (uses the STANDARD alphabet, no line breaks).
    pub images: HashMap<String, String>,
}

impl BoardClipboardData {
    pub fn new(
        centroid: [f32; 2],
        elements: Vec<Element>,
        line_connections: HashMap<String, LineEndpoints>,
        images: HashMap<String, String>,
    ) -> Self {
        Self {
            clip_type: BOARD_CLIP_TYPE.to_owned(),
            centroid,
            elements,
            line_connections,
            images,
        }
    }
}

/// Try to parse a board clipboard payload from a plain-text clipboard string.
/// Returns `None` if the text isn't a recognised miniGalaktyk board clipboard.
pub fn detect_board_clipboard(text: &str) -> Option<BoardClipboardData> {
    if !text.contains(BOARD_CLIP_TYPE) {
        return None;
    }
    match serde_json::from_str::<BoardClipboardData>(text) {
        Ok(data) if data.clip_type == BOARD_CLIP_TYPE => Some(data),
        _ => None,
    }
}

/// Write a `BoardClipboardData` to the system clipboard as JSON text.
/// No-ops silently on platforms where clipboard write isn't available.
pub fn set_board_clipboard(data: &BoardClipboardData) -> Result<(), String> {
    let json = serde_json::to_string(data).map_err(|e| e.to_string())?;

    #[cfg(all(target_os = "windows", not(target_arch = "wasm32")))]
    {
        let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
        clipboard.set_text(json).map_err(|e| e.to_string())?;
    }

    #[cfg(not(all(target_os = "windows", not(target_arch = "wasm32"))))]
    {
        miniquad::window::clipboard_set(&json);
    }

    Ok(())
}

/// Read raw clipboard text; returns `None` when unavailable.
pub fn get_clipboard_text() -> Option<String> {
    #[cfg(all(target_os = "windows", not(target_arch = "wasm32")))]
    {
        let mut clipboard = arboard::Clipboard::new().ok()?;
        clipboard.get_text().ok().filter(|t| !t.is_empty())
    }

    #[cfg(not(all(target_os = "windows", not(target_arch = "wasm32"))))]
    {
        miniquad::window::clipboard_get()
    }
}