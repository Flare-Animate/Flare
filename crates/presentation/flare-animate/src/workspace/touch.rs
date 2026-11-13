//! Touch: the sizes the interface uses when it is driven with fingers instead of a pointer.
//!
//! A phone or a tablet answers with bigger targets: the rail of panel icons, the dock's tab
//! bar and egui's own widgets all grow, so a thumb lands on what it aims at. The choice is
//! made by the surface (the web build detects a phone; the Android shell always says yes) and
//! given to the workspace with [`Workspace::set_touch`].

use super::*;

/// The rail's width with a pointer, and with fingers.
pub(super) fn rail_width(touch: bool) -> f32 {
    if touch { 52.0 } else { 34.0 }
}

/// One rail button's square side.
pub(super) fn rail_button(touch: bool) -> f32 {
    if touch { 44.0 } else { 28.0 }
}

/// The icon inside a rail button.
pub(super) fn rail_icon(touch: bool) -> f32 {
    if touch { 24.0 } else { 18.0 }
}

/// The dock's tab bar height: a tab is a finger's target when the panel has to be switched.
pub(super) fn tab_bar(touch: bool) -> f32 {
    if touch {
        34.0
    } else {
        super::dock::TAB_BAR_HEIGHT
    }
}

/// The spacing and sizes egui's own widgets use with fingers, laid over the palette's base
/// style. Larger everywhere a press lands: buttons, menu items, icons and the gaps between.
pub(super) fn apply_touch_style(ctx: &egui::Context) {
    ctx.global_style_mut(|style| {
        let spacing = &mut style.spacing;
        spacing.interact_size.y = 40.0;
        spacing.button_padding = egui::vec2(12.0, 10.0);
        spacing.menu_margin = egui::Margin::symmetric(10, 9);
        spacing.window_margin = egui::Margin::same(12);
        spacing.icon_width = 24.0;
        spacing.icon_width_inner = 20.0;
        spacing.icon_spacing = 8.0;
        spacing.item_spacing = egui::vec2(8.0, 7.0);
        spacing.indent = 20.0;
    });
}

impl Workspace {
    /// Restyles for `touch` (or back to the pointer style) when the choice changes, and
    /// again whenever the palette was re-applied under it, which resets the spacing.
    pub(super) fn ensure_touch_style(&mut self, ctx: &egui::Context) {
        if self.styled_touch == Some(self.touch) {
            return;
        }
        self.styled_touch = Some(self.touch);
        flare_ui::theme::apply_palette(ctx);
        if self.touch {
            apply_touch_style(ctx);
        }
    }
}

#[cfg(test)]
#[path = "touch_tests.rs"]
mod tests;
