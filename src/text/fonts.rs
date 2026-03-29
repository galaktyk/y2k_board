use std::io::Cursor as IoCursor;
use std::sync::Arc;
#[cfg(target_arch = "wasm32")]
use cosmic_text::Fallback;
use cosmic_text::{fontdb, FontSystem};
#[cfg(target_arch = "wasm32")]
use unicode_script::Script;
use woff2_patched::convert_woff2_to_ttf;

pub const PRIMARY_UI_FONT: BundledFontAsset = BundledFontAsset {
    bytes: include_bytes!("../../fonts/W95FA.otf"),
    family_hint: "W95FA",
};

#[derive(Clone, Copy)]
pub struct BundledFontAsset {
    pub bytes: &'static [u8],
    pub family_hint: &'static str,
}

pub fn configure_bundled_font_defaults(font_system: &mut FontSystem) {
    let bundled_family = {
        let db = font_system.db_mut();
        let family = load_bundled_font(db, PRIMARY_UI_FONT)
            .unwrap_or_else(|| PRIMARY_UI_FONT.family_hint.to_string());

        db.set_sans_serif_family(family.clone());
        family
    };

    debug_assert!(!bundled_family.is_empty());
}

pub fn load_bundled_font(db: &mut fontdb::Database, asset: BundledFontAsset) -> Option<String> {
    let ids = db.load_font_source(fontdb::Source::Binary(Arc::new(asset.bytes.to_vec())));
    ids.first()
        .and_then(|id| db.face(*id))
        .and_then(|face| face.families.first())
        .map(|(name, _)| name.clone())
        .or_else(|| Some(asset.family_hint.to_string()))
}

pub fn decode_browser_font_bytes(bytes: Vec<u8>) -> Option<Vec<u8>> {
    if bytes.starts_with(b"wOF2") {
        let mut cursor = IoCursor::new(bytes);
        match convert_woff2_to_ttf(&mut cursor) {
            Ok(decoded) => Some(decoded),
            Err(err) => {
                eprintln!("[font] failed to decode woff2 payload: {err}");
                None
            }
        }
    } else {
        Some(bytes)
    }
}

pub fn new_font_system() -> FontSystem {
    #[cfg(target_arch = "wasm32")]
    {
        let mut db = fontdb::Database::new();
        db.set_monospace_family("W95FA");
        db.set_sans_serif_family("W95FA");
        db.set_serif_family("W95FA");
        return FontSystem::new_with_locale_and_db_and_fallback(
            "en-US".to_string(),
            db,
            WasmFontFallback,
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        FontSystem::new()
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Default, Clone, Copy)]
struct WasmFontFallback;

#[cfg(target_arch = "wasm32")]
impl Fallback for WasmFontFallback {
    fn common_fallback(&self) -> &[&'static str] {
        &[]
    }

    fn forbidden_fallback(&self) -> &[&'static str] {
        &[]
    }

    fn script_fallback(&self, script: Script, _locale: &str) -> &[&'static str] {
        &[]
    }
}
