#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

const INIT_SIZE: egui::Vec2 = egui::Vec2::new(400.0, 600.0);

fn main() -> eframe::Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_target(false)
        .init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(INIT_SIZE)
            .with_min_inner_size(INIT_SIZE)
            .with_title("Coze"),
        ..Default::default()
    };
    eframe::run_native(
        "coze",
        native_options,
        Box::new(|cc| Box::new(coze::App::new(cc))),
    )
}