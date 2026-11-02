//! Aligning and distributing.

use emath::Rect;

/// Which edge or center the Align panel lines shapes up on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    CenterH,
    Right,
    Top,
    CenterV,
    Bottom,
}

/// How far each of `rects` moves to line up on `target`'s edge or center.
pub fn align_deltas(rects: &[Rect], how: Align, target: Rect) -> Vec<emath::Vec2> {
    rects
        .iter()
        .map(|r| match how {
            Align::Left => emath::vec2(target.left() - r.left(), 0.0),
            Align::CenterH => emath::vec2(target.center().x - r.center().x, 0.0),
            Align::Right => emath::vec2(target.right() - r.right(), 0.0),
            Align::Top => emath::vec2(0.0, target.top() - r.top()),
            Align::CenterV => emath::vec2(0.0, target.center().y - r.center().y),
            Align::Bottom => emath::vec2(0.0, target.bottom() - r.bottom()),
        })
        .collect()
}

/// How far each of `rects` moves to leave equal gaps between neighbours along the axis, the two
/// outermost staying put. When the shapes are too big for that (together they are longer than the
/// space between the outer edges, so the gaps would be negative and could even reorder them),
/// their centers are spaced evenly instead, which keeps their order. Fewer than three
/// rectangles have nothing to distribute.
pub fn distribute_deltas(rects: &[Rect], horizontal: bool) -> Vec<emath::Vec2> {
    let mut deltas = vec![emath::Vec2::ZERO; rects.len()];
    if rects.len() < 3 {
        return deltas;
    }
    let span = |r: &Rect| {
        if horizontal {
            (r.left(), r.right())
        } else {
            (r.top(), r.bottom())
        }
    };
    let lo = |r: &Rect| span(r).0;
    let hi = |r: &Rect| span(r).1;
    let size = |r: &Rect| hi(r) - lo(r);
    let mut order: Vec<usize> = (0..rects.len()).collect();
    order.sort_by(|a, b| lo(&rects[*a]).total_cmp(&lo(&rects[*b])));
    let first = lo(&rects[order[0]]);
    let last = order
        .iter()
        .map(|i| hi(&rects[*i]))
        .fold(f32::MIN, f32::max);
    let total: f32 = rects.iter().map(size).sum();
    let gap = (last - first - total) / (rects.len() - 1) as f32;
    if gap < 0.0 {
        let center = |i: usize| (lo(&rects[i]) + hi(&rects[i])) / 2.0;
        let mut by_center: Vec<usize> = (0..rects.len()).collect();
        by_center.sort_by(|a, b| center(*a).total_cmp(&center(*b)));
        let (c0, c1) = (center(by_center[0]), center(by_center[by_center.len() - 1]));
        let step = (c1 - c0) / (rects.len() - 1) as f32;
        for (rank, index) in by_center.into_iter().enumerate() {
            let shift = c0 + step * rank as f32 - center(index);
            deltas[index] = if horizontal {
                emath::vec2(shift, 0.0)
            } else {
                emath::vec2(0.0, shift)
            };
        }
        return deltas;
    }
    let mut at = first;
    for index in order {
        let shift = at - lo(&rects[index]);
        deltas[index] = if horizontal {
            emath::vec2(shift, 0.0)
        } else {
            emath::vec2(0.0, shift)
        };
        at += size(&rects[index]) + gap;
    }
    deltas
}
