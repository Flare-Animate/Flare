// Release builds are a GUI app, with no console window on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[path = "mm_ed_app.rs"]
mod app;
#[path = "mm_ed_arrange_panels.rs"]
mod arrange_panels;
#[path = "mm_ed_color.rs"]
mod color;
#[path = "mm_ed_convert_symbol.rs"]
mod convert_symbol;
#[path = "mm_ed_cursors.rs"]
mod cursors;
#[path = "mm_ed_eraser.rs"]
mod eraser;
#[path = "mm_ed_filters.rs"]
mod filters;
#[path = "mm_ed_free_transform.rs"]
mod free_transform;
#[path = "mm_ed_gradient_transform.rs"]
mod gradient_transform;
#[path = "mm_ed_home.rs"]
mod home;
#[path = "mm_ed_icons.rs"]
mod icons;
#[path = "mm_ed_layer_tree.rs"]
mod layer_tree;
#[path = "mm_ed_library.rs"]
mod library;
#[path = "mm_ed_menu.rs"]
mod menu;
#[path = "mm_ed_paint_tools.rs"]
mod paint_tools;
#[path = "mm_ed_panels.rs"]
mod panels;
#[path = "mm_ed_pen.rs"]
mod pen;
#[path = "mm_ed_stage.rs"]
mod stage;
#[path = "mm_ed_test_movie.rs"]
mod test_movie;
#[path = "mm_ed_text_tool.rs"]
mod text_tool;
#[path = "mm_ed_theme.rs"]
mod theme;
#[path = "mm_ed_timeline.rs"]
mod timeline;
#[path = "mm_ed_tools.rs"]
mod tools;
#[path = "mm_ed_tweens.rs"]
mod tweens;
#[path = "mm_ed_widgets.rs"]
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
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("assets/logo.png")) {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("Minnamate", options, Box::new(|cc| Ok(Box::new(app::Editor::new(&cc.egui_ctx, path)))))
}
