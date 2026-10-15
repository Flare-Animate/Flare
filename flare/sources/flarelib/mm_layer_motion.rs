//! How a layer's links move its objects (docs/animate-ui-interaction.md §2.5): a classic
//! tween on a guided layer runs along its motion guide's stroke, and a parented layer
//! follows its parent layer's object. display.rs places everything through here.

use std::f32::consts::{PI, TAU};

use crate::display::tween_at;
use crate::doc::*;
use crate::hit::{ElementRef, along, polyline};

/// How far up a chain of parented layers motion is followed; a file can loop one.
const RIG_DEPTH: u32 = 8;

/// What keyframe `k` of layer `l` shows `offset` frames in: the instance its classic tween
/// puts there (along the layer's motion guide, if it has one), or None to show the keyframe
/// as is; and what layer parenting adds, the parent's motion since the keyframe began.
pub(crate) fn motion(tl: &Timeline, l: usize, k: usize, offset: u32) -> (Option<Instance>, Matrix) {
    motion_at(tl, l, k, offset, 0)
}

/// Instance `r` of `tl` as it shows at `frame`: tweened, on its guide, after its parent.
pub fn shown_instance(tl: &Timeline, r: ElementRef, frame: u32) -> Option<Instance> {
    let key = tl.layers.get(r.layer)?.frames.get(r.keyframe)?;
    let Some(Element::Instance(inst)) = key.elements.get(r.element) else { return None };
    let (tweened, rig) = motion(tl, r.layer, r.keyframe, frame.checked_sub(key.index)?);
    let mut inst = tweened.unwrap_or_else(|| inst.clone());
    inst.matrix = rig * inst.matrix;
    Some(inst)
}

/// After F6, which copies what a keyframe holds: each keyframe split off a guided tween or a
/// parented layer holds what showed there instead (at its own start parenting adds nothing),
/// so nothing jumps. `before` is the timeline before the split; the keyframes starting where
/// none did are the new ones.
pub fn keep_shown(before: &Timeline, after: &mut Timeline) {
    for (l, layer) in after.layers.iter_mut().enumerate() {
        let Some(old) = before.layers.get(l) else { continue };
        for key in layer.frames.iter_mut().filter(|k| !old.frames.iter().any(|o| o.index == k.index)) {
            let Some(o) = old.keyframe_at(key.index) else { continue };
            let (tweened, rig) = motion(before, l, o, key.index - old.frames[o].index);
            if let (Some(shown), [Element::Instance(copy)]) = (&tweened, &mut key.elements[..]) {
                copy.matrix = rig * shown.matrix;
            } else if rig != Matrix::IDENTITY {
                key.elements.iter_mut().for_each(|el| crate::transform::transform(el, &rig));
            }
        }
    }
}

fn motion_at(tl: &Timeline, l: usize, k: usize, offset: u32, depth: u32) -> (Option<Instance>, Matrix) {
    let layer = &tl.layers[l];
    let key = &layer.frames[k];
    let tweened = match tl.guide_of(l) {
        Some(g) => guided(tl, g, layer, k, offset),
        None => tween_at(key, layer.frames.get(k + 1), offset),
    };
    (tweened, rig(tl, l, k, key.index + offset, depth))
}

/// Keyframe `k` of `layer`, steered by guide layer `g`: a classic tween's transformation
/// point runs along the guide's stroke, from the point on it nearest where the tween starts
/// to the one nearest where it ends, and the keyframe it arrives at sits on that point. With
/// Orient to path the object turns as the stroke does, its own turn at each keyframe kept.
// ponytail: Animate snaps an object onto the guide as it's dragged (the Snap setting); here it
// shows snapped, and the file keeps where it was dropped. Snap while dragging when that matters.
fn guided(tl: &Timeline, g: usize, layer: &Layer, k: usize, offset: u32) -> Option<Instance> {
    let key = &layer.frames[k];
    let single = |f: &Frame| match &f.elements[..] {
        [Element::Instance(i)] => Some(i.clone()),
        _ => None,
    };
    let pivot = |i: &Instance| i.matrix.apply(i.pivot);
    if let Some(mut inst) = tween_at(key, layer.frames.get(k + 1), offset) {
        let (Some(a), Some(b)) = (single(key), layer.frames.get(k + 1).and_then(single)) else { return Some(inst) };
        let Some(track) = Track::of(tl, g, key.index, pivot(&a)) else { return Some(inst) };
        // How far along, eased as `tween_at` eases.
        let Tween::Classic { ease, .. } = key.tween else { return Some(inst) };
        let t = offset as f32 / key.duration as f32;
        let t = t + ease * t * (1.0 - t);
        let (s0, s1) = (track.project(pivot(&a)), track.project(pivot(&b)));
        let s = s0 + (s1 - s0) * t;
        let (to, heading) = track.at(s);
        let turn = if key.extra.get("motionTweenOrientToPath") == Some("true") {
            heading - (track.at(s0).1 + (track.at(s1).1 - track.at(s0).1) * t)
        } else {
            0.0
        };
        place(&mut inst, to, turn);
        return Some(inst);
    }
    // The keyframe a guided tween arrives at.
    let prev = layer.frames.get(k.checked_sub(1)?)?;
    tween_at(prev, Some(key), prev.duration)?;
    let (a, mut b) = (single(prev)?, single(key)?);
    let track = Track::of(tl, g, prev.index, pivot(&a))?;
    let to = track.at(track.project(pivot(&b))).0;
    place(&mut b, to, 0.0);
    Some(b)
}

