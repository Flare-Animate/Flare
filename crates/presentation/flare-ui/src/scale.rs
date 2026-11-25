//! How large the whole interface is drawn: one factor over every panel, icon, row and text,
//! as Animate's interface scaling is, applied as egui's zoom so nothing is sized twice.

/// The sizes Preferences offers, as factors of the interface's own design size.
pub const SIZES: [f32; 7] = [0.9, 1.0, 1.15, 1.25, 1.5, 1.75, 2.0];

/// The size a person starts with: a step above the design size, which read as small beside
/// Animate's.
pub const DEFAULT: f32 = 1.15;

/// The smallest and largest factor kept; a file asking for more or less is brought into range.
pub const MIN: f32 = 0.75;
pub const MAX: f32 = 2.0;

/// The smallest area the workspace is laid out for, in points (`MIN_WINDOW_SIZE` in the
/// workspace), which a larger size must still leave it.
pub const LAYOUT_MIN: [f32; 2] = [960.0, 600.0];

/// `chosen` in range; anything not a number is the default.
pub fn clamp(chosen: f32) -> f32 {
    if chosen.is_finite() {
        chosen.clamp(MIN, MAX)
    } else {
        DEFAULT
    }
}

/// The factor to draw at in a window `native` points across and down (its size at a factor of
/// one). A size above the design size is lowered as far as it must be for the workspace to keep
/// its smallest layout, so a small window is never cut off for it; it never goes below the
/// design size on that account, and a size below it the person chose is kept.
pub fn effective(chosen: f32, native: [f32; 2]) -> f32 {
    let chosen = clamp(chosen);
    if chosen <= 1.0 {
        return chosen;
    }
    let fits = (native[0] / LAYOUT_MIN[0]).min(native[1] / LAYOUT_MIN[1]);
    if !fits.is_finite() {
        return chosen;
    }
    chosen.min(fits).max(1.0)
}

/// Draws `ctx` at `chosen`, as far as its window allows (see [`effective`]). Changed only when
/// it differs, since a change lays the whole interface out again on the next pass.
pub fn apply(ctx: &egui::Context, chosen: f32) {
    let current = ctx.zoom_factor();
    let size = ctx.content_rect().size() * current;
    let wanted = effective(chosen, [size.x, size.y]);
    if (wanted - current).abs() > 0.001 {
        ctx.set_zoom_factor(wanted);
    }
}

/// How a size is named in Preferences: a percentage.
pub fn label(size: f32) -> String {
    format!("{}%", (clamp(size) * 100.0).round() as u32)
}

#[cfg(test)]
#[path = "scale_tests.rs"]
mod tests;
