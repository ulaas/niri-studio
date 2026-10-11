mod app;
mod color;
mod pages;
mod preview;
mod sandbox;
mod shot;
mod theme;
mod widgets;
mod xkb;

use std::path::PathBuf;

fn main() -> eframe::Result {
    let path = match std::env::args().nth(1).as_deref() {
        Some("-h" | "--help") => {
            println!("usage: niri-studio [CONFIG]\n\nVisual editor for niri's config (default: $XDG_CONFIG_HOME/niri/config.kdl).");
            return Ok(());
        }
        Some(p) => PathBuf::from(p),
        None => doc::Document::default_path(),
    };

    let custom_titlebar = theme::Settings::load().custom_titlebar;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id("niri-studio")
            .with_title("niri studio")
            .with_decorations(!custom_titlebar)
            .with_inner_size([1280.0 * shot::scale(), 840.0 * shot::scale()])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "niri-studio",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, path)))),
    )
}