/// Turns `inst` by `turn` about its transformation point, then moves that point to `to`.
fn place(inst: &mut Instance, to: Point, turn: f32) {
    if turn != 0.0 {
        inst.matrix = Matrix::rotate(turn) * inst.matrix;
    }
    let at = inst.matrix.apply(inst.pivot);
    inst.matrix.tx += to.0 - at.0;
    inst.matrix.ty += to.1 - at.1;
}

/// What parenting adds to keyframe `k` of layer `l` at `frame`: the parent's object's motion
/// since the keyframe began, the keyframe holding where its objects were then. The whole
/// transform carries over, as Animate 22 does with Propagate Scale, Skew and Flip on.
// ponytail: Animate's position-and-rotation-only parenting (those toggles off) isn't modelled.
// The stage edits keyframes as stored, so on a parented layer past a keyframe's start (its
// parent moved since) a drag, a new drawing or a shape's selection marks are off by this
// matrix; map stage edits through its inverse when that matters.
fn rig(tl: &Timeline, l: usize, k: usize, frame: u32, depth: u32) -> Matrix {
    let Some(p) = tl.rig_parent(l, k).filter(|_| depth < RIG_DEPTH) else { return Matrix::IDENTITY };
    let start = tl.layers[l].frames[k].index;
    match (object(tl, p, frame, depth + 1), object(tl, p, start, depth + 1).and_then(|m| m.inverse())) {
        (Some(now), Some(then)) => now * then,
        _ => Matrix::IDENTITY,
    }
}

/// Where layer `p`'s top object shows at `frame`: its transform, tweened, guided and parented.
fn object(tl: &Timeline, p: usize, frame: u32, depth: u32) -> Option<Matrix> {
    let layer = tl.layers.get(p)?;
    let k = layer.keyframe_at(frame)?;
    let key = &layer.frames[k];
    let (tweened, rig) = motion_at(tl, p, k, frame - key.index, depth);
    let m = match tweened {
        Some(inst) => inst.matrix,
        None => top_matrix(key.elements.last()?)?,
    };
    Some(rig * m)
}

/// An element's transform; a group goes by its top member.
fn top_matrix(el: &Element) -> Option<Matrix> {
    match el {
        Element::Shape(Shape { matrix, .. }) | Element::Instance(Instance { matrix, .. }) | Element::Text(Text { matrix, .. }) => Some(*matrix),
        Element::Group(members) => top_matrix(members.last()?),
        Element::Unsupported { .. } => None,
    }
}

/// A motion guide's stroke as a polyline: how far along it each point is, and which way
/// each piece heads, unwrapped so a stroke that winds round keeps turning the same way.
struct Track {
    points: Vec<Point>,
    along: Vec<f32>,
    heading: Vec<f32>,
}

impl Track {
    /// The stroke guide layer `g` draws at `frame` that passes nearest `near`: its stroked
    /// edges, joined where they meet end to end.
    fn of(tl: &Timeline, g: usize, frame: u32, near: Point) -> Option<Track> {
        let layer = tl.layers.get(g)?;
        let key = &layer.frames[layer.keyframe_at(frame)?];
        let mut pieces = Vec::new();
        strokes(&key.elements, &mut pieces);
        let gap = |line: &Vec<Point>| line.windows(2).map(|w| along(near, w[0], w[1]).0).fold(f32::INFINITY, f32::min);
        Track::new(chain(pieces).into_iter().min_by(|a, b| gap(a).total_cmp(&gap(b)))?)
    }

