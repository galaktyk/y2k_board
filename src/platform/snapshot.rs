use std::path::{Path, PathBuf};

use crate::snapshot::{LoadedSnapshot, SnapshotData, SnapshotError};

pub(crate) trait SnapshotPersistenceAdapter {
    fn save_to_path(&self, snapshot: &SnapshotData, path: &Path) -> Result<PathBuf, SnapshotError>;
    fn load_from_path(&self, path: &Path) -> Result<LoadedSnapshot, SnapshotError>;
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
}
