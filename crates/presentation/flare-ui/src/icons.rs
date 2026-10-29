//! Icons: SVGs from `assets/ui/icons`, drawn at any size and tinted by state.
//!
//! The artwork is white, so a tint is the whole colour: brighter when hovered, accented when
//! active, dim when disabled. The files are compiled in (see `build.rs`); the egui image
//! loaders rasterize them at the size they are asked for, and cache the result.

include!(concat!(env!("OUT_DIR"), "/icon_table.rs"));

use egui::{Color32, Image, ImageSource, Vec2};

/// The SVG source of an icon, if there is one by that name.
pub fn source(name: &str) -> Option<&'static [u8]> {
    TABLE
        .binary_search_by_key(&name, |(n, _)| n)
        .ok()
        .map(|i| TABLE[i].1)
}

/// Whether an icon of this name exists.
pub fn exists(name: &str) -> bool {
    source(name).is_some()
}

/// Makes the SVG loaders available. Call once per context; calling again is harmless.
pub fn install(ctx: &egui::Context) {
    egui_extras::install_image_loaders(ctx);
}

/// An icon as an egui image of the given size and tint. A name with no icon draws nothing
/// visible (a transparent square), so a typo never breaks a layout.
pub fn image(name: &str, size: f32, tint: Color32) -> Image<'static> {
    debug_assert!(exists(name), "there is no icon named {name:?}");
    let bytes = source(name).unwrap_or(BLANK);
    let uri = format!("bytes://flare-icon/{name}.svg");
    Image::new(ImageSource::Bytes {
        uri: uri.into(),
        bytes: egui::load::Bytes::Static(bytes),
    })
    .fit_to_exact_size(Vec2::splat(size))
    .tint(tint)
}

/// Every icon's name, sorted.
pub fn names() -> impl Iterator<Item = &'static str> {
    TABLE.iter().map(|(name, _)| *name)
}

const BLANK: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"/>"#;

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_sorted_and_has_no_duplicates_so_lookup_works() {
        assert!(TABLE.len() > 100, "{} icons", TABLE.len());
        for pair in TABLE.windows(2) {
            assert!(pair[0].0 < pair[1].0, "{} then {}", pair[0].0, pair[1].0);
        }
    }

    #[test]
    fn every_icon_parses_and_draws_something_on_a_transparent_background() {
        for (name, svg) in TABLE {
            let rendered = egui_extras::image::load_svg_bytes_with_size(
                svg,
                egui::SizeHint::Size {
                    width: 48,
                    height: 48,
                    maintain_aspect_ratio: true,
                },
                &Default::default(),
            )
            .unwrap_or_else(|e| panic!("{name} does not render: {e}"));
            let inked = rendered.pixels.iter().filter(|p| p.a() > 0).count();
            let total = rendered.pixels.len();
            assert!(inked > 20, "{name} draws almost nothing ({inked} pixels)");
            assert!(inked < total * 9 / 10, "{name} fills its whole square");
        }
    }

    /// Arrows that point along an axis are drawn so they look balanced, not so their box is.
    const OPTICALLY_BALANCED: [&str; 3] = ["transport-play", "transport-prev", "transport-next"];

    #[test]
    fn every_icon_is_centered_in_its_square_so_buttons_line_up() {
        for (name, svg) in TABLE {
            if OPTICALLY_BALANCED.contains(name) {
                continue;
            }
            let rendered = egui_extras::image::load_svg_bytes_with_size(
                svg,
                egui::SizeHint::Size {
                    width: 96,
                    height: 96,
                    maintain_aspect_ratio: true,
                },
                &Default::default(),
            )
            .unwrap_or_else(|e| panic!("{name} does not render: {e}"));
            let [width, height] = rendered.size;
            let (mut x0, mut x1, mut y0, mut y1) = (width, 0, height, 0);
            for (i, pixel) in rendered.pixels.iter().enumerate() {
                if pixel.a() > 20 {
                    let (x, y) = (i % width, i / width);
                    (x0, x1, y0, y1) = (x0.min(x), x1.max(x + 1), y0.min(y), y1.max(y + 1));
                }
            }
            // In 24-unit icon coordinates, how far the drawing's box is from the middle.
            let off_x = ((x0 + x1) as f32 / 2.0 - width as f32 / 2.0) * 24.0 / width as f32;
            let off_y = ((y0 + y1) as f32 / 2.0 - height as f32 / 2.0) * 24.0 / height as f32;
            assert!(
                off_x.abs() <= 0.8 && off_y.abs() <= 0.8,
                "{name} sits off-center by ({off_x:.2}, {off_y:.2}) units"
            );
        }
    }

    #[test]
    fn icons_are_white_so_a_tint_is_the_whole_colour() {
        for (name, svg) in TABLE {
            let text = std::str::from_utf8(svg).expect("utf-8");
            assert!(
                text.contains("viewBox=\"0 0 24 24\""),
                "{name}: not 24 x 24"
            );
            let colours: Vec<&str> = text
                .match_indices('#')
                .map(|(i, _)| text.get(i..i + 4).unwrap_or("#"))
                .collect();
            assert!(
                colours.iter().all(|c| *c == "#fff"),
                "{name} uses a colour other than white: {colours:?}"
            );
        }
    }

    #[test]
    fn lookup_finds_icons_by_name() {
        assert!(exists("tool-selection"));
        assert!(!exists("no-such-icon"));
        assert!(source("no-such-icon").is_none());
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "there is no icon named")]
    fn asking_for_a_missing_icon_is_caught_in_debug_builds() {
        let _ = image("no-such-icon", 16.0, Color32::WHITE);
    }
}