    fn new(line: Vec<Point>) -> Option<Track> {
        let mut points: Vec<Point> = Vec::new();
        for p in line {
            if points.last().is_none_or(|q| (q.0 - p.0).hypot(q.1 - p.1) > 1e-3) {
                points.push(p);
            }
        }
        if points.len() < 2 {
            return None;
        }
        let (mut along, mut heading) = (vec![0.0], Vec::<f32>::new());
        for w in points.windows(2) {
            along.push(along[along.len() - 1] + (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1));
            let h = (w[1].1 - w[0].1).atan2(w[1].0 - w[0].0);
            heading.push(heading.last().map_or(h, |&p| p + (h - p + PI).rem_euclid(TAU) - PI));
        }
        Some(Track { points, along, heading })
    }

    /// How far along the track its point nearest `p` is.
    fn project(&self, p: Point) -> f32 {
        let mut best = (f32::INFINITY, 0.0);
        for (i, w) in self.points.windows(2).enumerate() {
            let (d, u) = along(p, w[0], w[1]);
            if d < best.0 {
                best = (d, self.along[i] + u * (self.along[i + 1] - self.along[i]));
            }
        }
        best.1
    }

    /// The point `s` along the track, and the heading there.
    fn at(&self, s: f32) -> (Point, f32) {
        let i = self.along.partition_point(|&x| x <= s).saturating_sub(1).min(self.points.len() - 2);
        let (a, b, len) = (self.points[i], self.points[i + 1], self.along[i + 1] - self.along[i]);
        let u = ((s - self.along[i]) / len).clamp(0.0, 1.0);
        ((a.0 + (b.0 - a.0) * u, a.1 + (b.1 - a.1) * u), self.heading[i])
    }
}

/// The stroked edges of `elements`, as polylines in their timeline's coordinates.
// ponytail: only shapes (and groups of them) guide; a stroke inside a symbol or a text doesn't,
// as Animate's guides are lines drawn on the guide layer.
fn strokes(elements: &[Element], out: &mut Vec<Vec<Point>>) {
    for el in elements {
        match el {
            Element::Shape(s) => {
                for p in s.edges.iter().filter(|e| e.stroke != 0).flat_map(|e| &e.paths) {
                    out.push(polyline(p).into_iter().map(|q| s.matrix.apply(q)).collect());
                }
            }
            Element::Group(members) => strokes(members, out),
            _ => {}
        }
    }
}

