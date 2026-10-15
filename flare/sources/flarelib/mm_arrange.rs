//! Element commands of the Edit and Modify menus: copy and paste, Duplicate,
//! Group, Ungroup, Break Apart, Arrange and Align. Each works on one timeline,
//! on elements of the keyframes showing at one frame, and returns where the
//! elements it made or moved are, for the editor to keep them selected.

use std::collections::{BTreeSet, HashMap};

use crate::display::graphic_frame;
use crate::doc::*;
use crate::edit::on_stage;
use crate::hit::{self, ElementRef, Rect};
use crate::transform::transform;

impl Timeline {
    pub fn element(&self, r: ElementRef) -> Option<&Element> {
        self.layers.get(r.layer)?.frames.get(r.keyframe)?.elements.get(r.element)
    }

    pub fn element_mut(&mut self, r: ElementRef) -> Option<&mut Element> {
        self.layers.get_mut(r.layer)?.frames.get_mut(r.keyframe)?.elements.get_mut(r.element)
    }
}

/// `refs` from the bottom of the stage up: lower layers first, then each keyframe's own order.
fn stacked(refs: &[ElementRef]) -> Vec<ElementRef> {
    let mut refs = refs.to_vec();
    refs.sort_by_key(|r| (std::cmp::Reverse(r.layer), r.keyframe, r.element));
    refs.dedup();
    refs
}

/// Copy: the elements `refs`, bottom-most first.
pub fn copy(tl: &Timeline, refs: &[ElementRef]) -> Vec<Element> {
    stacked(refs).into_iter().filter_map(|r| tl.element(r).cloned()).collect()
}

/// Cut and Clear: removes the elements `refs` and returns them, bottom-most first.
pub fn take(tl: &mut Timeline, refs: &[ElementRef]) -> Vec<Element> {
    // Top-most first, so removing one doesn't shift the next.
    let mut taken: Vec<Element> = stacked(refs)
        .into_iter()
        .rev()
        .filter_map(|r| {
            let key = tl.layers.get_mut(r.layer)?.frames.get_mut(r.keyframe)?;
            (r.element < key.elements.len()).then(|| key.elements.remove(r.element))
        })
        .collect();
    taken.reverse();
    taken
}

/// Paste: `elements` on top of what layer `layer` shows at `frame`, in a new
/// blank keyframe there if the layer ends before it. Returns where they went.
pub fn paste(tl: &mut Timeline, layer: usize, frame: u32, elements: Vec<Element>) -> Vec<ElementRef> {
    let Some(l) = tl.layers.get_mut(layer) else { return Vec::new() };
    if l.keyframe_at(frame).is_none() {
        l.insert_blank_keyframe(frame);
    }
    let Some(keyframe) = l.keyframe_at(frame) else { return Vec::new() };
    let key = &mut l.frames[keyframe];
    let first = key.elements.len();
    key.elements.extend(elements);
    (first..key.elements.len()).map(|element| ElementRef { layer, keyframe, element }).collect()
}

/// Duplicate: a copy of each element `refs`, moved by `(dx, dy)`, on top of
/// its keyframe. Returns the copies.
pub fn duplicate(tl: &mut Timeline, refs: &[ElementRef], (dx, dy): Point) -> Vec<ElementRef> {
    let mut copies = Vec::new();
    for r in stacked(refs) {
        let Some(mut el) = tl.element(r).cloned() else { continue };
        hit::translate(&mut el, dx, dy);
        let key = &mut tl.layers[r.layer].frames[r.keyframe];
        copies.push(ElementRef { element: key.elements.len(), ..r });
        key.elements.push(el);
    }
    copies
}

