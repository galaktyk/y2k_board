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