/// Joins polylines that meet end to end into longer ones, turning them round as needed.
fn chain(mut pieces: Vec<Vec<Point>>) -> Vec<Vec<Point>> {
    let meets = |a: Point, b: Point| (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01;
    let mut out = Vec::new();
    while let Some(mut line) = pieces.pop() {
        loop {
            let (first, last) = (line[0], line[line.len() - 1]);
            if let Some(i) = pieces.iter().position(|p| meets(p[0], last) || meets(p[p.len() - 1], last)) {
                let mut p = pieces.swap_remove(i);
                if !meets(p[0], last) {
                    p.reverse();
                }
                line.extend(p.into_iter().skip(1));
            } else if let Some(i) = pieces.iter().position(|p| meets(p[0], first) || meets(p[p.len() - 1], first)) {
                let mut p = pieces.swap_remove(i);
                if !meets(p[p.len() - 1], first) {
                    p.reverse();
                }
                p.extend(line.into_iter().skip(1));
                line = p;
            } else {
                break;
            }
        }
        out.push(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::display::display_list;

    fn instance(x: f32, y: f32, turn: f32) -> Element {
        Element::Instance(Instance {
            symbol: "Dot".into(),
            kind: SymbolKind::Graphic,
            matrix: Matrix::translate(x, y) * Matrix::rotate(turn),
            pivot: (0.0, 0.0),
            color: ColorTransform::IDENTITY,
            looping: Loop::Loop,
            first_frame: 0,
            extra: Extra::default(),
        })
    }

    fn keyframe(index: u32, duration: u32, tween: Tween, elements: Vec<Element>) -> Frame {
        Frame { index, duration, tween, elements, extra: Extra::default() }
    }

    /// A stroke through `points`, 1 px wide.
    fn line(points: &[Point]) -> Element {
        let black = Paint::Solid([0.0, 0.0, 0.0, 1.0]);
        let segs = points[1..].iter().map(|&p| Seg::Line(p)).collect();
        Element::Shape(Shape {
            matrix: Matrix::IDENTITY,
            fills: vec![],
            strokes: vec![Stroke { width: 1.0, paint: black, cap: Cap::Round, join: Join::Round }],
            edges: vec![Edge { fill0: 0, fill1: 0, stroke: 1, paths: vec![Path { start: points[0], segs }] }],
            extra: Extra::default(),
        })
    }

    /// A document whose scene holds `layers`, top first, and a graphic "Dot" (a 2 px square).
    fn doc(layers: Vec<Layer>) -> Document {
        let mut doc = Document::new(550.0, 400.0, 24.0);
        doc.scenes[0].layers = layers;
        let mut dot = Timeline::new("Dot");
        dot.layers[0].frames[0].elements = vec![line(&[(0.0, 0.0), (1.0, 0.0)])];
        doc.library.insert("Dot".into(), Symbol { kind: SymbolKind::Graphic, timeline: dot, extra: Extra::default() });
        doc
    }

    fn layer(name: &str, frames: Vec<Frame>) -> Layer {
        Layer { name: name.into(), kind: LayerKind::Normal, frames, extra: Extra::default() }
    }

    /// Where layer `l`'s first item sits at frame `f`: its origin, and how far its x axis is turned.
    fn at(doc: &Document, l: usize, f: u32) -> (f32, f32, f32) {
        let list = display_list(doc, &doc.scenes[0], f, &mut BTreeSet::new());
        let m = list.iter().find(|p| p.source.layer == l).expect("on stage").matrix;
        (m.tx, m.ty, m.b.atan2(m.a))
    }

    fn near(got: (f32, f32, f32), want: (f32, f32, f32)) -> bool {
        (got.0 - want.0).abs() < 0.05 && (got.1 - want.1).abs() < 0.05 && (got.2 - want.2).abs() < 1e-3
    }

    #[test]
    fn a_guided_tween_runs_along_the_guides_stroke() {
        // An L-shaped guide: right 100 px from (100, 100), then down 100 px. The tween runs 10
        // frames from near its start (off it by 3 px) to near its end; arc length 200.
        let classic = Tween::Classic { ease: 0.0, rotate: Rotate::Auto };
        let guide = Layer { kind: LayerKind::Guide, ..layer("Guide", vec![keyframe(0, 11, Tween::None, vec![line(&[(100.0, 100.0), (200.0, 100.0), (200.0, 200.0)])])]) };
        let ball = layer("Ball", vec![keyframe(0, 10, classic, vec![instance(100.0, 97.0, 0.0)]), keyframe(10, 1, Tween::None, vec![instance(204.0, 200.0, 0.0)])]);
        let mut doc = doc(vec![guide, ball]);
        doc.scenes[0].set_parent(1, Some(0));
        let ball = |doc: &Document, f| at(doc, 1, f);
        // Snapped at both ends, a quarter of the way at frame 2.5's neighbours, round the corner at 5.
        assert!(near(ball(&doc, 0), (100.0, 100.0, 0.0)), "{:?}", ball(&doc, 0));
        assert!(near(ball(&doc, 2), (140.0, 100.0, 0.0)), "{:?}", ball(&doc, 2));
        assert!(near(ball(&doc, 5), (200.0, 100.0, 0.0)), "{:?}", ball(&doc, 5));
        assert!(near(ball(&doc, 8), (200.0, 160.0, 0.0)), "{:?}", ball(&doc, 8));
        assert!(near(ball(&doc, 10), (200.0, 200.0, 0.0)), "the keyframe arrived at sits on the guide too: {:?}", ball(&doc, 10));

        // Orient to path: it turns with the stroke, back to its own turn at each keyframe.
        doc.scenes[0].layers[1].frames[0].extra.set("motionTweenOrientToPath", Some("true"));
        let quarter = PI / 2.0;
        assert!(near(ball(&doc, 0), (100.0, 100.0, 0.0)), "{:?}", ball(&doc, 0));
        assert!(near(ball(&doc, 4), (180.0, 100.0, -0.4 * quarter)), "{:?}", ball(&doc, 4));
        assert!(near(ball(&doc, 6), (200.0, 120.0, 0.4 * quarter)), "{:?}", ball(&doc, 6));
        // Unguided, the same tween runs straight.
        doc.scenes[0].set_parent(1, None);
        assert!(near(ball(&doc, 5), (152.0, 148.5, 0.0)), "{:?}", ball(&doc, 5));
    }

    #[test]
    fn a_parented_layer_follows_its_parents_object() {
        // The arm turns a quarter about (100, 100) and moves 50 px right over 10 frames; the
        // hand, drawn at (150, 100), follows from its keyframe on.
        let classic = Tween::Classic { ease: 0.0, rotate: Rotate::Auto };
        let arm = layer("Arm", vec![keyframe(0, 10, classic, vec![instance(100.0, 100.0, 0.0)]), keyframe(10, 1, Tween::None, vec![instance(150.0, 100.0, PI / 2.0)])]);
        let hand = layer("Hand", vec![keyframe(0, 11, Tween::None, vec![instance(150.0, 100.0, 0.0)])]);
        let mut doc = doc(vec![arm, hand]);
        assert!(doc.scenes[0].set_rig_parent(1, Some(0)));
        let hand = |doc: &Document, f| at(doc, 1, f);
        assert!(near(hand(&doc, 0), (150.0, 100.0, 0.0)), "{:?}", hand(&doc, 0));
        // Halfway, the arm sits at (125, 100) turned an eighth: the hand 50 px out along it.
        let eighth = PI / 4.0;
        assert!(near(hand(&doc, 5), (125.0 + 50.0 * eighth.cos(), 100.0 + 50.0 * eighth.sin(), eighth)), "{:?}", hand(&doc, 5));
        assert!(near(hand(&doc, 10), (150.0, 150.0, PI / 2.0)), "{:?}", hand(&doc, 10));

        // A keyframe holds where its objects were when it began: from frame 5 on, the hand
        // drawn at (200, 100) moves with the arm's motion since frame 5.
        let hand_layer = &mut doc.scenes[0].layers[1];
        hand_layer.insert_blank_keyframe(5);
        hand_layer.frames[1].elements = vec![instance(200.0, 100.0, 0.0)];
        assert!(near(hand(&doc, 5), (200.0, 100.0, 0.0)), "{:?}", hand(&doc, 5));
        let (now, then) = (Matrix::translate(150.0, 100.0) * Matrix::rotate(PI / 2.0), Matrix::translate(125.0, 100.0) * Matrix::rotate(eighth));
        let want = now * then.inverse().unwrap() * Matrix::translate(200.0, 100.0);
        assert!(near(hand(&doc, 10), (want.tx, want.ty, PI / 2.0 - eighth)), "{:?} vs {want:?}", hand(&doc, 10));

        // Removing the parent: the hand stays where its keyframes put it.
        assert!(doc.scenes[0].set_rig_parent(1, None));
        assert!(near(hand(&doc, 10), (200.0, 100.0, 0.0)), "{:?}", hand(&doc, 10));
    }

    #[test]
    fn f6_keeps_what_shows_on_guided_and_parented_layers() {
        // The guided tween from the first test, with a hand parented to the ball.
        let classic = Tween::Classic { ease: 0.0, rotate: Rotate::Auto };
        let guide = Layer { kind: LayerKind::Guide, ..layer("Guide", vec![keyframe(0, 11, Tween::None, vec![line(&[(100.0, 100.0), (200.0, 100.0), (200.0, 200.0)])])]) };
        let ball = layer("Ball", vec![keyframe(0, 10, classic, vec![instance(100.0, 97.0, 0.0)]), keyframe(10, 1, Tween::None, vec![instance(204.0, 200.0, 0.0)])]);
        let hand = layer("Hand", vec![keyframe(0, 11, Tween::None, vec![instance(150.0, 100.0, 0.0)])]);
        let mut doc = doc(vec![guide, ball, hand]);
        doc.scenes[0].set_parent(1, Some(0));
        assert!(doc.scenes[0].set_rig_parent(2, Some(1)));
        let shown = |doc: &Document| (0..11).map(|f| (at(doc, 1, f), at(doc, 2, f))).collect::<Vec<_>>();
        let (was, before) = (shown(&doc), doc.scenes[0].clone());
        doc.scenes[0].layers[1].insert_keyframe(3);
        doc.scenes[0].layers[2].insert_keyframe(5);
        keep_shown(&before, &mut doc.scenes[0]);
        for (f, (was, now)) in was.iter().zip(shown(&doc)).enumerate() {
            assert!(near(was.0, now.0) && near(was.1, now.1), "frame {f}: {was:?} became {now:?}");
        }
    }

    #[test]
    fn strokes_chain_end_to_end() {
        let pieces = vec![vec![(10.0, 0.0), (20.0, 0.0)], vec![(0.0, 0.0), (10.0, 0.0)], vec![(30.0, 5.0), (20.0, 0.0)], vec![(50.0, 50.0), (60.0, 50.0)]];
        let mut lines = chain(pieces);
        lines.sort_by_key(|l| l.len());
        assert_eq!(lines, [vec![(50.0, 50.0), (60.0, 50.0)], vec![(30.0, 5.0), (20.0, 0.0), (10.0, 0.0), (0.0, 0.0)]]);
        let track = Track::new(vec![(0.0, 0.0), (10.0, 0.0), (20.0, 0.0), (30.0, 5.0)]).unwrap();
        assert_eq!(track.project((15.0, 3.0)), 15.0);
        assert_eq!(track.at(5.0), ((5.0, 0.0), 0.0));
    }
}
