//! The look of the interface: one dark palette, spacing and rounding, applied to egui.
//!
//! Colours are named here and used by name, so changing the look is a change in this file.

use egui::{Color32, CornerRadius, FontId, Stroke, TextStyle, Visuals};

/// Which of the two palettes is in use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Dark,
    Light,
}

impl Mode {
    /// How it is kept in the preferences file.
    pub fn code(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    /// A kept value back; anything not understood is dark, the default.
    pub fn from_code(code: &str) -> Self {
        if code == "light" {
            Self::Light
        } else {
            Self::Dark
        }
    }
}

thread_local! {
    // The interface is drawn on one thread, and each test runs on its own, so a test that draws
    // the light palette cannot change the colors another is looking at.
    static MODE: std::cell::Cell<Mode> = const { std::cell::Cell::new(Mode::Dark) };
}

/// Chooses the palette the color functions below answer from, for this thread.
pub fn set_mode(mode: Mode) {
    MODE.with(|m| m.set(mode));
}

pub fn mode() -> Mode {
    MODE.with(std::cell::Cell::get)
}

struct Palette {
    bg: Color32,
    panel: Color32,
    panel_raised: Color32,
    field: Color32,
    faint: Color32,
    border: Color32,
    border_strong: Color32,
    hover: Color32,
    text: Color32,
    text_strong: Color32,
    text_dim: Color32,
    text_disabled: Color32,
    accent: Color32,
    warning: Color32,
    stage_surround: Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

const DARK: Palette = Palette {
    bg: rgb(0x1d, 0x20, 0x25),
    panel: rgb(0x25, 0x28, 0x2e),
    panel_raised: rgb(0x2d, 0x31, 0x38),
    field: rgb(0x1a, 0x1c, 0x20),
    faint: rgb(0x29, 0x2d, 0x33),
    border: rgb(0x3a, 0x3f, 0x47),
    border_strong: rgb(0x55, 0x5b, 0x66),
    hover: rgb(0x39, 0x3e, 0x47),
    text: rgb(0xd9, 0xdd, 0xe3),
    text_strong: Color32::WHITE,
    text_dim: rgb(0x9a, 0xa1, 0xac),
    text_disabled: rgb(0x5d, 0x63, 0x6d),
    accent: rgb(0x4c, 0x8d, 0xff),
    warning: rgb(0xe8, 0xb3, 0x4a),
    stage_surround: rgb(0x16, 0x18, 0x1c),
};

const LIGHT: Palette = Palette {
    bg: rgb(0xe3, 0xe6, 0xeb),
    panel: rgb(0xf1, 0xf3, 0xf6),
    panel_raised: rgb(0xfb, 0xfc, 0xfd),
    field: rgb(0xe4, 0xe7, 0xec),
    faint: rgb(0xec, 0xee, 0xf2),
    border: rgb(0xc6, 0xcb, 0xd3),
    border_strong: rgb(0xa5, 0xac, 0xb7),
    hover: rgb(0xdf, 0xe3, 0xe9),
    text: rgb(0x22, 0x26, 0x2c),
    text_strong: Color32::BLACK,
    text_dim: rgb(0x58, 0x5f, 0x6a),
    text_disabled: rgb(0xa2, 0xa8, 0xb2),
    accent: rgb(0x2c, 0x6c, 0xe0),
    warning: rgb(0xa8, 0x6a, 0x10),
    stage_surround: rgb(0xcd, 0xd1, 0xd8),
};

fn palette() -> &'static Palette {
    match mode() {
        Mode::Dark => &DARK,
        Mode::Light => &LIGHT,
    }
}

pub fn bg() -> Color32 {
    palette().bg
}
pub fn panel() -> Color32 {
    palette().panel
}
pub fn panel_raised() -> Color32 {
    palette().panel_raised
}
pub fn field() -> Color32 {
    palette().field
}
pub fn border() -> Color32 {
    palette().border
}
pub fn hover() -> Color32 {
    palette().hover
}
pub fn text() -> Color32 {
    palette().text
}
/// Text and icons that are lit: hovered or chosen. White on the dark palette, black on the light.
pub fn text_strong() -> Color32 {
    palette().text_strong
}
pub fn text_dim() -> Color32 {
    palette().text_dim
}
pub fn text_disabled() -> Color32 {
    palette().text_disabled
}
pub fn accent() -> Color32 {
    palette().accent
}
pub fn warning() -> Color32 {
    palette().warning
}
/// A button that throws work away.
pub fn danger() -> Color32 {
    Color32::from_rgb(0xc2, 0x3b, 0x3b)
}
pub fn stage_surround() -> Color32 {
    palette().stage_surround
}

/// Source Sans 3 (SIL Open Font License; see `assets/ui/fonts/OFL.txt`).
const SOURCE_SANS: &[u8] = include_bytes!("../../../../assets/ui/fonts/SourceSans3-Regular.ttf");

/// The fonts: Source Sans first for proportional text, then `cjk` (a system font with Chinese,
/// Japanese and Korean characters) when there is one, with egui's built-in fonts behind them for
/// the glyphs they lack. Monospace is untouched.
fn definitions(cjk: Option<egui::FontData>) -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "source-sans-3".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(SOURCE_SANS)),
    );
    let mut order = vec!["source-sans-3".to_owned()];
    if let Some(data) = cjk {
        fonts
            .font_data
            .insert("system-cjk".to_owned(), std::sync::Arc::new(data));
        order.push("system-cjk".to_owned());
    }
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        for (at, name) in order.into_iter().enumerate() {
            family.insert(at, name);
        }
    }
    fonts
}

