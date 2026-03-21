for compress

upx --best --lzma target/release/minigalaktyk.exe



# Build Windows
cargo build --release


# Build WASM
cargo build --release --target wasm32-unknown-unknown