// Release builds are a GUI app, with no console window on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod arrange_panels;
mod color;
mod convert_symbol;
mod cursors;
mod eraser;
mod filters;
mod free_transform;
mod gradient_transform;
mod home;
mod icons;
mod layer_tree;
mod library;
mod menu;
mod paint_tools;
mod panels;
mod pen;
mod stage;
mod test_movie;
mod text_tool;
mod theme;
mod timeline;
mod tools;
mod tweens;
mod widgets;

use eframe::egui;

fn main() -> eframe::Result {
    let path = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Minnamate")
        .with_inner_size([1600.0, 940.0])
        // The app bar is the title bar, as in Animate on Windows.
        .with_decorations(false)
        .with_min_inner_size([960.0, 600.0]);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../../../assets/logo.png")) {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("Minnamate", options, Box::new(|cc| Ok(Box::new(app::Editor::new(&cc.egui_ctx, path)))))
}
