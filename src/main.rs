mod app;
mod board;
mod camera;
mod clipboard;
mod debug;
mod images;
mod input;
mod platform;
mod rendering;
mod palette;
mod services;
mod snapshot;
mod spatial;
mod stats;
mod text;

mod ui;

use miniquad::conf;

fn resized_icon<const N: usize>(image: &image::DynamicImage, size: u32) -> [u8; N] {
    image
        .resize_exact(size, size, image::imageops::FilterType::Lanczos3)
        .to_rgba8()
        .into_raw()
        .try_into()
        .expect("resized icon should match expected RGBA byte count")
}

fn window_icon() -> conf::Icon {
    let image = image::load_from_memory_with_format(
        include_bytes!("../assets/icon.ico"),
        image::ImageFormat::Ico,
    )
    .expect("window icon should decode");

    conf::Icon {
        small: resized_icon::<{ 16 * 16 * 4 }>(&image, 16),
        medium: resized_icon::<{ 32 * 32 * 4 }>(&image, 32),
        big: resized_icon::<{ 64 * 64 * 4 }>(&image, 64),
    }
}

fn main() {
    let conf = conf::Conf {
        window_title: "Y2KBoard".to_string(),
        window_width: 1280,
        window_height: 800,
        high_dpi: true,
        icon: Some(window_icon()),
        platform: conf::Platform {
            blocking_event_loop: true,
            ..Default::default()
        },
        ..Default::default()
    };
    miniquad::start(conf, || Box::new(app::App::new()));
}

