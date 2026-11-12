//! The picture on the start-up splash card: a campfire at night, an original drawing with no text
//! in it, so the words stay translatable and are set by the program beside it. The drawing is the
//! sky, the trees, the stones and the logs; the fire on the logs is the app's own mark, painted
//! over it so the two always match.

use egui::{CornerRadius, Image, ImageSource, Rect, Ui, Vec2, pos2};

const SVG: &[u8] = include_bytes!("../../../../assets/ui/splash.svg");

/// The drawing's size in its own units; the picture keeps this aspect ratio.
pub const SIZE: Vec2 = Vec2::new(320.0, 340.0);

/// The fire in the drawing's units: its centre and size, standing on the logs drawn in the SVG
/// (crossed at 160, 278), so the two change together.
const FIRE: (f32, f32, f32) = (160.0, 204.0, 118.0);

/// The drawing alone, at `size`.
pub fn backdrop(size: Vec2) -> Image<'static> {
    Image::new(ImageSource::Bytes {
        uri: "bytes://flare-splash.svg".into(),
        bytes: egui::load::Bytes::Static(SVG),
    })
    .fit_to_exact_size(size)
}

/// Paints the whole picture into `rect`, rounded by `radius`. The caller gives `rect` the
/// drawing's aspect ratio; another one stretches it.
pub fn paint(ui: &mut Ui, rect: Rect, radius: u8) {
    backdrop(rect.size())
        .corner_radius(CornerRadius::same(radius))
        .paint_at(ui, rect);
    let fire = fire_rect(rect.size());
    crate::icons::flame(fire.width()).paint_at(ui, fire.translate(rect.min.to_vec2()));
}

/// Where the fire goes in a picture of `size`, from its top left.
fn fire_rect(size: Vec2) -> Rect {
    let scale = size / SIZE;
    let (x, y, side) = FIRE;
    Rect::from_center_size(pos2(x * scale.x, y * scale.y), Vec2::splat(side * scale.x))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn the_drawing_parses_opaque_at_its_own_aspect_ratio() {
        let rendered = egui_extras::image::load_svg_bytes_with_size(
            SVG,
            egui::SizeHint::Size {
                width: 320,
                height: 340,
                maintain_aspect_ratio: true,
            },
            &Default::default(),
        )
        .expect("the splash parses");
        assert_eq!(rendered.size, [320, 340]);
        assert!(
            rendered.pixels.iter().all(|p| p.a() == 255),
            "opaque drawing"
        );
    }

    #[test]
    fn the_fire_stands_in_the_middle_of_the_picture_on_the_logs() {
        let whole = Rect::from_min_size(egui::Pos2::ZERO, SIZE);
        let fire = fire_rect(SIZE);
        assert!(whole.contains_rect(fire), "{fire:?} leaves the picture");
        assert!((fire.center().x - SIZE.x / 2.0).abs() < 1.0, "{fire:?}");
        // The mark's flame fills its square to near the bottom, so the square ends on the logs.
        assert!((255.0..=285.0).contains(&fire.bottom()), "{fire:?}");
        let half = fire_rect(SIZE / 2.0);
        assert!(
            (half.width() * 2.0 - fire.width()).abs() < 1e-3,
            "scales with the picture"
        );
    }
}