/// Group: the elements `refs` become one group, bottom-most first, on the
/// topmost layer among them, where the lowest of them there was. Returns the group.
pub fn group(tl: &mut Timeline, refs: &[ElementRef]) -> Option<ElementRef> {
    let refs: Vec<_> = stacked(refs).into_iter().filter(|&r| tl.element(r).is_some()).collect();
    let top = refs.iter().map(|r| r.layer).min()?;
    let at = *refs.iter().find(|r| r.layer == top)?;
    let members = take(tl, &refs);
    tl.layers[at.layer].frames[at.keyframe].elements.insert(at.element, Element::Group(members));
    Some(at)
}

/// Break Apart (and Ungroup, for groups): what `el` comes apart into, `offset`
/// frames into its keyframe. A group gives its members. An instance gives what
/// its symbol shows then (a movie clip or button, its first frame), placed where
/// the instance shows it. None for the rest: shapes are as far apart as they go,
/// and bitmaps aren't modelled.
pub fn broken_apart(doc: &Document, el: &Element, offset: u32) -> Option<Vec<Element>> {
    let inst = match el {
        Element::Group(members) => return Some(members.clone()),
        Element::Instance(inst) => inst,
        // ponytail: Animate breaks text into a text per letter, then those into shapes. That
        // needs the glyph outlines as shapes (text.rs has them for drawing); until then it stays.
        Element::Text(_) | Element::Shape(_) | Element::Unsupported { .. } => return None,
    };
    let tl = &doc.library.get(&inst.symbol)?.timeline;
    let frame = if inst.kind == SymbolKind::Graphic { graphic_frame(inst, offset, tl) } else { 0 };
    // ponytail: the instance's color effect doesn't carry over to the parts; bake it into their
    // solid colors if that's wanted.
    let mut parts = Vec::new();
    for layer in tl.layers.iter().rev().filter(|l| l.kind == LayerKind::Normal) {
        if let Some(i) = layer.keyframe_at(frame) {
            let key = &layer.frames[i];
            parts.extend(on_stage(key, layer.frames.get(i + 1), frame - key.index));
        }
    }
    parts.iter_mut().for_each(|el| transform(el, &inst.matrix));
    Some(parts)
}

/// Puts the parts of each element named in `parts` in its place, and returns
/// where the elements of `selection` went: a replaced one's parts, in its place.
pub fn replace(tl: &mut Timeline, selection: &[ElementRef], mut parts: HashMap<ElementRef, Vec<Element>>) -> Vec<ElementRef> {
    let keys: BTreeSet<_> = selection.iter().chain(parts.keys()).map(|r| (r.layer, r.keyframe)).collect();
    let mut out = Vec::new();
    for (layer, keyframe) in keys {
        let Some(key) = tl.layers.get_mut(layer).and_then(|l| l.frames.get_mut(keyframe)) else { continue };
        for (element, el) in std::mem::take(&mut key.elements).into_iter().enumerate() {
            let r = ElementRef { layer, keyframe, element };
            let new = parts.remove(&r);
            let picked = new.is_some() || selection.contains(&r);
            for el in new.unwrap_or_else(|| vec![el]) {
                if picked {
                    out.push(ElementRef { element: key.elements.len(), ..r });
                }
                key.elements.push(el);
            }
        }
    }
    out
}

/// Modify > Arrange.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Arrange {
    BringToFront,
    BringForward,
    SendBackward,
    SendToBack,
}

