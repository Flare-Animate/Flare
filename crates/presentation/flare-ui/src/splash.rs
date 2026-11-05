//! The picture behind the start-up splash: an original drawing with no text in it, so the words
//! stay translatable and are set by the program over it.

use egui::{Image, ImageSource, Vec2};

const SVG: &[u8] = include_bytes!("../../../../assets/ui/splash.svg");

/// The splash's size in points; the artwork's own aspect ratio.
pub const SIZE: Vec2 = Vec2::new(610.0, 344.0);

/// The backdrop, drawn at `size`.
pub fn backdrop(size: Vec2) -> Image<'static> {
    Image::new(ImageSource::Bytes {
        uri: "bytes://flare-splash.svg".into(),
        bytes: egui::load::Bytes::Static(SVG),
    })
    .fit_to_exact_size(size)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn the_backdrop_parses_and_has_the_splash_aspect_ratio() {
        let rendered = egui_extras::image::load_svg_bytes_with_size(
            SVG,
            egui::SizeHint::Size {
                width: 610,
                height: 344,
                maintain_aspect_ratio: true,
            },
            &Default::default(),
        )
        .expect("the splash parses");
        assert_eq!(rendered.size, [610, 344]);
        assert!(
            rendered.pixels.iter().all(|p| p.a() == 255),
            "opaque backdrop"
        );
        assert!((SIZE.x / SIZE.y - 610.0 / 344.0).abs() < 1e-4);
    }
}
