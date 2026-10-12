//! The Flare application icon for the native Studio window.
//!
//! The main Qt application gets its icon from `flare/sources/flare/Flare.ico`
//! (wired up by `flare.rc`). That file is a PNG-in-ICO container, so Rust cannot
//! read it without pulling in a PNG decoder — which would mean a new crate and
//! Cargo.lock churn for one asset.
//!
//! Instead `tools/icon/extract-ico.mjs` extracts that exact payload to
//! `assets/flare-icon.png` and decodes it to raw RGBA at
//! `assets/flare-icon.rgba`. `include_bytes!` embeds it at compile time, so
//! there is no runtime file lookup and no extra dependency, and the Studio
//! window shows the same icon as the main application rather than the default
//! executable icon.
//!
//! Regenerate with:
//!     node tools/icon/extract-ico.mjs && node tools/icon/verify-ico.mjs

use eframe::egui;

/// Width and height in pixels of `assets/flare-icon.rgba`.
pub const ICON_SIZE: usize = 256;

/// Raw, unpremultiplied RGBA, straight from `Flare.ico`.
const RGBA: &[u8] = include_bytes!("assets/flare-icon.rgba");

/// The Flare icon, ready for `ViewportBuilder::with_icon`.
pub fn icon_data() -> egui::IconData {
    egui::IconData {
        rgba: RGBA.to_vec(),
        width: ICON_SIZE as u32,
        height: ICON_SIZE as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_rgba_matches_its_declared_geometry() {
        // A short buffer here means egui would read past the end and show
        // garbage or panic, so this is the check that matters.
        assert_eq!(
            RGBA.len(),
            ICON_SIZE * ICON_SIZE * 4,
            "flare-icon.rgba must be {} bytes ({}x{}x4)",
            ICON_SIZE * ICON_SIZE * 4,
            ICON_SIZE,
            ICON_SIZE
        );
    }

    #[test]
    fn icon_is_a_real_image_not_blank_or_noise() {
        let px = |i: usize| RGBA[i * 4 + 3]; // alpha
        let mut clear = 0usize;
        let mut solid = 0usize;
        let n = ICON_SIZE * ICON_SIZE;
        for i in 0..n {
            match px(i) {
                0 => clear += 1,
                255 => solid += 1,
                _ => {}
            }
        }
        assert!(clear > 0, "icon has no fully transparent pixels");
        assert!(solid > 0, "icon has no fully opaque pixels");

        // Real artwork has large flat regions; a mis-decoded PNG is noise.
        let mut same = 0usize;
        for y in 0..ICON_SIZE {
            for x in 0..ICON_SIZE - 1 {
                let a = (y * ICON_SIZE + x) * 4;
                let b = a + 4;
                if RGBA[a..a + 4] == RGBA[b..b + 4] {
                    same += 1;
                }
            }
        }
        let flat = same as f32 / ((ICON_SIZE * (ICON_SIZE - 1)) as f32);
        assert!(
            flat > 0.3,
            "icon looks like noise ({:.1}% identical neighbour pairs)",
            flat * 100.0
        );
    }

    #[test]
    fn icon_data_carries_the_embedded_pixels() {
        let data = icon_data();
        assert_eq!(data.width, ICON_SIZE as u32);
        assert_eq!(data.height, ICON_SIZE as u32);
        assert_eq!(data.rgba.len(), RGBA.len());
        assert_eq!(&data.rgba[..4], &RGBA[..4]);
    }
}