/// Moves the elements `refs` up or down their keyframes' stacks, keeping
/// their order among themselves. Returns where they went.
pub fn arrange(tl: &mut Timeline, refs: &[ElementRef], how: Arrange) -> Vec<ElementRef> {
    let keys: BTreeSet<_> = refs.iter().map(|r| (r.layer, r.keyframe)).collect();
    let mut out = Vec::new();
    for (layer, keyframe) in keys {
        let Some(key) = tl.layers.get_mut(layer).and_then(|l| l.frames.get_mut(keyframe)) else { continue };
        let n = key.elements.len();
        let picked: Vec<bool> = (0..n).map(|element| refs.contains(&ElementRef { layer, keyframe, element })).collect();
        // Where each element comes from, by where it goes.
        let mut order: Vec<usize> = (0..n).collect();
        match how {
            Arrange::BringToFront => order.sort_by_key(|&i| picked[i]),
            Arrange::SendToBack => order.sort_by_key(|&i| !picked[i]),
            // Each one past its neighbour, if that one stays: top down (or bottom up), so a run
            // of picked elements moves as one.
            Arrange::BringForward => {
                for i in (1..n).rev() {
                    if picked[order[i - 1]] && !picked[order[i]] {
                        order.swap(i - 1, i);
                    }
                }
            }
            Arrange::SendBackward => {
                for i in 1..n {
                    if picked[order[i]] && !picked[order[i - 1]] {
                        order.swap(i - 1, i);
                    }
                }
            }
        }
        let mut old: Vec<Option<Element>> = std::mem::take(&mut key.elements).into_iter().map(Some).collect();
        key.elements = order.iter().filter_map(|&i| old[i].take()).collect();
        out.extend((0..n).filter(|&e| picked[order[e]]).map(|element| ElementRef { layer, keyframe, element }));
    }
    out
}

/// Modify > Align and the Align panel. `y` works down the stage instead of across.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Align {
    /// Lines up left edges (`at` 0), centers (0.5) or right edges (1).
    Edges { y: bool, at: f32 },
    /// Spreads those evenly between the outermost two.
    Distribute { y: bool, at: f32 },
    /// Makes the gaps between neighbours equal.
    Space { y: bool },
    /// Gives each the width, the height or both of the largest.
    Match { width: bool, height: bool },
}

