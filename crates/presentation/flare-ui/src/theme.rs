//! The look of the interface: one dark palette, spacing and rounding, applied to egui.
//!
//! Colours are named here and used by name, so changing the look is a change in this file.

use egui::{Color32, CornerRadius, FontId, Stroke, TextStyle, Visuals};

/// Which of the palettes is in use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Dark,
    Light,
    Clay,
    Synthwave,
    Blue,
    Neutral,
    Green,
    LiquidGlass,
}

impl Mode {
    /// How it is kept in the preferences file.
    pub fn code(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::Clay => "clay",
            Self::Synthwave => "synthwave",
            Self::Blue => "blue",
            Self::Neutral => "neutral",
            Self::Green => "green",
            Self::LiquidGlass => "liquidglass",
        }
    }

    /// A kept value back; anything not understood is dark, the default.
    pub fn from_code(code: &str) -> Self {
        match code {
            "light" => Self::Light,
            "clay" => Self::Clay,
            "synthwave" => Self::Synthwave,
            "blue" => Self::Blue,
            "neutral" => Self::Neutral,
            "green" => Self::Green,
            "liquidglass" => Self::LiquidGlass,
            _ => Self::Dark,
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

const CLAY: Palette = Palette {
    bg: rgb(0x2a, 0x24, 0x1e),
    panel: rgb(0x33, 0x2c, 0x24),
    panel_raised: rgb(0x3d, 0x35, 0x2c),
    field: rgb(0x22, 0x1d, 0x18),
    faint: rgb(0x38, 0x30, 0x28),
    border: rgb(0x5a, 0x4e, 0x40),
    border_strong: rgb(0x7a, 0x6a, 0x55),
    hover: rgb(0x45, 0x3b, 0x30),
    text: rgb(0xe8, 0xdd, 0xcc),
    text_strong: Color32::WHITE,
    text_dim: rgb(0xb0, 0xa0, 0x88),
    text_disabled: rgb(0x6a, 0x5e, 0x50),
    accent: rgb(0xd4, 0x8a, 0x4a),
    warning: rgb(0xe8, 0xb3, 0x4a),
    stage_surround: rgb(0x1e, 0x18, 0x14),
};

const SYNTHWAVE: Palette = Palette {
    bg: rgb(0x1a, 0x0a, 0x2e),
    panel: rgb(0x22, 0x10, 0x38),
    panel_raised: rgb(0x2a, 0x14, 0x44),
    field: rgb(0x14, 0x08, 0x24),
    faint: rgb(0x26, 0x12, 0x3e),
    border: rgb(0x5a, 0x2a, 0x7a),
    border_strong: rgb(0x8a, 0x4a, 0xaa),
    hover: rgb(0x3a, 0x1a, 0x5a),
    text: rgb(0xe0, 0xd0, 0xff),
    text_strong: Color32::WHITE,
    text_dim: rgb(0xa0, 0x80, 0xcc),
    text_disabled: rgb(0x5a, 0x40, 0x7a),
    accent: rgb(0xff, 0x4a, 0xc8),
    warning: rgb(0xff, 0xb3, 0x4a),
    stage_surround: rgb(0x12, 0x06, 0x20),
};

const BLUE: Palette = Palette {
    bg: rgb(0x1a, 0x20, 0x2e),
    panel: rgb(0x20, 0x28, 0x38),
    panel_raised: rgb(0x28, 0x32, 0x44),
    field: rgb(0x14, 0x1a, 0x28),
    faint: rgb(0x24, 0x2e, 0x40),
    border: rgb(0x3a, 0x4a, 0x6a),
    border_strong: rgb(0x55, 0x6a, 0x8a),
    hover: rgb(0x2e, 0x3a, 0x52),
    text: rgb(0xd0, 0xdc, 0xf0),
    text_strong: Color32::WHITE,
    text_dim: rgb(0x8a, 0x9a, 0xbb),
    text_disabled: rgb(0x4a, 0x5a, 0x7a),
    accent: rgb(0x4a, 0x8d, 0xff),
    warning: rgb(0xe8, 0xb3, 0x4a),
    stage_surround: rgb(0x12, 0x18, 0x24),
};

const NEUTRAL: Palette = Palette {
    bg: rgb(0x2a, 0x2a, 0x2a),
    panel: rgb(0x33, 0x33, 0x33),
    panel_raised: rgb(0x3d, 0x3d, 0x3d),
    field: rgb(0x22, 0x22, 0x22),
    faint: rgb(0x38, 0x38, 0x38),
    border: rgb(0x55, 0x55, 0x55),
    border_strong: rgb(0x77, 0x77, 0x77),
    hover: rgb(0x40, 0x40, 0x40),
    text: rgb(0xd9, 0xd9, 0xd9),
    text_strong: Color32::WHITE,
    text_dim: rgb(0x99, 0x99, 0x99),
    text_disabled: rgb(0x5a, 0x5a, 0x5a),
    accent: rgb(0x88, 0x88, 0x88),
    warning: rgb(0xe8, 0xb3, 0x4a),
    stage_surround: rgb(0x1e, 0x1e, 0x1e),
};

const GREEN: Palette = Palette {
    bg: rgb(0x1a, 0x24, 0x1a),
    panel: rgb(0x20, 0x2e, 0x20),
    panel_raised: rgb(0x28, 0x38, 0x28),
    field: rgb(0x14, 0x1e, 0x14),
    faint: rgb(0x24, 0x32, 0x24),
    border: rgb(0x3a, 0x5a, 0x3a),
    border_strong: rgb(0x55, 0x7a, 0x55),
    hover: rgb(0x2e, 0x42, 0x2e),
    text: rgb(0xd0, 0xe8, 0xd0),
    text_strong: Color32::WHITE,
    text_dim: rgb(0x8a, 0xaa, 0x8a),
    text_disabled: rgb(0x4a, 0x6a, 0x4a),
    accent: rgb(0x4a, 0xc8, 0x4a),
    warning: rgb(0xe8, 0xb3, 0x4a),
    stage_surround: rgb(0x12, 0x1c, 0x12),
};

const LIQUID_GLASS: Palette = Palette {
    bg: rgb(0x17, 0x21, 0x31),
    panel: rgb(0x1f, 0x2b, 0x3f),
    panel_raised: rgb(0x22, 0x2f, 0x45),
    field: rgb(0x0f, 0x16, 0x21),
    faint: rgb(0x26, 0x32, 0x40),
    border: rgb(0x4a, 0x5e, 0x80),
    border_strong: rgb(0x7d, 0x8f, 0xb0),
    hover: rgb(0x2e, 0x3e, 0x58),
    text: rgb(0xe6, 0xed, 0xf6),
    text_strong: Color32::WHITE,
    text_dim: rgb(0x9f, 0xb0, 0xc8),
    text_disabled: rgb(0x78, 0x86, 0xa0),
    accent: rgb(0x5f, 0xb4, 0xf0),
    warning: rgb(0xe8, 0xb3, 0x4a),
    stage_surround: rgb(0x0f, 0x16, 0x21),
};

fn palette() -> &'static Palette {
    match mode() {
        Mode::Dark => &DARK,
        Mode::Light => &LIGHT,
        Mode::Clay => &CLAY,
        Mode::Synthwave => &SYNTHWAVE,
        Mode::Blue => &BLUE,
        Mode::Neutral => &NEUTRAL,
        Mode::Green => &GREEN,
        Mode::LiquidGlass => &LIQUID_GLASS,
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

/// Noto Sans CJK (SIL Open Font License; see `assets/ui/fonts/OFL-NotoSansCJK.txt`), cut down to
/// the characters of each language's national character set and of its translations by
/// `scripts/fonts/subset_cjk.py`. Japanese, Chinese and Korean draw some of the same characters
/// differently, so each has its own.
const CJK_JA: &[u8] = include_bytes!("../../../../assets/ui/fonts/NotoSansCJK-ja-subset.otf");
const CJK_ZH: &[u8] = include_bytes!("../../../../assets/ui/fonts/NotoSansCJK-zh-subset.otf");
const CJK_KO: &[u8] = include_bytes!("../../../../assets/ui/fonts/NotoSansCJK-ko-subset.otf");

/// The CJK fonts, as (the language code that picks it, its name in the font definitions, data).
const CJK_FONTS: [(&str, &str, &[u8]); 3] = [
    ("zh-CN", "noto-cjk-zh", CJK_ZH),
    ("ja", "noto-cjk-ja", CJK_JA),
    ("ko", "noto-cjk-ko", CJK_KO),
];

/// A font family that draws only in the CJK font of `code` (`ja`, `zh-CN` or `ko`): for a name
/// that has to look right whatever language the interface is in, such as the language list's own
/// entries.
pub fn cjk_family(code: &str) -> Option<egui::FontFamily> {
    CJK_FONTS
        .iter()
        .find(|(c, _, _)| *c == code)
        .map(|(_, name, _)| egui::FontFamily::Name((*name).into()))
}

/// The fonts: Source Sans first for proportional text, then the CJK fonts with the one for `code`
/// (the interface's language) ahead of the others, then egui's built-in fonts for the glyphs
/// these lack. Monospace is untouched.
fn definitions(code: &str) -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "source-sans-3".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(SOURCE_SANS)),
    );
    let mut order: Vec<&str> = Vec::new();
    for (language, name, data) in CJK_FONTS {
        fonts.font_data.insert(
            name.to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(data)),
        );
        fonts
            .families
            .insert(egui::FontFamily::Name(name.into()), vec![name.to_owned()]);
        // A language's own font first; for the rest, Chinese, Japanese and Korean in turn, so
        // a name typed in any of them shows.
        if language == code {
            order.insert(0, name);
        } else {
            order.push(name);
        }
    }
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        family.insert(0, "source-sans-3".to_owned());
        for (at, name) in order.into_iter().enumerate() {
            family.insert(at + 1, name.to_owned());
        }
    }
    fonts
}

fn install_fonts(ctx: &egui::Context) {
    ctx.set_fonts(definitions("en"));
}

/// Puts the CJK font of the interface's language `code` ahead of the other two, so characters the
/// languages share are drawn the way that language draws them.
pub fn set_cjk_priority(ctx: &egui::Context, code: &str) {
    ctx.set_fonts(definitions(code));
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
        Mode::Light => Visuals::light(),
        Mode::Dark
        | Mode::Clay
        | Mode::Synthwave
        | Mode::Blue
        | Mode::Neutral
        | Mode::Green
        | Mode::LiquidGlass => Visuals::dark(),
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
#[path = "theme_tests.rs"]
mod tests;
