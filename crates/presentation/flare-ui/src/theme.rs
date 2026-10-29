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
/// The red-orange of the flame in the app icon, for the flame mark.
pub const FLAME: Color32 = Color32::from_rgb(0xff, 0x5a, 0x2e);
pub const ACCENT: Color32 = Color32::from_rgb(0x4c, 0x8d, 0xff);
pub const WARNING: Color32 = Color32::from_rgb(0xe8, 0xb3, 0x4a);
pub const STAGE_SURROUND: Color32 = Color32::from_rgb(0x16, 0x18, 0x1c);

/// Source Sans 3 (SIL Open Font License; see `assets/ui/fonts/OFL.txt`).
const SOURCE_SANS: &[u8] = include_bytes!("../../../../assets/ui/fonts/SourceSans3-Regular.ttf");

/// Source Sans first for proportional text; egui's built-in fonts stay behind it for the glyphs
/// it lacks, and monospace is untouched.
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "source-sans-3".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(SOURCE_SANS)),
    );
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        family.insert(0, "source-sans-3".to_owned());
    }
    ctx.set_fonts(fonts);
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
}