fn install_fonts(ctx: &egui::Context) {
    ctx.set_fonts(definitions(None));
}

/// The system fonts that carry Chinese and Japanese characters for the language `code` (`ja` or
/// `zh-CN`), best first, with the face to use in a collection. The two languages draw some of the
/// same characters differently, so each looks first for a font made for it.
pub fn cjk_font_candidates(code: &str) -> Vec<(&'static str, u32)> {
    let japanese = code.starts_with("ja");
    let mut found: Vec<(&'static str, u32)> = Vec::new();
    if japanese {
        found.extend([
            ("/System/Library/Fonts/\u{30d2}\u{30e9}\u{30ae}\u{30ce}\u{89d2}\u{30b4}\u{30b7}\u{30c3}\u{30af} W3.ttc", 0),
            ("C:\\Windows\\Fonts\\YuGothM.ttc", 0),
            ("C:\\Windows\\Fonts\\meiryo.ttc", 0),
            ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 0),
            ("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", 0),
            ("/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc", 0),
            ("/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc", 0),
        ]);
    } else {
        found.extend([
            ("/System/Library/Fonts/PingFang.ttc", 0),
            ("C:\\Windows\\Fonts\\msyh.ttc", 0),
            ("C:\\Windows\\Fonts\\simsun.ttc", 0),
            ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 2),
            ("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", 2),
            (
                "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
                2,
            ),
            ("/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc", 2),
            ("/usr/share/fonts/truetype/wqy/wqy-microhei.ttc", 0),
        ]);
    }
    // The other language's fonts after, then one font that covers most of both.
    found.extend([
        // macOS keeps no Japanese-specific font in this folder on every version; PingFang has the
        // kana, with Chinese forms of the characters they share.
        ("/System/Library/Fonts/PingFang.ttc", 0),
        ("/System/Library/Fonts/Hiragino Sans GB.ttc", 0),
        ("/Library/Fonts/Arial Unicode.ttf", 0),
        ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 0),
    ]);
    found
}

/// The largest font file read, so an odd file in a font folder cannot take the memory.
const MAX_FONT_BYTES: u64 = 64 * 1024 * 1024;

/// Adds the system's Chinese and Japanese characters to the interface's fonts, for the language
/// `code`. Nothing changes when the system has no such font.
pub fn add_cjk_font(ctx: &egui::Context, code: &str) {
    for (path, face) in cjk_font_candidates(code) {
        let Ok(meta) = std::fs::metadata(path) else {
            continue;
        };
        if meta.len() > MAX_FONT_BYTES {
            continue;
        }
        if let Ok(bytes) = std::fs::read(path) {
            let mut data = egui::FontData::from_owned(bytes);
            data.index = face;
            ctx.set_fonts(definitions(Some(data)));
            return;
        }
    }
}

