use std::path::{Path, PathBuf};
use std::collections::HashSet;
use crate::board::Element;
use crate::snapshot;

pub fn pick_save_path(snapshot_path: &Path) -> Option<PathBuf> {
    let default_name = snapshot_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("snapshot.bin");

    rfd::FileDialog::new()
        .add_filter("miniGalaktyk Snapshots", &["bin"])
        .set_directory(snapshot::snapshot_root(snapshot_path))
        .set_file_name(default_name)
        .save_file()
}

pub fn pick_load_path(snapshot_path: &Path) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("miniGalaktyk Snapshots", &["bin"])
        .set_directory(snapshot::snapshot_root(snapshot_path))
        .pick_file()
}

pub fn copy_assets(elements: &[Element], source_root: &Path, target_root: &Path) -> std::io::Result<()> {
    if source_root == target_root {
        return Ok(());
    }

    let mut copied_paths = HashSet::new();
    for element in elements {
        let Some(image) = element.image.as_ref() else {
            continue;
        };

        for relative_path in std::iter::once(image.asset_path.as_str())
            .chain(image.hires_asset_path.iter().map(String::as_str))
        {
            if !copied_paths.insert(relative_path.to_string()) {
                continue;
            }

            let source_path = source_root.join(relative_path);
            let target_path = target_root.join(relative_path);
            if source_path == target_path {
                continue;
            }

            if let Some(parent) = target_path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }

            std::fs::copy(&source_path, &target_path)?;
        }
    }

    Ok(())
}
