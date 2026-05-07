# Y2K Board

## Build Windows
cargo build --release

### Compress with UPX
upx --best --lzma target/release/y2kboard.exe

## Build Web
cargo web-build

The web build assembles a self-contained site in `target/wasm32-unknown-unknown/release`.

### Local testing
cd target/wasm32-unknown-unknown/release
python -m http.server 8000

### GitHub Pages
The GitHub Pages workflow builds from source and deploys `target/wasm32-unknown-unknown/release`.

-------
## License

cursor https://www.void1gaming.com/free-basic-cursor-pack  
font: W95FA.otf, notosans, deja vu sans