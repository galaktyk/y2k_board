mod app;
mod board;
mod camera;
mod input;
mod renderer;
mod spatial;
mod stats;
mod toolbar;

use miniquad::conf;

fn main() {
    let conf = conf::Conf {
        window_title: "Quadboard".to_string(),
        window_width: 1280,
        window_height: 800,
        high_dpi: true,
        ..Default::default()
    };
    miniquad::start(conf, || Box::new(app::App::new()));
}

