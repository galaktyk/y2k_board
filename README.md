for compress





# Build Windows
cargo build --release

## Compress with UPX
upx --best --lzma target/release/y2kboard.exe


# Build Web
cargo build --release --target wasm32-unknown-unknown

## Local testing
cd target/wasm32-unknown-unknown/release
python -m http.server 8000

## Host web
1. copy target/wasm32-unknown-unknown/release to build_web

copy font.js or .. if missing









-------
license

cursor https://www.void1gaming.com/free-basic-cursor-pack
font: W95FA.otf, notosans, deja vu sans