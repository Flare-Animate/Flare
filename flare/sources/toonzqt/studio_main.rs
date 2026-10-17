#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() -> eframe::Result {
    let mut wgpu_options = eframe::egui_wgpu::WgpuConfiguration::default();
    flare_studio::configure_renderer(&mut wgpu_options.wgpu_setup);
    eframe::run_native(
        "Flare Studio",
        eframe::NativeOptions {
            wgpu_options,
            viewport: eframe::egui::ViewportBuilder::default()
                .with_decorations(!cfg!(target_os = "windows"))
                // Same icon as the main Qt application (Flare.ico), embedded at
                // compile time. Without this the window showed the generic
                // default executable icon.
                .with_icon(flare_studio::icon::icon_data())
                .with_inner_size([1440.0, 920.0])
                .with_min_inner_size([800.0, 600.0]),
            ..Default::default()
        },
        Box::new(|cc| {
            let app = flare_studio::Studio::new(cc);
            // eframe 0.36 waits for a render before showing the root, while its
            // hidden-root path skips rendering. Show once, after initialization;
            // later minimize/hide requests remain entirely under user control.
            if let Some(window) = cc.winit_window() {
                window.set_visible(true);
                window.request_redraw();
            }
            Ok(Box::new(app))
        }),
    )
}
