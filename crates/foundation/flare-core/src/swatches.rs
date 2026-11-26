//! The swatches a person picks colors from, as Animate's Swatches panel has them: the 216
//! web-safe colors (every mix of 0, 51, 102, 153, 204 and 255 in red, green and blue) in the
//! classic twelve-column layout, a row of greys, skin tones, and the person's own colors, kept in
//! a bounded list. The web-safe palette is a standard of the web, not any program's artwork; the
//! skin tones are Flare's own.

use crate::Rgba8;

/// How many colors of their own a person can keep.
pub const MAX_CUSTOM_SWATCHES: usize = 120;

/// The steps of each channel in the web-safe palette.
const STEPS: [u8; 6] = [0, 51, 102, 153, 204, 255];

/// The 216 web-safe colors, row by row in twelve columns: the red steps in pairs side by side,
/// green down the rows of each pair, blue across, as Flash and Animate lay them out.
pub fn web_safe() -> Vec<Rgba8> {
    let mut colors = Vec::with_capacity(216);
    for red_pair in 0..3 {
        for green in STEPS {
            for red in [STEPS[red_pair * 2], STEPS[red_pair * 2 + 1]] {
                for blue in STEPS {
                    colors.push(Rgba8::opaque(red, green, blue));
                }
            }
        }
    }
    colors
}

/// Twelve greys from black to white.
pub fn greys() -> Vec<Rgba8> {
    (0..12u16)
        .map(|i| {
            let v = u8::try_from(i * 255 / 11).unwrap_or(u8::MAX);
            Rgba8::opaque(v, v, v)
        })
        .collect()
}

/// Twelve skin tones from the fairest to the deepest, chosen by eye for characters in flat color.
const SKIN_BASES: [(u8, u8, u8); 12] = [
    (0xFD, 0xE7, 0xD6),
    (0xF8, 0xD5, 0xBE),
    (0xF3, 0xC4, 0xA5),
    (0xEB, 0xB0, 0x8C),
    (0xE0, 0xA0, 0x7A),
    (0xCF, 0x8B, 0x64),
    (0xBD, 0x77, 0x52),
    (0xA6, 0x64, 0x43),
    (0x8D, 0x52, 0x36),
    (0x74, 0x42, 0x2C),
    (0x5C, 0x34, 0x23),
    (0x45, 0x27, 0x1B),
];

/// Skin tones in two rows of twelve: the tones from fairest to deepest, and under each its shadow,
/// darker and warmer, because a cartoon's skin is a base color and one shade of it. Blue and green
/// fall faster than red so the shadow does not go grey.
pub fn skin_tones() -> Vec<Rgba8> {
    let shade = |v: u8, keep: u16| u8::try_from(u16::from(v) * keep / 100).unwrap_or(u8::MAX);
    let bases = SKIN_BASES.iter().map(|&(r, g, b)| Rgba8::opaque(r, g, b));
    let shadows = SKIN_BASES
        .iter()
        .map(|&(r, g, b)| Rgba8::opaque(shade(r, 84), shade(g, 74), shade(b, 70)));
    bases.chain(shadows).collect()
}

/// The person's own swatches, in the order added, each once, at most [`MAX_CUSTOM_SWATCHES`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CustomSwatches(Vec<Rgba8>);

impl CustomSwatches {
    /// The list from kept colors: repeats after the first and anything past the bound are dropped,
    /// so a damaged preferences file still gives a usable list.
    pub fn from_colors(colors: impl IntoIterator<Item = Rgba8>) -> Self {
        let mut list = Self::default();
        for color in colors {
            if list.0.len() >= MAX_CUSTOM_SWATCHES {
                break;
            }
            list.add(color);
        }
        list
    }

    pub fn colors(&self) -> &[Rgba8] {
        &self.0
    }

    /// Adds `color` at the end. A color already there is not added twice, and a full list refuses;
    /// either way the answer says whether it was added.
    pub fn add(&mut self, color: Rgba8) -> bool {
        if self.0.contains(&color) || self.0.len() >= MAX_CUSTOM_SWATCHES {
            return false;
        }
        self.0.push(color);
        true
    }

    /// Takes away the swatch at `index`; an index past the end changes nothing.
    pub fn remove(&mut self, index: usize) -> Option<Rgba8> {
        (index < self.0.len()).then(|| self.0.remove(index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_web_safe_palette_has_every_mix_once_in_twelve_columns() {
        let colors = web_safe();
        assert_eq!(colors.len(), 216);
        let unique: std::collections::HashSet<_> = colors.iter().map(|c| (c.r, c.g, c.b)).collect();
        assert_eq!(unique.len(), 216);
        assert!(colors.iter().all(|c| c.a == 255 && STEPS.contains(&c.r)));
        // The first row: red 0 then red 51, green 0, blue rising across each half.
        assert_eq!(colors[0], Rgba8::opaque(0, 0, 0));
        assert_eq!(colors[5], Rgba8::opaque(0, 0, 255));
        assert_eq!(colors[6], Rgba8::opaque(51, 0, 0));
        assert_eq!(
            colors[12],
            Rgba8::opaque(0, 51, 0),
            "green steps down the rows"
        );
        assert_eq!(colors[215], Rgba8::opaque(255, 255, 255));
    }

    #[test]
    fn greys_run_from_black_to_white() {
        let greys = greys();
        assert_eq!(greys.len(), 12);
        assert_eq!(greys[0], Rgba8::opaque(0, 0, 0));
        assert_eq!(greys[11], Rgba8::opaque(255, 255, 255));
        assert!(
            greys
                .windows(2)
                .all(|w| w[0].r < w[1].r && w[1].r == w[1].g)
        );
    }

    #[test]
    fn skin_tones_run_fair_to_deep_with_a_warmer_shadow_under_each() {
        let tones = skin_tones();
        assert_eq!(tones.len(), 24);
        let light = |c: &Rgba8| u32::from(c.r) * 299 + u32::from(c.g) * 587 + u32::from(c.b) * 114;
        let (bases, shadows) = tones.split_at(12);
        assert!(bases.windows(2).all(|w| light(&w[0]) > light(&w[1])));
        for (base, shadow) in bases.iter().zip(shadows) {
            assert!(light(shadow) < light(base), "{base:?} {shadow:?}");
            assert!(
                shadow.r >= shadow.g && shadow.g >= shadow.b,
                "warm: {shadow:?}"
            );
            assert_eq!(shadow.a, 255);
        }
        let unique: std::collections::HashSet<_> = tones.iter().map(|c| (c.r, c.g, c.b)).collect();
        assert_eq!(unique.len(), 24);
    }

    #[test]
    fn custom_swatches_are_unique_bounded_and_removable() {
        let mut list = CustomSwatches::default();
        let red = Rgba8::opaque(255, 0, 0);
        assert!(list.add(red));
        assert!(!list.add(red), "once");
        assert_eq!(list.remove(5), None);
        assert_eq!(list.remove(0), Some(red));
        for i in 0..200u8 {
            list.add(Rgba8::opaque(i, 1, 2));
        }
        assert_eq!(list.colors().len(), MAX_CUSTOM_SWATCHES);
        assert!(!list.add(Rgba8::opaque(1, 2, 3)), "full");
        let kept = CustomSwatches::from_colors(
            std::iter::repeat_n(red, 3).chain((0..200u8).map(|i| Rgba8::opaque(0, i, 0))),
        );
        assert_eq!(kept.colors().len(), MAX_CUSTOM_SWATCHES);
        assert_eq!(kept.colors()[0], red);
        assert_eq!(kept.colors()[1], Rgba8::opaque(0, 0, 0));
    }
}
