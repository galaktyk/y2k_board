mod app;
mod board;
mod camera;
mod clipboard;
mod debug;
mod images;
mod input;
mod platform;
mod renderer;
mod rendering;
mod palette;
mod services;
mod snapshot;
mod spatial;
mod stats;
mod text;
mod tool;
mod toolbar;
mod ui;

use miniquad::conf;

fn main() {
    let conf = conf::Conf {
        window_title: "Quadboard".to_string(),
        window_width: 1280,
        window_height: 800,
        high_dpi: true,
        platform: conf::Platform {
            blocking_event_loop: true,
            ..Default::default()
        },
        ..Default::default()
    };
    miniquad::start(conf, || Box::new(app::App::new()));
}

