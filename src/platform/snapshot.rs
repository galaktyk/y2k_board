use std::path::{Path, PathBuf};

use crate::snapshot::{LoadedSnapshot, SnapshotData, SnapshotError};

#[cfg(target_arch = "wasm32")]
use serde::{Deserialize, Serialize};

pub(crate) trait SnapshotDialogAdapter {
    fn pick_save_path(&self, snapshot_path: &Path) -> Option<PathBuf>;
    fn pick_load_path(&self, snapshot_path: &Path) -> Option<PathBuf>;
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
pub(crate) type PlatformSnapshotDialogAdapter = WindowsSnapshotDialogAdapter;

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "windows")))]
pub(crate) type PlatformSnapshotDialogAdapter = DesktopSnapshotDialogAdapter;

#[cfg(target_arch = "wasm32")]
pub(crate) type PlatformSnapshotDialogAdapter = WebSnapshotAdapter;

pub(crate) trait SnapshotPersistenceAdapter {
    fn save_to_path(&self, snapshot: &SnapshotData, path: &Path) -> Result<PathBuf, SnapshotError>;
    fn load_from_path(&self, path: &Path) -> Result<LoadedSnapshot, SnapshotError>;

    #[cfg(target_arch = "wasm32")]
    fn save_to_bytes(&self, snapshot: &SnapshotData) -> Result<Vec<u8>, SnapshotError>;

    #[cfg(target_arch = "wasm32")]
    fn load_from_bytes(&self, bytes: &[u8], path: &Path) -> Result<LoadedSnapshot, SnapshotError>;
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct LegacyEmbeddedSnapshotAsset {
    relative_path: String,
    bytes: Vec<u8>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct LegacySnapshotBundle {
    format_version: u32,
    data: SnapshotData,
    assets: Vec<LegacyEmbeddedSnapshotAsset>,
}

#[cfg(target_arch = "wasm32")]
const LEGACY_SNAPSHOT_BUNDLE_VERSION: u32 = 1;

#[cfg(not(target_arch = "wasm32"))]
fn default_snapshot_name(snapshot_path: &Path) -> &str {
    snapshot_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("snapshot.bin")
}

#[cfg(not(target_arch = "wasm32"))]
fn pick_save_path_with_rfd(snapshot_path: &Path) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("miniGalaktyk Snapshots", &["bin"])
        .set_directory(crate::snapshot::snapshot_root(snapshot_path))
        .set_file_name(default_snapshot_name(snapshot_path))
        .save_file()
}

#[cfg(not(target_arch = "wasm32"))]
fn pick_load_path_with_rfd(snapshot_path: &Path) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("miniGalaktyk Snapshots", &["bin"])
        .set_directory(crate::snapshot::snapshot_root(snapshot_path))
        .pick_file()
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
pub(crate) struct WindowsSnapshotDialogAdapter;

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
impl WindowsSnapshotDialogAdapter {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
impl SnapshotDialogAdapter for WindowsSnapshotDialogAdapter {
    fn pick_save_path(&self, snapshot_path: &Path) -> Option<PathBuf> {
        pick_save_path_with_rfd(snapshot_path)
    }

    fn pick_load_path(&self, snapshot_path: &Path) -> Option<PathBuf> {
        pick_load_path_with_rfd(snapshot_path)
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "windows")))]
pub(crate) struct DesktopSnapshotDialogAdapter;

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "windows")))]
impl DesktopSnapshotDialogAdapter {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "windows")))]
impl SnapshotDialogAdapter for DesktopSnapshotDialogAdapter {
    fn pick_save_path(&self, snapshot_path: &Path) -> Option<PathBuf> {
        pick_save_path_with_rfd(snapshot_path)
    }

    fn pick_load_path(&self, snapshot_path: &Path) -> Option<PathBuf> {
        pick_load_path_with_rfd(snapshot_path)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) type PlatformSnapshotAdapter = DiskSnapshotAdapter;

#[cfg(target_arch = "wasm32")]
pub(crate) type PlatformSnapshotAdapter = WebSnapshotAdapter;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct DiskSnapshotAdapter;

#[cfg(not(target_arch = "wasm32"))]
impl DiskSnapshotAdapter {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl SnapshotPersistenceAdapter for DiskSnapshotAdapter {
    fn save_to_path(&self, snapshot: &SnapshotData, path: &Path) -> Result<PathBuf, SnapshotError> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let bytes = bincode::serialize(snapshot)?;
        std::fs::write(path, bytes)?;
        Ok(path.to_path_buf())
    }

    fn load_from_path(&self, path: &Path) -> Result<LoadedSnapshot, SnapshotError> {
        let bytes = std::fs::read(path)?;
        Ok(LoadedSnapshot {
            data: bincode::deserialize(&bytes)?,
            path: path.to_path_buf(),
        })
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) struct WebSnapshotAdapter;

#[cfg(target_arch = "wasm32")]
impl WebSnapshotAdapter {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[cfg(target_arch = "wasm32")]
impl SnapshotPersistenceAdapter for WebSnapshotAdapter {
    fn save_to_path(&self, _snapshot: &SnapshotData, _path: &Path) -> Result<PathBuf, SnapshotError> {
        Err(SnapshotError::UnsupportedPlatform)
    }

    fn load_from_path(&self, _path: &Path) -> Result<LoadedSnapshot, SnapshotError> {
        Err(SnapshotError::UnsupportedPlatform)
    }

    fn save_to_bytes(&self, snapshot: &SnapshotData) -> Result<Vec<u8>, SnapshotError> {
        Ok(bincode::serialize(snapshot)?)
    }

    fn load_from_bytes(&self, bytes: &[u8], path: &Path) -> Result<LoadedSnapshot, SnapshotError> {
        if let Ok(bundle) = bincode::deserialize::<LegacySnapshotBundle>(bytes) {
            if bundle.format_version == LEGACY_SNAPSHOT_BUNDLE_VERSION {
                return Ok(LoadedSnapshot {
                    data: bundle.data,
                    path: path.to_path_buf(),
                });
            }
        }

        Ok(LoadedSnapshot {
            data: bincode::deserialize(bytes)?,
            path: path.to_path_buf(),
        })
    }
}

#[cfg(target_arch = "wasm32")]
impl SnapshotDialogAdapter for WebSnapshotAdapter {
    fn pick_save_path(&self, _snapshot_path: &Path) -> Option<PathBuf> {
        None
    }

    fn pick_load_path(&self, _snapshot_path: &Path) -> Option<PathBuf> {
        None
    }
}
