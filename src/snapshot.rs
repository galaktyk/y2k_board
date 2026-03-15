use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::board::{Board, Element};
use crate::platform::snapshot::{PlatformSnapshotAdapter, SnapshotPersistenceAdapter};

const SNAPSHOT_FILENAME: &str = "snapshot.bin";

#[derive(Clone, Debug)]
pub struct LoadedSnapshot {
    pub data: SnapshotData,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SnapshotData {
    pub elements: Vec<Element>,
    pub next_id: u64,
}

#[derive(Debug)]
pub enum SnapshotError {
    #[cfg(target_arch = "wasm32")]
    UnsupportedPlatform,
    Io(std::io::Error),
    Encode(bincode::Error),
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(target_arch = "wasm32")]
            SnapshotError::UnsupportedPlatform => {
                write!(f, "snapshot save/load is only implemented for native desktop builds")
            }
            SnapshotError::Io(err) => write!(f, "{err}"),
            SnapshotError::Encode(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for SnapshotError {}

impl From<std::io::Error> for SnapshotError {
    fn from(value: std::io::Error) -> Self {
        SnapshotError::Io(value)
    }
}

impl From<bincode::Error> for SnapshotError {
    fn from(value: bincode::Error) -> Self {
        SnapshotError::Encode(value)
    }
}

pub fn default_snapshot_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(SNAPSHOT_FILENAME)
}

pub fn snapshot_root(path: &Path) -> PathBuf {
    path.parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

pub fn snapshot_from_board(board: &Board) -> SnapshotData {
    SnapshotData {
        elements: board
            .elements
            .iter()
            .cloned()
            .map(|mut element| {
                element.selected = false;
                element
            })
            .collect(),
        next_id: board.next_available_id(),
    }
}

pub fn save_to_path(board: &Board, path: &Path) -> Result<PathBuf, SnapshotError> {
    PlatformSnapshotAdapter::new().save_to_path(&snapshot_from_board(board), path)
}

pub fn load_from_path(path: &Path) -> Result<LoadedSnapshot, SnapshotError> {
    PlatformSnapshotAdapter::new().load_from_path(path)
}