//! The look of the interface: one dark palette, spacing and rounding, applied to egui.
//!
//! Colours are named here and used by name, so changing the look is a change in this file.

use egui::{Color32, CornerRadius, FontId, Stroke, TextStyle, Visuals};

pub const BG: Color32 = Color32::from_rgb(0x1d, 0x20, 0x25);
pub const PANEL: Color32 = Color32::from_rgb(0x25, 0x28, 0x2e);
pub const PANEL_RAISED: Color32 = Color32::from_rgb(0x2d, 0x31, 0x38);
pub const FIELD: Color32 = Color32::from_rgb(0x1a, 0x1c, 0x20);
pub const BORDER: Color32 = Color32::from_rgb(0x3a, 0x3f, 0x47);
pub const HOVER: Color32 = Color32::from_rgb(0x39, 0x3e, 0x47);
pub const TEXT: Color32 = Color32::from_rgb(0xd9, 0xdd, 0xe3);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x9a, 0xa1, 0xac);
pub const TEXT_DISABLED: Color32 = Color32::from_rgb(0x5d, 0x63, 0x6d);
pub const ACCENT: Color32 = Color32::from_rgb(0x4c, 0x8d, 0xff);
pub const WARNING: Color32 = Color32::from_rgb(0xe8, 0xb3, 0x4a);
/// A button that throws work away.
pub const DANGER: Color32 = Color32::from_rgb(0xc2, 0x3b, 0x3b);
pub const STAGE_SURROUND: Color32 = Color32::from_rgb(0x16, 0x18, 0x1c);

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
    let mut visuals = Visuals::dark();
    visuals.panel_fill = PANEL;
    visuals.window_fill = PANEL_RAISED;
    visuals.extreme_bg_color = FIELD;
    visuals.faint_bg_color = Color32::from_rgb(0x29, 0x2d, 0x33);
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.override_text_color = Some(TEXT);
    visuals.hyperlink_color = ACCENT;
    visuals.selection.bg_fill = ACCENT.gamma_multiply(0.45);
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
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
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.bg_fill = PANEL_RAISED;
    visuals.widgets.inactive.weak_bg_fill = PANEL_RAISED;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.hovered.bg_fill = HOVER;
    visuals.widgets.hovered.weak_bg_fill = HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x55, 0x5b, 0x66));
    visuals.widgets.active.bg_fill = ACCENT.gamma_multiply(0.55);
    visuals.widgets.active.weak_bg_fill = ACCENT.gamma_multiply(0.55);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
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
}