/// How each of `boxes` moves or scales for `how`, as matrices in their
/// timeline's coordinates: lined up with each other, or with `stage` if given.
// ponytail: Distribute and Space keep between the outermost boxes even with a stage given, where
// Animate spreads them across the stage. Add that once it's been checked against Animate.
pub fn align(boxes: &[Rect], stage: Option<Rect>, how: Align) -> Vec<Matrix> {
    let Some(all) = boxes.iter().copied().reduce(Rect::union) else { return Vec::new() };
    let to = stage.unwrap_or(all);
    let span = |r: &Rect, y: bool| if y { (r.min.1, r.max.1) } else { (r.min.0, r.max.0) };
    let line = |r: &Rect, y: bool, at: f32| {
        let (lo, hi) = span(r, y);
        lo + (hi - lo) * at
    };
    let shift = |d: f32, y: bool| if y { Matrix::translate(0.0, d) } else { Matrix::translate(d, 0.0) };
    let by = |key: &dyn Fn(&Rect) -> f32| {
        let mut order: Vec<usize> = (0..boxes.len()).collect();
        order.sort_by(|&a, &b| key(&boxes[a]).total_cmp(&key(&boxes[b])));
        order
    };
    let gaps = (boxes.len() - 1).max(1) as f32;
    let mut out = vec![Matrix::IDENTITY; boxes.len()];
    match how {
        Align::Edges { y, at } => {
            for (m, r) in out.iter_mut().zip(boxes) {
                *m = shift(line(&to, y, at) - line(r, y, at), y);
            }
        }
        Align::Distribute { y, at } => {
            let order = by(&|r| line(r, y, at));
            let first = line(&boxes[order[0]], y, at);
            let step = (line(&boxes[order[order.len() - 1]], y, at) - first) / gaps;
            for (k, &i) in order.iter().enumerate() {
                out[i] = shift(first + step * k as f32 - line(&boxes[i], y, at), y);
            }
        }
        Align::Space { y } => {
            let size = |r: &Rect| span(r, y).1 - span(r, y).0;
            let (lo, hi) = span(&all, y);
            let gap = (hi - lo - boxes.iter().map(size).sum::<f32>()) / gaps;
            let mut next = lo;
            for i in by(&|r| span(r, y).0) {
                out[i] = shift(next - span(&boxes[i], y).0, y);
                next += size(&boxes[i]) + gap;
            }
        }
        Align::Match { width, height } => {
            let size = |r: &Rect| (r.max.0 - r.min.0, r.max.1 - r.min.1);
            let largest = boxes.iter().map(size).fold((0.0f32, 0.0f32), |a, b| (a.0.max(b.0), a.1.max(b.1)));
            let (w, h) = stage.map_or(largest, |s| size(&s));
            let factor = |on: bool, to: f32, from: f32| if on && from > 0.0 { to / from } else { 1.0 };
            for (m, r) in out.iter_mut().zip(boxes) {
                let (bw, bh) = size(r);
                *m = Matrix::scale(factor(width, w, bw), factor(height, h, bh)).about(r.min);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw;

    fn el(layer: usize, keyframe: usize, element: usize) -> ElementRef {
        ElementRef { layer, keyframe, element }
    }

    /// An element known by its name.
    fn mark(name: &str) -> Element {
        Element::Unsupported { name: name.into(), xml: String::new() }
    }

    fn names(elements: &[Element]) -> String {
        let name = |e: &Element| match e {
            Element::Unsupported { name, .. } => name.clone(),
            Element::Group(members) => format!("({})", names(members)),
            _ => "?".into(),
        };
        elements.iter().map(name).collect::<Vec<_>>().join(" ")
    }

    /// One frame long, with these layers (top first) of marked elements.
    fn timeline(layers: &[&[&str]]) -> Timeline {
        let mut tl = Timeline::new("Scene 1");
        tl.layers = layers
            .iter()
            .map(|marks| {
                let mut l = Timeline::new("").layers.remove(0);
                l.frames[0].elements = marks.iter().map(|m| mark(m)).collect();
                l
            })
            .collect();
        tl
    }

    fn layer(tl: &Timeline, l: usize) -> String {
        names(&tl.layers[l].frames[0].elements)
    }

    fn at(tl: &Timeline, refs: &[ElementRef]) -> String {
        names(&refs.iter().map(|&r| tl.element(r).unwrap().clone()).collect::<Vec<_>>())
    }

    #[test]
    fn copy_take_paste_and_duplicate() {
        let mut tl = timeline(&[&["a", "b"], &["c", "d"]]);
        // Bottom-most first, however they were picked.
        let picked = [el(0, 0, 1), el(1, 0, 0), el(0, 0, 0), el(1, 0, 0)];
        assert_eq!(names(&copy(&tl, &picked)), "c a b");
        assert_eq!(names(&take(&mut tl, &picked)), "c a b");
        assert_eq!((layer(&tl, 0), layer(&tl, 1)), (String::new(), "d".into()));

        // Pasting past the layer's end makes a blank keyframe there.
        let pasted = paste(&mut tl, 1, 3, vec![mark("x"), mark("y")]);
        assert_eq!(pasted, [el(1, 1, 0), el(1, 1, 1)]);
        assert_eq!((tl.layers[1].frames[1].index, tl.layers[1].frames[0].duration), (3, 3));
        assert_eq!(at(&tl, &pasted), "x y");
        assert!(paste(&mut tl, 9, 0, vec![mark("z")]).is_empty());

        // Copies go on top of their own keyframes, moved.
        let mut tl = Timeline::new("Scene 1");
        let square = draw::rectangle([(0.0, 0.0), (10.0, 10.0)], 0.0, None, Some(Paint::Solid([1.0; 4])));
        tl.layers[0].frames[0].elements = vec![Element::Shape(square), mark("b")];
        let copies = duplicate(&mut tl, &[el(0, 0, 0)], (10.0, 10.0));
        assert_eq!(copies, [el(0, 0, 2)]);
        let Some(Element::Shape(s)) = tl.element(copies[0]) else { panic!("not a shape") };
        assert_eq!((s.matrix.tx, s.matrix.ty), (10.0, 10.0));
    }

    #[test]
    fn group_then_ungroup() {
        let mut tl = timeline(&[&["a", "b", "c"], &["d", "e"]]);
        // c and e go into a group on the top layer, where c was, e below c.
        let g = group(&mut tl, &[el(0, 0, 2), el(1, 0, 1)]).unwrap();
        assert_eq!(g, el(0, 0, 2));
        assert_eq!((layer(&tl, 0), layer(&tl, 1)), ("a b (e c)".into(), "d".into()));
        let g = group(&mut tl, &[g, el(0, 0, 0)]).unwrap();
        assert_eq!((g, layer(&tl, 0)), (el(0, 0, 0), "(a (e c)) b".into()));
        assert_eq!(group(&mut tl, &[]), None);

        // Ungrouping puts the members in the group's place, and keeps the rest of the selection.
        let parts = |tl: &Timeline, r| match tl.element(r) {
            Some(Element::Group(m)) => HashMap::from([(r, m.clone())]),
            _ => HashMap::new(),
        };
        let p = parts(&tl, g);
        let picked = replace(&mut tl, &[g, el(0, 0, 1)], p);
        assert_eq!((layer(&tl, 0), at(&tl, &picked)), ("a (e c) b".into(), "a (e c) b".into()));
        let p = parts(&tl, el(0, 0, 1));
        let picked = replace(&mut tl, &[el(0, 0, 1)], p);
        assert_eq!((layer(&tl, 0), at(&tl, &picked)), ("a e c b".into(), "e c".into()));
    }

    #[test]
    fn breaking_apart_an_instance() {
        let mut doc = Document::new(550.0, 400.0, 24.0);
        // "Walk": a 10 px square at x = 0 then x = 20, over a guide layer that never shows.
        let square = |x: f32| {
            let mut s = draw::rectangle([(0.0, 0.0), (10.0, 10.0)], 0.0, None, Some(Paint::Solid([1.0; 4])));
            s.matrix = Matrix::translate(x, 0.0);
            Element::Shape(s)
        };
        let mut walk = Timeline::new("Walk");
        walk.layers[0].frames[0].elements = vec![square(0.0)];
        walk.layers[0].insert_keyframe(1);
        walk.layers[0].frames[1].elements = vec![square(20.0)];
        let mut guide = Timeline::new("").layers.remove(0);
        guide.kind = LayerKind::Guide;
        guide.frames[0].elements = vec![mark("guide")];
        walk.layers.push(guide);
        doc.library.insert("Walk".into(), Symbol { kind: SymbolKind::Graphic, timeline: walk, extra: Extra::default() });

        let instance = |kind| {
            Element::Instance(Instance {
                symbol: "Walk".into(),
                kind,
                matrix: Matrix::translate(100.0, 50.0) * Matrix::scale(2.0, 2.0),
                pivot: (5.0, 5.0),
                color: ColorTransform::IDENTITY,
                looping: Loop::Loop,
                first_frame: 0,
                extra: Extra::default(),
            })
        };
        let x_of = |parts: &[Element]| match parts {
            [Element::Shape(s)] => (s.matrix.tx, s.matrix.a),
            _ => panic!("one shape expected: {parts:?}"),
        };
        // A graphic shows the frame it has reached; a movie clip its first.
        assert_eq!(x_of(&broken_apart(&doc, &instance(SymbolKind::Graphic), 1).unwrap()), (140.0, 2.0));
        assert_eq!(x_of(&broken_apart(&doc, &instance(SymbolKind::Graphic), 2).unwrap()), (100.0, 2.0), "it loops");
        assert_eq!(x_of(&broken_apart(&doc, &instance(SymbolKind::MovieClip), 1).unwrap()), (100.0, 2.0));
        assert_eq!(broken_apart(&doc, &Element::Group(vec![mark("a")]), 0), Some(vec![mark("a")]));
        assert_eq!(broken_apart(&doc, &square(0.0), 0), None);
    }

    #[test]
    fn arrange_keeps_the_picked_in_order() {
        let tl = timeline(&[&["a", "b", "c", "d"], &["e", "f"]]);
        let run = |picked: &[usize], how| {
            let mut tl = tl.clone();
            let refs: Vec<_> = picked.iter().map(|&e| el(0, 0, e)).collect();
            let moved = arrange(&mut tl, &refs, how);
            // The same elements stay picked, at their new places.
            assert_eq!(at(&tl, &moved), names(&copy(&timeline(&[&["a", "b", "c", "d"]]), &refs)), "{how:?}");
            assert_eq!(layer(&tl, 1), "e f", "other layers stay");
            layer(&tl, 0)
        };
        use Arrange::*;
        assert_eq!(run(&[0, 2], BringToFront), "b d a c");
        assert_eq!(run(&[0, 2], BringForward), "b a d c");
        assert_eq!(run(&[0, 1], BringForward), "c a b d", "a run moves as one");
        assert_eq!(run(&[3], BringForward), "a b c d", "already on top");
        assert_eq!(run(&[1, 3], SendBackward), "b a d c");
        assert_eq!(run(&[2, 3], SendBackward), "a c d b");
        assert_eq!(run(&[1, 3], SendToBack), "b d a c");
    }

    fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect { min: (x0, y0), max: (x1, y1) }
    }

    /// The boxes after `align`.
    fn aligned(boxes: &[Rect], stage: Option<Rect>, how: Align) -> Vec<Rect> {
        let moves = align(boxes, stage, how);
        boxes.iter().zip(moves).map(|(r, m)| Rect { min: m.apply(r.min), max: m.apply(r.max) }).collect()
    }

    #[test]
    fn align_distribute_space_and_match() {
        let boxes = [rect(0.0, 0.0, 10.0, 10.0), rect(30.0, 5.0, 50.0, 15.0), rect(100.0, 40.0, 140.0, 80.0)];
        let lefts = |rs: Vec<Rect>| rs.iter().map(|r| r.min.0).collect::<Vec<_>>();
        let tops = |rs: Vec<Rect>| rs.iter().map(|r| r.min.1).collect::<Vec<_>>();
        assert_eq!(lefts(aligned(&boxes, None, Align::Edges { y: false, at: 0.0 })), [0.0; 3]);
        assert_eq!(lefts(aligned(&boxes, None, Align::Edges { y: false, at: 1.0 })), [130.0, 120.0, 100.0]);
        assert_eq!(tops(aligned(&boxes, None, Align::Edges { y: true, at: 0.5 })), [35.0, 35.0, 20.0]);
        let stage = Some(rect(0.0, 0.0, 550.0, 400.0));
        assert_eq!(tops(aligned(&boxes, stage, Align::Edges { y: true, at: 1.0 })), [390.0, 390.0, 360.0]);

        // Centers 5, 40, 120 spread to 5, 62.5, 120.
        let centers = aligned(&boxes, None, Align::Distribute { y: false, at: 0.5 });
        assert_eq!(centers.iter().map(|r| (r.min.0 + r.max.0) / 2.0).collect::<Vec<_>>(), [5.0, 62.5, 120.0]);
        // 140 across holds 70 of boxes, so the two gaps are 35 each.
        assert_eq!(lefts(aligned(&boxes, None, Align::Space { y: false })), [0.0, 45.0, 100.0]);

        let matched = aligned(&boxes, None, Align::Match { width: true, height: false });
        assert!(matched.iter().all(|r| r.max.0 - r.min.0 == 40.0), "{matched:?}");
        assert_eq!(matched[1].min, boxes[1].min, "they grow from their top-left corners");
        assert_eq!(matched[1].max.1, boxes[1].max.1);
        let to_stage = aligned(&boxes[..1], stage, Align::Match { width: true, height: true });
        assert_eq!(to_stage[0], rect(0.0, 0.0, 550.0, 400.0));
        assert!(align(&[], None, Align::Space { y: true }).is_empty());
    }
}
