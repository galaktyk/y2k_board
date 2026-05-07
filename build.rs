use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn copy_if_different(source: &Path, dest: &Path) {
    let source_bytes = fs::read(source).expect("source web asset should be readable");
    let dest_matches = fs::read(dest)
        .map(|dest_bytes| dest_bytes == source_bytes)
        .unwrap_or(false);
    if dest_matches {
        return;
    }

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("destination directory should be created");
    }
    fs::write(dest, source_bytes).expect("web asset should be copied to target output");
}

fn copy_dir_contents(source_dir: &Path, dest_dir: &Path) {
    let entries = fs::read_dir(source_dir).expect("source directory should be readable");
    for entry in entries {
        let entry = entry.expect("directory entry should be readable");
        let source_path = entry.path();
        let dest_path = dest_dir.join(entry.file_name());
        let file_type = entry.file_type().expect("file type should be readable");
        if file_type.is_dir() {
            copy_dir_contents(&source_path, &dest_path);
        } else if file_type.is_file() {
            copy_if_different(&source_path, &dest_path);
        }
    }
}

fn wasm_output_dir() -> PathBuf {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir should be set"));
    let target_root = env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| manifest_dir.join("target"));
    let target_triple = env::var("TARGET").expect("target triple should be set");
    let profile = env::var("PROFILE").expect("profile should be set");
    target_root.join(target_triple).join(profile)
}

fn sync_wasm_web_assets() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir should be set"));
    let web_dir = manifest_dir.join("web");
    let cursor_dir = manifest_dir.join("assets").join("cursor");
    let fonts_dir = manifest_dir.join("fonts");
    let output_dir = wasm_output_dir();

    copy_if_different(&web_dir.join("gl.js"), &output_dir.join("gl.js"));
    copy_if_different(&web_dir.join("index.html"), &output_dir.join("index.html"));
    copy_if_different(&web_dir.join("font.js"), &output_dir.join("font.js"));
    copy_if_different(&web_dir.join("favicon.ico"), &output_dir.join("favicon.ico"));
    copy_if_different(
        &cursor_dir.join("default_cursor.png"),
        &output_dir.join("cursor").join("default_cursor.png"),
    );
    copy_if_different(
        &cursor_dir.join("caret.png"),
        &output_dir.join("cursor").join("caret.png"),
    );
    copy_if_different(
        &cursor_dir.join("pointer_cursor.png"),
        &output_dir.join("cursor").join("pointer_cursor.png"),
    );
    copy_if_different(
        &cursor_dir.join("sticky_cursor.png"),
        &output_dir.join("cursor").join("sticky_cursor.png"),
    );
    copy_dir_contents(&fonts_dir, &output_dir.join("fonts"));
}

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=fonts");
    println!("cargo:rerun-if-changed=web/font.js");
    println!("cargo:rerun-if-changed=web/favicon.ico");
    println!("cargo:rerun-if-changed=assets/cursor/caret.png");
    println!("cargo:rerun-if-changed=assets/cursor/default_cursor.png");
    println!("cargo:rerun-if-changed=assets/cursor/pointer_cursor.png");
    println!("cargo:rerun-if-changed=assets/cursor/sticky_cursor.png");
    println!("cargo:rerun-if-changed=web/gl.js");
    println!("cargo:rerun-if-changed=web/index.html");

    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        sync_wasm_web_assets();
    }

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resources = winres::WindowsResource::new();
        resources.set_icon("assets/icon.ico");
        resources
            .compile()
            .expect("windows icon resource should compile");
    }
}