/// The gutter between docked panels and around the dock.
pub const GUTTER: f32 = 4.0;

/// Space between a docked panel's edge and its content (horizontal, vertical).
pub const PANEL_PADDING: (i8, i8) = (10, 8);

/// Applies the palette, spacing and type sizes to `ctx`.
pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);
    apply_palette(ctx);
}

/// The colors, spacing and type sizes of the palette in use, without touching the fonts: called
/// again when the person switches between the dark and the light one.
pub fn apply_palette(ctx: &egui::Context) {
    let mut visuals = match mode() {
        Mode::Dark => Visuals::dark(),
        Mode::Light => Visuals::light(),
    };
    visuals.panel_fill = panel();
    visuals.window_fill = panel_raised();
    visuals.extreme_bg_color = field();
    visuals.faint_bg_color = palette().faint;
    visuals.window_stroke = Stroke::new(1.0, border());
    visuals.override_text_color = Some(text());
    visuals.hyperlink_color = accent();
    visuals.selection.bg_fill = accent().gamma_multiply(0.45);
    visuals.selection.stroke = Stroke::new(1.0, accent());
    let round = CornerRadius::same(5);
    visuals.window_corner_radius = CornerRadius::same(8);
    visuals.menu_corner_radius = CornerRadius::same(6);
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = round;
    }
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, border());
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, text());
    visuals.widgets.inactive.bg_fill = panel_raised();
    visuals.widgets.inactive.weak_bg_fill = panel_raised();
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, border());
    visuals.widgets.hovered.bg_fill = hover();
    visuals.widgets.hovered.weak_bg_fill = hover();
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, palette().border_strong);
    visuals.widgets.active.bg_fill = accent().gamma_multiply(0.55);
    visuals.widgets.active.weak_bg_fill = accent().gamma_multiply(0.55);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, accent());
    ctx.set_visuals(visuals);

    ctx.global_style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(8.0, 3.0);
        style.spacing.interact_size.y = 22.0;
        style.spacing.window_margin = egui::Margin::same(10);
        style.spacing.menu_margin = egui::Margin::symmetric(8, 6);
        style.spacing.indent = 14.0;
        style
            .text_styles
            .insert(TextStyle::Body, FontId::proportional(14.0));
        style
            .text_styles
            .insert(TextStyle::Button, FontId::proportional(14.0));
        style
            .text_styles
            .insert(TextStyle::Small, FontId::proportional(12.0));
        style
            .text_styles
            .insert(TextStyle::Heading, FontId::proportional(16.0));
    });
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
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

    #[test]
    fn each_cjk_language_looks_first_for_its_own_font_and_only_for_whole_paths() {
        let ja = cjk_font_candidates("ja");
        let zh = cjk_font_candidates("zh-CN");
        assert_ne!(ja[0], zh[0], "the two draw some characters differently");
        assert!(
            ja[0].0.contains("Hiragino") || ja[0].0.contains("\u{30d2}") || ja[0].0.contains("Yu")
        );
        assert!(zh[0].0.contains("PingFang"));
        for (path, _) in ja.iter().chain(&zh) {
            assert!(
                path.starts_with('/') || path.starts_with("C:\\"),
                "{path} is not an absolute path"
            );
        }
        assert!(ja.len() < 40 && zh.len() < 40);
    }

    #[test]
    fn a_missing_font_changes_nothing_and_a_found_one_goes_after_the_interface_font() {
        let ctx = egui::Context::default();
        apply(&ctx);
        add_cjk_font(&ctx, "ja"); // there may be no such font here: that must be harmless
        let plain = definitions(None);
        assert_eq!(
            plain.families[&egui::FontFamily::Proportional][0],
            "source-sans-3"
        );
        let with = definitions(Some(egui::FontData::from_static(SOURCE_SANS)));
        let family = &with.families[&egui::FontFamily::Proportional];
        assert_eq!(
            (family[0].as_str(), family[1].as_str()),
            ("source-sans-3", "system-cjk")
        );
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
        for mode in [Mode::Dark, Mode::Light] {
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
        for m in [Mode::Dark, Mode::Light] {
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
}
