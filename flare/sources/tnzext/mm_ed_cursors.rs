//! Our own mouse pointers, drawn with tiny-skia and handed to the OS as bitmap
//! cursors: the Selection tool's black arrow and its badges, the hand, the
//! magnifier, the pencil and the Free Transform glyphs. Windows has no hand or
//! magnifier of its own (egui's Grab and ZoomIn become a four-way arrow and the
//! plain arrow there).

use std::collections::HashMap;

use eframe::egui::{self, CustomCursorImage};
use minnamate_core::tiny_skia as sk;

use crate::icons::svg_path;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Cursor {
    /// The Selection tool's black arrow, alone or with a badge for what a drag does:
    Arrow,
    /// moves what's under it,
    Move,
    /// bends the edge under it,
    Arc,
    /// moves the corner or end point under it,
    Corner,
    /// moves a copy (Alt).
    Copy,
    /// Hand tool, open and grabbing.
    Hand,
    Grab,
    /// Zoom tool, and with Alt.
    ZoomIn,
    ZoomOut,
    Pencil,
    /// Free Transform, just outside a corner and on a side.
    Rotate,
    SkewX,
    SkewY,
}

/// A part of a glyph on a 32-unit grid: dark parts are black with a white
/// rim, light ones white with a black rim, so every pointer shows on any color.
enum Part {
    Fill(&'static str, bool),
    Line(&'static str, f32, bool),
}
use Part::*;

const DARK: bool = false;
const LIGHT: bool = true;
const ARROW: &str = "M2 2 V19 L5.8 15.3 L8.5 21.3 L10.9 20.2 L8.3 14.4 H13.4 Z";
const LENS: &str = "M5 13 C5 8.6 8.6 5 13 5 C17.4 5 21 8.6 21 13 C21 17.4 17.4 21 13 21 C8.6 21 5 17.4 5 13 Z";
const PALM: &str = "M10.4 15 C10.4 13.9 11.2 13.2 12.2 13.2 H24 C25 13.2 25.6 13.9 25.6 15 V20.5 C25.6 25 22.6 28 18.2 28 C14.2 28 11.6 25.8 10.4 22 Z";

/// The glyph's parts, drawn in order, and its hotspot.
fn glyph(cursor: Cursor) -> (Vec<Part>, (f32, f32)) {
    let arrow = |badge: Vec<Part>| (std::iter::once(Fill(ARROW, DARK)).chain(badge).collect(), (2.0, 2.0));
    let lens = |sign: &'static str| {
        let parts = vec![Fill(LENS, LIGHT), Line(LENS, 1.8, DARK), Line("M19 19 L27 27", 3.2, DARK), Line(sign, 1.6, DARK)];
        (parts, (13.0, 13.0))
    };
    match cursor {
        Cursor::Arrow => arrow(Vec::new()),
        Cursor::Move => arrow(vec![
            Line("M21 16.5 V25.5 M16.5 21 H25.5", 1.0, DARK),
            Fill("M21 15 L19.3 17.2 H22.7 Z M21 27 L19.3 24.8 H22.7 Z M15 21 L17.2 19.3 V22.7 Z M27 21 L24.8 19.3 V22.7 Z", DARK),
        ]),
        Cursor::Arc => arrow(vec![Line("M16.5 24 Q21 17 25.5 24", 1.2, DARK)]),
        Cursor::Corner => arrow(vec![Line("M17.5 17.5 V24.5 H24.5", 1.2, DARK)]),
        Cursor::Copy => arrow(vec![Line("M21 16.5 V25.5 M16.5 21 H25.5", 1.4, DARK)]),
        Cursor::Hand => (
            vec![Line("M13 15 V7.5 M16.8 14 V5.5 M20.6 14 V7 M24.2 15.5 V10 M10.9 21 L6.9 16.5", 3.2, LIGHT), Fill(PALM, LIGHT)],
            (16.0, 16.0),
        ),
        Cursor::Grab => (
            vec![Line("M13 15 V12 M16.8 14 V11 M20.6 14 V11.5 M24.2 15.5 V13 M10.9 20.5 L8.6 18", 3.2, LIGHT), Fill(PALM, LIGHT)],
            (16.0, 16.0),
        ),
        Cursor::ZoomIn => lens("M13 9.5 V16.5 M9.5 13 H16.5"),
        Cursor::ZoomOut => lens("M9.5 13 H16.5"),
        Cursor::Pencil => (
            vec![
                Fill("M3.5 28.5 L6 21.5 L22.5 5 L27 9.5 L10.5 26 Z", LIGHT),
                Line("M3.5 28.5 L6 21.5 L22.5 5 L27 9.5 L10.5 26 Z M6 21.5 L10.5 26 M19.5 8 L24 12.5", 1.0, DARK),
                Fill("M3.5 28.5 L4.4 25.8 L6.2 27.6 Z", DARK),
            ],
            (3.5, 28.5),
        ),
        // Three quarters of a turn, clockwise from the left round to the bottom.
        Cursor::Rotate => (
            vec![
                Line("M9.5 16 C9.5 12.4 12.4 9.5 16 9.5 C19.6 9.5 22.5 12.4 22.5 16 C22.5 19.6 19.6 22.5 16 22.5", 1.5, DARK),
                Fill("M12 22.5 L16 19 V26 Z", DARK),
            ],
            (16.0, 16.0),
        ),
        // Two arrows sliding past each other, along x or along y.
        Cursor::SkewX => (vec![Line("M8 13 H21 M24 19 H11", 1.5, DARK), Fill("M24.5 13 L20.5 10 V16 Z M7.5 19 L11.5 16 V22 Z", DARK)], (16.0, 16.0)),
        Cursor::SkewY => (vec![Line("M19 8 V21 M13 24 V11", 1.5, DARK), Fill("M19 24.5 L22 20.5 H16 Z M13 7.5 L16 11.5 H10 Z", DARK)], (16.0, 16.0)),
    }
}

#[derive(Clone, Default)]
struct Cache(HashMap<(Cursor, u32), CustomCursorImage>);

/// Shows `cursor` as the pointer this frame, drawn once per size.
pub fn show(ctx: &egui::Context, cursor: Cursor) {
    let px = (32.0 * ctx.pixels_per_point()).round().clamp(32.0, 128.0) as u32;
    let id = egui::Id::new("minnamate-cursors");
    let cached = ctx.data_mut(|d| d.get_temp_mut_or_default::<Cache>(id).0.get(&(cursor, px)).cloned());
    let image = cached.or_else(|| {
        let image = draw(cursor, px)?;
        ctx.data_mut(|d| d.get_temp_mut_or_default::<Cache>(id).0.insert((cursor, px), image.clone()));
        Some(image)
    });
    ctx.set_cursor_image(image);
}

/// `px` square: every part's rim first, a unit wide, then the parts on top.
fn draw(cursor: Cursor, px: u32) -> Option<CustomCursorImage> {
    let (parts, (hx, hy)) = glyph(cursor);
    let mut pm = sk::Pixmap::new(px, px)?;
    let scale = px as f32 / 32.0;
    let ts = sk::Transform::from_scale(scale, scale);
    let paint = |white: bool| {
        let mut paint = sk::Paint::default();
        paint.set_color(if white { sk::Color::WHITE } else { sk::Color::BLACK });
        paint
    };
    let stroke = |width: f32| sk::Stroke { width, line_cap: sk::LineCap::Round, line_join: sk::LineJoin::Round, ..Default::default() };
    for rim in [true, false] {
        for part in &parts {
            match *part {
                Fill(d, light) if rim => pm.stroke_path(&svg_path(d)?, &paint(!light), &stroke(2.0), ts, None),
                Fill(d, light) => pm.fill_path(&svg_path(d)?, &paint(light), sk::FillRule::Winding, ts, None),
                Line(d, width, light) if rim => pm.stroke_path(&svg_path(d)?, &paint(!light), &stroke(width + 2.0), ts, None),
                Line(d, width, light) => pm.stroke_path(&svg_path(d)?, &paint(light), &stroke(width), ts, None),
            }
        }
    }
    let side = px as u16;
    let hotspot = [(hx * scale) as u16, (hy * scale) as u16];
    Some(CustomCursorImage { rgba: pm.take_demultiplied().into(), size: [side, side], hotspot })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cursor_draws() {
        use Cursor::*;
        for cursor in [Arrow, Move, Arc, Corner, Copy, Hand, Grab, ZoomIn, ZoomOut, Pencil, Rotate, SkewX, SkewY] {
            let image = draw(cursor, 64).expect("the glyph's paths parse");
            assert!(image.rgba.chunks(4).any(|px| px[3] > 0), "{cursor:?} draws something");
            assert!(image.size == [64, 64] && image.hotspot.iter().all(|&h| h < 64), "{cursor:?}");
        }
        // The arrow's tip is its hotspot.
        let arrow = draw(Cursor::Arrow, 32).unwrap();
        assert_eq!(arrow.hotspot, [2, 2]);
        assert!(arrow.rgba[(2 * 32 + 2) * 4 + 3] > 0);
    }
}
