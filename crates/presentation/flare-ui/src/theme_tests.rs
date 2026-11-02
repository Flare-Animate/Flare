use super::*;

#[test]
fn spacing_leaves_room_between_and_around_things() {
    let ctx = egui::Context::default();
    apply(&ctx);
    let spacing = ctx.global_style().spacing.clone();
    assert!(spacing.item_spacing.x >= 6.0 && spacing.item_spacing.y >= 4.0);
    assert!(spacing.window_margin.left >= 10);
    assert!(i16::from(PANEL_PADDING.0) > i16::from(GUTTER as i8));
    assert!(spacing.interact_size.y >= 22.0);
}

fn proportional(code: &str) -> Vec<String> {
    definitions(code).families[&egui::FontFamily::Proportional].clone()
}

#[test]
fn the_interface_font_is_first_and_the_languages_own_cjk_font_leads_the_others() {
    for (code, own) in [
        ("ja", "noto-cjk-ja"),
        ("zh-CN", "noto-cjk-zh"),
        ("ko", "noto-cjk-ko"),
    ] {
        let family = proportional(code);
        assert_eq!(family[0], "source-sans-3");
        assert_eq!(family[1], own, "{code}");
        assert_eq!(
            family.iter().filter(|f| f.starts_with("noto-cjk")).count(),
            3,
            "a name in any CJK language still shows"
        );
    }
    // For a language without one, all three follow, in a fixed order.
    assert_eq!(
        proportional("de")[..4],
        ["source-sans-3", "noto-cjk-zh", "noto-cjk-ja", "noto-cjk-ko"]
    );
    assert!(cjk_family("ja").is_some() && cjk_family("de").is_none());
}

#[test]
fn egui_loads_each_embedded_font_and_finds_the_languages_own_name_in_it() {
    let ctx = egui::Context::default();
    apply(&ctx);
    set_cjk_priority(&ctx, "ja");
    // Fonts take a pass to be set up.
    for step in 0..2 {
        let input = egui::RawInput {
            time: Some(f64::from(step)),
            ..egui::RawInput::default()
        };
        ctx.run_ui(input, |_| {}).textures_delta.clear();
    }
    for (code, name) in [("ja", "日本語"), ("zh-CN", "简体中文"), ("ko", "한국어")] {
        let family = cjk_family(code).expect("family");
        let found = ctx.fonts_mut(|f| f.has_glyphs(&FontId::new(14.0, family), name));
        assert!(found, "{code}'s font draws {name}");
    }
}

/// WCAG relative luminance of a color.
fn luminance(c: Color32) -> f32 {
    let channel = |v: u8| {
        let v = f32::from(v) / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
}

fn contrast(a: Color32, b: Color32) -> f32 {
    let (hi, lo) = (
        luminance(a).max(luminance(b)),
        luminance(a).min(luminance(b)),
    );
    (hi + 0.05) / (lo + 0.05)
}

#[test]
fn both_palettes_keep_text_readable_on_every_surface_and_the_light_one_is_light() {
    for mode in [
        Mode::Dark,
        Mode::Light,
        Mode::Clay,
        Mode::Synthwave,
        Mode::Blue,
        Mode::Neutral,
        Mode::Green,
        Mode::LiquidGlass,
    ] {
        set_mode(mode);
        for surface in [bg(), panel(), panel_raised(), field(), hover()] {
            assert!(
                contrast(text(), surface) >= 7.0,
                "{mode:?} text on {surface:?}"
            );
            assert!(
                contrast(text_dim(), surface) >= 3.5,
                "{mode:?} dim text on {surface:?}"
            );
            assert!(
                contrast(text_strong(), surface) >= contrast(text(), surface),
                "{mode:?} lit text is the stronger"
            );
        }
        assert!(
            contrast(text_disabled(), panel()) >= 1.8,
            "{mode:?} disabled is still seen"
        );
        assert!(contrast(border(), panel()) >= 1.1, "{mode:?} borders show");
        assert!(contrast(accent(), panel()) >= 3.0, "{mode:?} accent shows");
    }
    set_mode(Mode::Dark);
    let dark_panel = luminance(panel());
    set_mode(Mode::Light);
    assert!(luminance(panel()) > 0.7 && dark_panel < 0.05);
    assert!(
        luminance(field()) < luminance(panel_raised()),
        "the inset box is the lower surface"
    );
    set_mode(Mode::Dark);
}

#[test]
fn the_mode_is_per_thread_kept_by_code_and_unknown_codes_are_dark() {
    assert_eq!(mode(), Mode::Dark, "dark is the default");
    set_mode(Mode::Light);
    let other = std::thread::spawn(mode).join().expect("thread");
    assert_eq!(other, Mode::Dark, "another thread is not affected");
    assert_eq!(mode(), Mode::Light);
    set_mode(Mode::Dark);
    for m in [
        Mode::Dark,
        Mode::Light,
        Mode::Clay,
        Mode::Synthwave,
        Mode::Blue,
        Mode::Neutral,
        Mode::Green,
        Mode::LiquidGlass,
    ] {
        assert_eq!(Mode::from_code(m.code()), m);
    }
    assert_eq!(Mode::from_code("neon"), Mode::Dark);
    assert_eq!(Mode::from_code(""), Mode::Dark);
}

#[test]
fn the_palette_is_applied_to_eguis_own_widgets_in_both_modes() {
    let ctx = egui::Context::default();
    set_mode(Mode::Light);
    apply_palette(&ctx);
    let light = ctx.global_style().visuals.clone();
    assert!(!light.dark_mode && light.panel_fill == panel());
    set_mode(Mode::Dark);
    apply_palette(&ctx);
    let dark = ctx.global_style().visuals.clone();
    assert!(dark.dark_mode && dark.panel_fill == panel());
    assert_ne!(light.panel_fill, dark.panel_fill);
}
