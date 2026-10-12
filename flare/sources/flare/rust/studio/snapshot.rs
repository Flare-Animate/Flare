//! Production UI screenshot, without opening or focusing a native window.
use egui_kittest::kittest::Queryable;
use flare_studio::{app::Preferences, commands::Command, i18n::Locale, Studio};
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args.get(1).cloned().unwrap_or_else(|| "studio.png".into());
    let locale = match args.get(2).map(String::as_str) {
        Some("fr") => Locale::French,
        Some("expanded") => Locale::Expanded,
        Some("rtl") => Locale::RtlPreview,
        _ => Locale::English,
    };
    let theme = if args.get(6).is_some_and(|s| s == "light") {
        flare_studio::theme::Theme::Light
    } else {
        flare_studio::theme::Theme::Dark
    };
    let width = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1440.0);
    let height = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(920.0);
    let hover = args.get(7).cloned();
    // Width/height are output pixels. Match the reference display scaling
    // explicitly rather than comparing 100% and 125% captures as equal geometry.
    let scale: f32 = args.get(9).and_then(|s| s.parse().ok()).unwrap_or(1.0);
    assert!(scale.is_finite() && (0.5..=3.0).contains(&scale));
    let mut setup = egui_kittest::wgpu::default_wgpu_setup();
    flare_studio::configure_renderer(&mut setup);
    let mut harness = egui_kittest::Harness::builder()
        .with_size(eframe::egui::vec2(width / scale, height / scale))
        .with_pixels_per_point(scale)
        .wgpu_setup(setup)
        .build_eframe(move |cc| {
            if let Some(state) = &cc.wgpu_render_state {
                let adapter = state.adapter.get_info();
                eprintln!(
                    "Snapshot adapter: {} ({:?}, {:?})",
                    adapter.name, adapter.backend, adapter.device_type
                );
            }
            let mut app = Studio::with_preferences(
                &cc.egui_ctx,
                Preferences {
                    theme,
                    locale,
                    rulers: args.get(5).is_some_and(|s| s == "rulers"),
                    frame_display: match args.get(8).map(String::as_str) {
                        Some("preview") => flare_studio::app::FrameDisplay::Preview,
                        Some("context") => flare_studio::app::FrameDisplay::Context,
                        _ => flare_studio::app::FrameDisplay::Cells,
                    },
                    ..Default::default()
                },
            );
            if args.get(5).is_some_and(|s| s == "demo") {
                app.dispatch(Command::Demo, &cc.egui_ctx);
            }
            if args.get(5).is_some_and(|s| s == "new") {
                app.dispatch(Command::New, &cc.egui_ctx);
            }
            app
        });
    for _ in 0..8 {
        harness.step();
    }
    if let Some(key) = hover.filter(|s| s != "none") {
        let label = flare_studio::i18n::Localizer::new(locale).text(&key);
        harness.get_by_label(&label).hover();
        for _ in 0..8 {
            harness.step();
        }
    }
    match harness
        .render()
        .and_then(|img| img.save(&out).map_err(|e| e.to_string()))
    {
        Ok(()) => println!("Wrote {out}"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
