//! The look of the interface: one dark palette, spacing and rounding, applied to egui.
//!
//! Colours are named here and used by name, so changing the look is a change in this file.

use egui::{Color32, CornerRadius, FontId, Stroke, TextStyle, Visuals};

/// Which of the palettes is in use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    Darkest,
    #[default]
    Dark,
    Medium,
    Light,
    Lightest,
    HighContrast,
    Neutral,
    Blue,
    Teal,
    Green,
    Clay,
    Rose,
    Synthwave,
    LiquidGlass,
    Sepia,
    Mint,
}

impl Mode {
    /// Every theme, in the order Preferences lists them: the greys from darkest to lightest, as
    /// Animate's interface brightness runs, then high contrast, then the tinted ones.
    pub const ALL: [Self; 16] = [
        Self::Darkest,
        Self::Dark,
        Self::Medium,
        Self::Light,
        Self::Lightest,
        Self::HighContrast,
        Self::Neutral,
        Self::Blue,
        Self::Teal,
        Self::Green,
        Self::Clay,
        Self::Rose,
        Self::Synthwave,
        Self::LiquidGlass,
        Self::Sepia,
        Self::Mint,
    ];

    /// How it is kept in the preferences file.
    pub fn code(self) -> &'static str {
        match self {
            Self::Darkest => "darkest",
            Self::Dark => "dark",
            Self::Medium => "medium",
            Self::Light => "light",
            Self::Lightest => "lightest",
            Self::HighContrast => "contrast",
            Self::Neutral => "neutral",
            Self::Blue => "blue",
            Self::Teal => "teal",
            Self::Green => "green",
            Self::Clay => "clay",
            Self::Rose => "rose",
            Self::Synthwave => "synthwave",
            Self::LiquidGlass => "liquidglass",
            Self::Sepia => "sepia",
            Self::Mint => "mint",
        }
    }

    /// A kept value back; anything not understood is dark, the default.
    pub fn from_code(code: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|mode| mode.code() == code)
            .unwrap_or_default()
    }

    /// Whether the theme is drawn dark on light, with egui's light visuals under it.
    pub fn is_light(self) -> bool {
        palette_of(self).light
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

#[path = "theme_palettes.rs"]
mod palettes;
use palettes::*;

fn palette_of(mode: Mode) -> &'static Palette {
    match mode {
        Mode::Darkest => &DARKEST,
        Mode::Dark => &DARK,
        Mode::Medium => &MEDIUM,
        Mode::Light => &LIGHT,
        Mode::Lightest => &LIGHTEST,
        Mode::HighContrast => &HIGH_CONTRAST,
        Mode::Neutral => &NEUTRAL,
        Mode::Blue => &BLUE,
        Mode::Teal => &TEAL,
        Mode::Green => &GREEN,
        Mode::Clay => &CLAY,
        Mode::Rose => &ROSE,
        Mode::Synthwave => &SYNTHWAVE,
        Mode::LiquidGlass => &LIQUID_GLASS,
        Mode::Sepia => &SEPIA,
        Mode::Mint => &MINT,
    }
}

fn palette() -> &'static Palette {
    palette_of(mode())
}

pub fn bg() -> Color32 {
    palette().bg
}
/// Bars around and between panels: the menu bar, tab rows, the panel rail and the status bar.
/// A step between the ground and the panels, so a bar never runs into the gap beside it or the
/// panel under it.
pub fn chrome() -> Color32 {
    palette().chrome
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
/// Text and icons drawn on `fill`: white or black, whichever stands out more. An accent is blue in
/// some themes and yellow or pink in others, so white on the accent is not always readable.
pub fn ink_on(fill: Color32) -> Color32 {
    // The relative luminance at which white and black stand out equally (WCAG's formula).
    if relative_luminance(fill) > 0.179 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

/// WCAG relative luminance of a color. A see-through fill is measured as egui keeps it,
/// premultiplied, which is how it looks over a dark panel.
fn relative_luminance(c: Color32) -> f32 {
    let (r, g, b) = (c.r(), c.g(), c.b());
    let channel = |v: u8| {
        let v = f32::from(v) / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
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
pub const GUTTER: f32 = 3.0;

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
    let mut visuals = if mode().is_light() {
        Visuals::light()
    } else {
        Visuals::dark()
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
            .insert(TextStyle::Body, FontId::proportional(13.0));
        style
            .text_styles
            .insert(TextStyle::Button, FontId::proportional(13.0));
        style
            .text_styles
            .insert(TextStyle::Small, FontId::proportional(11.0));
        style
            .text_styles
            .insert(TextStyle::Heading, FontId::proportional(14.0));
    });
}

#[cfg(test)]
#[allow(clippy::expect_used)]
#[path = "theme_tests.rs"]
mod tests;
