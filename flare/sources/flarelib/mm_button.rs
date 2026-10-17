//! Button symbols: the Up, Over, Down and Hit frames of their timeline, and how a player shows
//! them under the pointer. The renderer draws hovered and pressed buttons through `shown_frame`;
//! the SWF export writes the four states as DefineButton2 (swf_export.rs).

use std::collections::BTreeSet;

use tiny_skia::Transform;

use crate::display::{Item, Placed, display_list, unmasked};
use crate::doc::{Document, Matrix, Point, Symbol, SymbolKind, Timeline};

/// The states a button's first four frames show, in order.
pub const STATES: [&str; 4] = ["Up", "Over", "Down", "Hit"];
pub const UP: usize = 0;
pub const OVER: usize = 1;
pub const DOWN: usize = 2;
pub const HIT: usize = 3;

/// The frame of a button's timeline that shows `state`. A timeline that stops short holds its last
/// frame, but for Hit, which falls back to the Up frame as Animate's help says.
// ponytail: Over and Down past a short timeline's end are assumed to hold its last frame; check
// against a SWF Animate publishes from a one-frame button.
pub fn frame(tl: &Timeline, state: usize) -> u32 {
    let count = tl.frame_count().max(1);
    match state as u32 {
        s if s < count => s,
        3 => 0,
        _ => count - 1,
    }
}

/// Where a player's pointer is, in the canvas's pixels, and whether its button is down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Live {
    pub pointer: Point,
    pub down: bool,
}

/// Whether `p`, in `sym`'s coordinates, is inside its Hit frame. Every layer counts, hidden ones too,
/// as they publish.
pub fn hits(doc: &Document, sym: &Symbol, p: Point) -> bool {
    let list = display_list(doc, &sym.timeline, frame(&sym.timeline, HIT), &mut BTreeSet::new());
    unmasked(list).iter().any(|q| crate::hit::shapes(doc, &q.item, q.matrix, 0).iter().any(|(s, m)| crate::hit::shape_hit(s, m, p, 0.0)))
}

/// The frame the renderer shows of a clip drawn through `ts`: a button under `live`'s pointer shows
/// its Over frame, or Down while pressed; every other clip, and a button left alone, its first.
// ponytail: every button under the pointer reacts, overlapping ones too, where a player picks the topmost.
pub(crate) fn shown_frame(doc: &Document, sym: &Symbol, kind: SymbolKind, ts: Transform, live: Option<Live>) -> u32 {
    let (Some(live), Some(inverse)) = (live.filter(|_| kind == SymbolKind::Button), ts.invert()) else { return 0 };
    let mut p = tiny_skia::Point::from_xy(live.pointer.0, live.pointer.1);
    inverse.map_point(&mut p);
    let state = match (hits(doc, sym, (p.x, p.y)), live.down) {
        (false, _) => UP,
        (true, false) => OVER,
        (true, true) => DOWN,
    };
    frame(&sym.timeline, state)
}

/// Whether a button in `list` (a display list, as display.rs makes them) is under `p`, given in the
/// listed timeline's coordinates: the player's pointer turns into a hand there.
pub fn under(doc: &Document, list: &[Placed], p: Point) -> bool {
    fn walk(doc: &Document, list: &[Placed], to_list: Matrix, p: Point, depth: u32) -> bool {
        list.iter().any(|q| {
            let m = to_list * q.matrix;
            match &q.item {
                Item::Clip(sym, SymbolKind::Button) => m.inverse().is_some_and(|inv| hits(doc, sym, inv.apply(p))),
                // Movie clips show their first frame, as the renderer draws them.
                Item::Clip(sym, _) if depth < 32 => walk(doc, &display_list(doc, &sym.timeline, 0, &mut BTreeSet::new()), m, p, depth + 1),
                Item::Masked { content, .. } => walk(doc, content, to_list, p, depth),
                _ => false,
            }
        })
    }
    walk(doc, list, Matrix::IDENTITY, p, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::*;
    use crate::draw;

    /// A button whose Up, Over, Down and Hit frames each hold a square `10 * (state + 1)` wide.
    fn button() -> Symbol {
        let mut timeline = Timeline::new("Button");
        let layer = &mut timeline.layers[0];
        for state in 0..4u32 {
            if state > 0 {
                layer.insert_blank_keyframe(state);
            }
            let side = 10.0 * (state + 1) as f32;
            let square = draw::rectangle([(0.0, 0.0), (side, side)], 0.0, None, Some(Paint::Solid([0.0, 0.0, 1.0, 1.0])));
            draw::add(&mut layer.frames[state as usize], square);
        }
        Symbol { kind: SymbolKind::Button, timeline, extra: Extra::default() }
    }

    #[test]
    fn states_and_their_frames() {
        let full = button();
        assert_eq!((0..4).map(|s| frame(&full.timeline, s)).collect::<Vec<_>>(), [0, 1, 2, 3]);
        // One frame: Up throughout, Hit included. Two: Down holds Over; Hit is Up.
        let one = Timeline::new("One");
        assert_eq!((0..4).map(|s| frame(&one, s)).collect::<Vec<_>>(), [0, 0, 0, 0]);
        let mut two = Timeline::new("Two");
        two.layers[0].insert_blank_keyframe(1);
        assert_eq!((0..4).map(|s| frame(&two, s)).collect::<Vec<_>>(), [0, 1, 1, 0]);
        // The Hit frame's 40 px square decides, not the Up frame's 10 px one.
        let doc = Document::new(100.0, 100.0, 24.0);
        assert!(hits(&doc, &full, (35.0, 35.0)) && !hits(&doc, &full, (45.0, 5.0)));
    }
}
