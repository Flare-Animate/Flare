//! Free Transform tool (Q), and Modify > Transform. As in Animate, a box with
//! eight handles surrounds the selection: drag a handle to scale, just outside
//! a corner to rotate, a side between handles to skew, inside to move, and the
//! white transformation point to move what it all turns about. Double-click the
//! point to put it back in the middle.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use eframe::egui::{self, Color32, CursorIcon, Painter, Pos2, Rect, Response, Stroke, Ui, emath::GuiRounding, pos2, vec2};
use minnamate_core::doc::{Element, Instance, Matrix, Point};
use minnamate_core::hit::{self, ElementRef};
use minnamate_core::transform::transform;

use crate::app::{Action, Editor};
use crate::cursors::{self, Cursor};
use crate::stage::{local_point, screen_point};

/// The box's corners and side middles, in its unit square.
const CORNERS: [Point; 4] = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
const SIDES: [Point; 4] = [(0.5, 0.0), (1.0, 0.5), (0.5, 1.0), (0.0, 0.5)];
/// A handle's side, in screen points.
const HANDLE: f32 = 7.0;
/// How near a handle or the transformation point grabs it, in screen points.
const GRAB: f32 = 5.0;
/// How near a side skews.
const SIDE: f32 = 4.0;
/// How far outside a corner still rotates.
const TURN: f32 = 18.0;

/// What the tool remembers between frames.
#[derive(Default)]
pub struct State {
    /// The box and transformation point the last transform left, while the
    /// selected elements are only moved.
    kept: Option<Kept>,
    /// The drag in progress.
    drag: Option<Gesture>,
}

struct Kept {
    /// The selected elements then.
    elements: Vec<(ElementRef, Element)>,
    /// Their bounds then.
    bounds: hit::Rect,
    target: Target,
}

/// The box around the selection and its transformation point, in the edited timeline's coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Target {
    /// Maps the unit square onto the box: (0, 0) to its top-left corner and
    /// (1, 1) to the bottom-right one, as they were before any turn or flip.
    bx: Matrix,
    pivot: Point,
}

struct Gesture {
    zone: Zone,
    /// Where the drag began, in the edited timeline's coordinates.
    start: Point,
    from: Target,
    /// The selected elements before the drag.
    originals: Vec<(ElementRef, Element)>,
    /// The transform so far.
    m: Matrix,
    /// `Editor::rev` once the drag last wrote: anything else changing the document ends the drag.
    rev: u32,
}

/// Where the pointer is on the box, by unit-square positions.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Zone {
    Pivot,
    /// A handle: corners scale both ways, side middles one.
    Scale(Point),
    /// Just outside a corner.
    Rotate(Point),
    /// A side between handles, by its middle.
    Skew(Point),
    Move,
}

/// Pointer input on the stage while this tool is active. `stage` is the
/// stage's screen rectangle.
pub fn input(ui: &Ui, ed: &mut Editor, resp: &Response, stage: Rect) {
    let (shift, alt) = ui.input(|i| (i.modifiers.shift, i.modifiers.alt));
    let screen = screen(ed, stage);
    // How near the transformation point snaps, in the edited timeline's units.
    let reach = GRAB / (screen.a * screen.d - screen.b * screen.c).abs().sqrt();
    if let Some(mut g) = ed.transform.drag.take() {
        // A drag lasts while nothing else changes the document or the selection under it (an
        // undo, Delete). One a tool switch left behind ends too, as it stood.
        let live = g.rev == ed.rev && g.originals.iter().map(|(r, _)| r).eq(&ed.selection);
        if live && resp.drag_stopped() {
            return finish(ed, g);
        }
        if live && resp.dragged() {
            if let Some(p) = resp.interact_pointer_pos() {
                let p = local_point(ed, stage, p);
                update(ed, &mut g, p, shift, alt, reach);
            }
            cursor(ui.ctx(), g.zone, &(screen * g.from.bx));
            ed.transform.drag = Some(g);
            return;
        }
    }
    // The Selection tool's own drags (a marquee, say) run to their end first.
    let t = if ed.drag.is_none() { target(ed) } else { None };
    let zone_at = |p: Pos2| t.and_then(|t| zone(&(screen * t.bx), screen.apply(t.pivot), p));
    if resp.hovered() {
        match t.zip(resp.hover_pos().and_then(zone_at)) {
            Some((t, zone)) => cursor(ui.ctx(), zone, &(screen * t.bx)),
            None => ui.ctx().set_cursor_icon(CursorIcon::Default),
        }
    }
    if resp.drag_started()
        && let Some(origin) = ui.input(|i| i.pointer.press_origin())
        && let (Some(from), Some(zone)) = (t, zone_at(origin))
    {
        let start = local_point(ed, stage, origin);
        let mut g = Gesture { zone, start, from, originals: selected(ed), m: Matrix::IDENTITY, rev: ed.rev };
        if let Some(p) = resp.interact_pointer_pos() {
            let p = local_point(ed, stage, p);
            update(ed, &mut g, p, shift, alt, reach);
        }
        ed.transform.drag = Some(g);
        return;
    }
    match (t, resp.clicked().then(|| resp.interact_pointer_pos().and_then(zone_at)).flatten()) {
        (Some(t), Some(Zone::Pivot)) if resp.double_clicked() => set_pivot(ed, Target { pivot: t.bx.apply((0.5, 0.5)), ..t }),
        // Clicks on the handles, sides and pivot do nothing; inside the box they select.
        (_, Some(zone)) if zone != Zone::Move => {}
        _ => crate::stage::tool_input(ui, ed, resp, stage),
    }
}

/// Feedback drawn over the stage (screen space).
pub fn overlay(p: &Painter, ed: &Editor, stage: Rect) {
    let Some(t) = ed.transform.drag.as_ref().map(Gesture::now).or_else(|| target(ed)) else { return };
    let ppp = p.ctx().pixels_per_point();
    let screen = screen(ed, stage);
    let bx = screen * t.bx;
    let at = |q: Point| {
        let (x, y) = bx.apply(q);
        pos2(x, y).round_to_pixel_center(ppp)
    };
    let ink = Stroke::new(1.0, Color32::BLACK);
    p.add(egui::Shape::closed_line(CORNERS.iter().map(|&c| at(c)).collect(), ink));
    // Black squares in a white rim, as Animate draws them, so they show on dark art too.
    for &h in CORNERS.iter().chain(&SIDES) {
        let square = Rect::from_center_size(at(h), vec2(HANDLE, HANDLE));
        p.rect_filled(square, 0.0, Color32::WHITE);
        p.rect_filled(square.shrink(1.0), 0.0, Color32::BLACK);
    }
    let (x, y) = screen.apply(t.pivot);
    crate::stage::transformation_point(p, pos2(x, y));
}

/// The tool's section of Properties > Tool.
pub fn options(ui: &mut Ui, ed: &mut Editor) {
    let _ = (ui, ed);
}

/// Modify > Transform: flips and quarter turns, about the transformation point.
pub fn modify(ed: &mut Editor, action: Action) {
    let m = match action {
        Action::FlipHorizontal => Matrix::scale(-1.0, 1.0),
        Action::FlipVertical => Matrix::scale(1.0, -1.0),
        Action::Rotate90Cw => Matrix::rotate(FRAC_PI_2),
        Action::Rotate90Ccw => Matrix::rotate(-FRAC_PI_2),
        _ => return,
    };
    about_pivot(ed, m, None);
}

/// Transforms the selection by `m` about its transformation point: one undo
/// step, or with `gesture` one per run of edits from that field.
pub fn about_pivot(ed: &mut Editor, m: Matrix, gesture: Option<&'static str>) {
    let Some(t) = target(ed) else { return };
    let m = m.about(t.pivot);
    match gesture {
        Some(field) => ed.checkpoint_gesture(field),
        None => ed.checkpoint(),
    }
    transform_selection(ed, &m);
    keep(ed, Target { bx: m * t.bx, pivot: t.pivot });
}

/// Properties > Object W and H: scales the selection, whose bounds are `b`,
/// to `w` × `h` about their top-left corner.
pub fn resize(ed: &mut Editor, b: hit::Rect, w: f32, h: f32) {
    let (bw, bh) = (b.max.0 - b.min.0, b.max.1 - b.min.1);
    if (w == bw && h == bh) || bw <= 0.0 || bh <= 0.0 {
        return;
    }
    ed.checkpoint_gesture("object-size");
    transform_selection(ed, &Matrix::scale(w / bw, h / bh).about(b.min));
}

/// Transforms every selected element by `m`. Callers checkpoint first.
fn transform_selection(ed: &mut Editor, m: &Matrix) {
    // A motion tween's target, inside its span, gets property keyframes instead.
    let frame = ed.frame;
    for r in ed.selection.clone() {
        minnamate_core::motion::edit_shown(ed.timeline_mut(), r, frame, |el| transform(el, m));
    }
}

/// The selection's box and transformation point, if any of it is on stage.
fn target(ed: &Editor) -> Option<Target> {
    let bounds = hit::bounds(&ed.doc, ed.timeline(), ed.frame, &ed.selection)?;
    // ponytail: a shape redrawn just as it was, in the same place, takes the old box and point;
    // tell elements apart by identity if that ever matters.
    let kept = ed.transform.kept.as_ref().filter(|k| same_size(k.bounds, bounds) && only_moved(ed, &k.elements));
    let mut t = match kept {
        // Moved since, by the arrow keys or the Selection tool: follow it.
        Some(k) => {
            let m = Matrix::translate(bounds.min.0 - k.bounds.min.0, bounds.min.1 - k.bounds.min.1);
            Target { bx: m * k.target.bx, pivot: m.apply(k.target.pivot) }
        }
        None => {
            let bx = own_box(ed).unwrap_or_else(|| rect(bounds));
            Target { bx, pivot: bx.apply((0.5, 0.5)) }
        }
    };
    if let Some(inst) = lone_instance(ed) {
        t.pivot = inst.matrix.apply(inst.pivot);
    }
    Some(t)
}

fn same_size(a: hit::Rect, b: hit::Rect) -> bool {
    let size = |r: hit::Rect| (r.max.0 - r.min.0, r.max.1 - r.min.1);
    let ((aw, ah), (bw, bh)) = (size(a), size(b));
    (aw - bw).abs() < 0.01 && (ah - bh).abs() < 0.01
}

/// Whether the selection is still `then`, at most moved since (by the arrow keys, the
/// Selection tool or Properties X and Y). An undo, a new document or a new drawing isn't.
fn only_moved(ed: &Editor, then: &[(ElementRef, Element)]) -> bool {
    then.len() == ed.selection.len()
        && then.iter().zip(&ed.selection).all(|((r, el), now)| r == now && element(ed, *r).is_some_and(|now| moved(el, now)))
}

/// Whether `now` is `then`, put wherever it is now.
fn moved(then: &Element, now: &Element) -> bool {
    let turn = |m: &Matrix| [m.a, m.b, m.c, m.d];
    match (then, now) {
        (Element::Shape(a), Element::Shape(b)) => turn(&a.matrix) == turn(&b.matrix) && a.edges == b.edges,
        (Element::Instance(a), Element::Instance(b)) => turn(&a.matrix) == turn(&b.matrix) && a.symbol == b.symbol,
        (Element::Group(a), Element::Group(b)) => a.len() == b.len() && a.iter().zip(b).all(|(a, b)| moved(a, b)),
        _ => then == now,
    }
}

/// The selected elements, with where they are.
fn selected(ed: &Editor) -> Vec<(ElementRef, Element)> {
    ed.selection.iter().filter_map(|&r| Some((r, element(ed, r)?.clone()))).collect()
}

/// A lone shape or instance's box lies along its own axes, so it turns and
/// skews with it.
fn own_box(ed: &Editor) -> Option<Matrix> {
    let (r, el) = lone(ed)?;
    let m = match el {
        Element::Shape(s) => s.matrix,
        Element::Instance(_) => lone_instance(ed)?.matrix,
        _ => return None,
    };
    let local = hit::bounds_through(&ed.doc, ed.timeline(), ed.frame, &[r], m.inverse()?)?;
    Some(m * rect(local))
}

/// The box `r` as the image of the unit square.
fn rect(r: hit::Rect) -> Matrix {
    Matrix { a: r.max.0 - r.min.0, b: 0.0, c: 0.0, d: r.max.1 - r.min.1, tx: r.min.0, ty: r.min.1 }
}

fn element(ed: &Editor, r: ElementRef) -> Option<&Element> {
    ed.timeline().layers.get(r.layer)?.frames.get(r.keyframe)?.elements.get(r.element)
}

/// The only selected element.
fn lone(ed: &Editor) -> Option<(ElementRef, &Element)> {
    let [r] = ed.selection[..] else { return None };
    Some((r, element(ed, r)?))
}

/// An instance keeps its own transformation point, as in Animate; a motion tween's
/// target is where it shows.
fn lone_instance(ed: &Editor) -> Option<Instance> {
    match lone(ed)? {
        (r, Element::Instance(i)) => match crate::tweens::shown_target(ed, r) {
            Some(Element::Instance(shown)) => Some(shown),
            _ => Some(i.clone()),
        },
        _ => None,
    }
}

/// The edited timeline's coordinates to screen points, as a matrix: what
/// `screen_point` does, read off at three points (far apart, for precision).
pub(crate) fn screen(ed: &Editor, stage: Rect) -> Matrix {
    const STEP: f32 = 1024.0;
    let at = |p| screen_point(ed, stage, p);
    let (o, x, y) = (at((0.0, 0.0)), at((STEP, 0.0)), at((0.0, STEP)));
    Matrix { a: (x.x - o.x) / STEP, b: (x.y - o.y) / STEP, c: (y.x - o.x) / STEP, d: (y.y - o.y) / STEP, tx: o.x, ty: o.y }
}

/// What the pointer at `p` is over, with the box `bx` and transformation point `pivot` on screen.
fn zone(bx: &Matrix, (px, py): Point, p: Pos2) -> Option<Zone> {
    let at = |q: Point| {
        let (x, y) = bx.apply(q);
        pos2(x, y)
    };
    let grabs = |q: Pos2| (q.x - p.x).abs() <= GRAB && (q.y - p.y).abs() <= GRAB;
    if grabs(pos2(px, py)) {
        return Some(Zone::Pivot);
    }
    if let Some(&h) = CORNERS.iter().chain(&SIDES).find(|&&h| grabs(at(h))) {
        return Some(Zone::Scale(h));
    }
    // Each side runs between the corners either side of its middle.
    let ends = |(u, v): Point| if u == 0.5 { [(0.0, v), (1.0, v)] } else { [(u, 0.0), (u, 1.0)] };
    if let Some(&h) = SIDES.iter().find(|&&h| {
        let [a, b] = ends(h);
        to_segment(p, at(a), at(b)) <= SIDE
    }) {
        return Some(Zone::Skew(h));
    }
    let inside = bx.inverse().map(|inv| inv.apply((p.x, p.y)));
    if inside.is_some_and(|(u, v)| (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v)) {
        return Some(Zone::Move);
    }
    CORNERS.iter().find(|&&c| at(c).distance(p) <= TURN).map(|&c| Zone::Rotate(c))
}

/// From `p` to the segment `a`-`b`.
fn to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = if ab.length_sq() > 0.0 { ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
    p.distance(a + t * ab)
}

impl Gesture {
    /// The box and transformation point as the drag has them now.
    fn now(&self) -> Target {
        let bx = if self.zone == Zone::Pivot { self.from.bx } else { self.m * self.from.bx };
        Target { bx, pivot: self.m.apply(self.from.pivot) }
    }

    /// The transform a drag to `p` makes. The transformation point snaps to
    /// the handles and the middle within `reach`.
    fn matrix(&self, p: Point, shift: bool, alt: bool, reach: f32) -> Matrix {
        let (t, start) = (&self.from, self.start);
        let moved = Matrix::translate(p.0 - start.0, p.1 - start.1);
        match self.zone {
            Zone::Move => moved,
            Zone::Pivot => {
                let to = moved.apply(t.pivot);
                let near = |q: &Point| (q.0 - to.0).abs() <= reach && (q.1 - to.1).abs() <= reach;
                match CORNERS.iter().chain(&SIDES).chain(&[(0.5, 0.5)]).map(|&q| t.bx.apply(q)).find(near) {
                    Some(q) => Matrix::translate(q.0 - t.pivot.0, q.1 - t.pivot.1),
                    None => moved,
                }
            }
            Zone::Rotate(corner) => {
                // About the transformation point, or with Alt the opposite corner.
                let c = if alt { t.bx.apply((1.0 - corner.0, 1.0 - corner.1)) } else { t.pivot };
                let angle = |q: Point| (q.1 - c.1).atan2(q.0 - c.0);
                let mut turn = angle(p) - angle(start);
                if shift {
                    turn = (turn / FRAC_PI_4).round() * FRAC_PI_4;
                }
                Matrix::rotate(turn).about(c)
            }
            Zone::Scale(h) | Zone::Skew(h) => {
                // Worked out in the box's unit square, where the handle at `h` follows the pointer.
                let Some(inv) = t.bx.inverse() else { return Matrix::IDENTITY };
                let (q0, q1) = (inv.apply(start), inv.apply(p));
                let d = (q1.0 - q0.0, q1.1 - q0.1);
                // What stays put: the opposite handle, or with Alt the transformation point.
                let fixed = if alt { inv.apply(t.pivot) } else { (1.0 - h.0, 1.0 - h.1) };
                let unit = if matches!(self.zone, Zone::Scale(_)) { scale(h, fixed, d, shift) } else { skew(h, fixed, d) };
                t.bx * unit.about(fixed) * inv
            }
        }
    }
}

/// Moving the handle at `h` by `d` scales about `fixed`. Shift keeps proportions.
fn scale(h: Point, fixed: Point, d: Point, shift: bool) -> Matrix {
    // On each axis the handle moves along: its new distance from `fixed` over its old one.
    let factor = |h: f32, fixed: f32, d: f32| (h != 0.5 && (h - fixed).abs() > 1e-6).then(|| (h + d - fixed) / (h - fixed));
    let (fx, fy) = (factor(h.0, fixed.0, d.0), factor(h.1, fixed.1, d.1));
    let (mut sx, mut sy) = (fx.unwrap_or(1.0), fy.unwrap_or(1.0));
    if shift {
        // Both axes take the bigger change, each keeping its own flip.
        let s = fx.into_iter().chain(fy).map(f32::abs).max_by(|a, b| (a - 1.0).abs().total_cmp(&(b - 1.0).abs()));
        let s = s.unwrap_or(1.0);
        (sx, sy) = (s.copysign(sx), s.copysign(sy));
    }
    // Never quite flat, so the matrix stays invertible.
    let floor = |s: f32| if s.abs() < 1e-3 { 1e-3f32.copysign(s) } else { s };
    Matrix::scale(floor(sx), floor(sy))
}

/// Sliding the side whose middle is `h` along itself by `d` shears about `fixed`.
fn skew(h: Point, fixed: Point, d: Point) -> Matrix {
    let shear = |along: f32, out: f32, fixed: f32| if (out - fixed).abs() > 1e-6 { along / (out - fixed) } else { 0.0 };
    if h.0 == 0.5 {
        // The top or bottom side slides sideways.
        Matrix::skew(shear(d.0, h.1, fixed.1), 0.0)
    } else {
        Matrix::skew(0.0, shear(d.1, h.0, fixed.0))
    }
}

/// Drags the selection (or the transformation point) to `p`, live, as one undo step.
fn update(ed: &mut Editor, g: &mut Gesture, p: Point, shift: bool, alt: bool, reach: f32) {
    let m = g.matrix(p, shift, alt, reach);
    if m == g.m {
        return;
    }
    let step = g.m.inverse().map(|undo| m * undo);
    g.m = m;
    if g.zone == Zone::Pivot {
        return;
    }
    ed.checkpoint_gesture("free-transform");
    let frame = ed.frame;
    for (r, before) in &g.originals {
        // A motion tween's target is keyed by the step since the last update; the rest start over.
        if minnamate_core::motion::tweening(ed.timeline(), *r, frame) {
            if let Some(step) = step {
                minnamate_core::motion::edit_shown(ed.timeline_mut(), *r, frame, |el| transform(el, &step));
            }
        } else if let Some(el) = ed.element_mut(*r) {
            *el = before.clone();
            transform(el, &m);
        }
    }
    g.rev = ed.rev;
}

fn finish(ed: &mut Editor, g: Gesture) {
    ed.end_gesture();
    if g.m == Matrix::IDENTITY {
        return;
    }
    match g.zone {
        Zone::Pivot => set_pivot(ed, g.now()),
        _ => keep(ed, g.now()),
    }
}

/// Remembers `target` for the selection as it is now.
fn keep(ed: &mut Editor, target: Target) {
    let bounds = hit::bounds(&ed.doc, ed.timeline(), ed.frame, &ed.selection);
    ed.transform.kept = bounds.map(|bounds| Kept { elements: selected(ed), bounds, target });
}

/// Moves the transformation point to `t.pivot`. An instance stores it, in its
/// symbol's coordinates, so that is a document change.
fn set_pivot(ed: &mut Editor, t: Target) {
    if let [r] = ed.selection[..]
        && let Some(inst) = lone_instance(ed)
        && let Some(local) = inst.matrix.inverse().map(|inv| inv.apply(t.pivot))
        && local != inst.pivot
    {
        ed.checkpoint();
        if let Some(Element::Instance(inst)) = ed.element_mut(r) {
            inst.pivot = local;
        }
    }
    keep(ed, t);
}

/// The pointer over `zone` of the box `bx` (on screen): the system's arrows
/// to scale and move, our own to rotate and skew.
fn cursor(ctx: &egui::Context, zone: Zone, bx: &Matrix) {
    // Which way a handle lies from the middle, along the box's own axes.
    let axis = |x: f32, y: f32| {
        let len = x.hypot(y).max(1e-6);
        (x / len, y / len)
    };
    let (u, v) = (axis(bx.a, bx.b), axis(bx.c, bx.d));
    let heading = |(hu, hv): Point| {
        let (su, sv) = (hu - 0.5, hv - 0.5);
        (su * u.1 + sv * v.1).atan2(su * u.0 + sv * v.0)
    };
    match zone {
        Zone::Pivot => ctx.set_cursor_icon(CursorIcon::Default),
        Zone::Move => ctx.set_cursor_icon(CursorIcon::Move),
        Zone::Scale(h) => ctx.set_cursor_icon(match ((heading(h) / FRAC_PI_4).round() as i32).rem_euclid(4) {
            0 => CursorIcon::ResizeHorizontal,
            1 => CursorIcon::ResizeNwSe,
            2 => CursorIcon::ResizeVertical,
            _ => CursorIcon::ResizeNeSw,
        }),
        Zone::Rotate(_) => cursors::show(ctx, Cursor::Rotate),
        // A side slides along itself, across the way it faces.
        Zone::Skew(h) => {
            let faces_up_or_down = ((heading(h) / FRAC_PI_2).round() as i32).rem_euclid(2) == 1;
            cursors::show(ctx, if faces_up_or_down { Cursor::SkewX } else { Cursor::SkewY })
        }
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Key, Modifiers};

    use super::*;
    use crate::app::testkit::*;
    use crate::tools::Tool;

    /// Within a pixel: pointer positions round to the screen grid.
    fn near(a: Point, b: Point) -> bool {
        (a.0 - b.0).abs() < 1.0 && (a.1 - b.1).abs() < 1.0
    }

    fn close(a: Point, b: Point) -> bool {
        (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
    }

    fn middle(b: hit::Rect) -> Point {
        ((b.min.0 + b.max.0) / 2.0, (b.min.1 + b.max.1) / 2.0)
    }

    /// A new document with a rectangle drawn from (100, 100) to (200, 150),
    /// selected with the Free Transform tool.
    fn rectangle() -> Harness<'static, Editor> {
        let mut h = blank();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        h.state_mut().tool = Tool::FreeTransform;
        mouse_click(&mut h, (150.0, 125.0));
        assert_eq!(h.state().selection.len(), 1, "with nothing selected, a click selects");
        h
    }

    #[test]
    fn scale_by_a_corner_then_rotate_outside_one() {
        let mut h = rectangle();
        let b = shape_bounds(h.state());
        // The bottom-right handle, dragged by (100, 50), scales from the top-left corner.
        mouse_drag(&mut h, b.max, (b.max.0 + 100.0, b.max.1 + 50.0));
        let s = shape_bounds(h.state());
        assert!(near(s.min, b.min) && near(s.max, (b.max.0 + 100.0, b.max.1 + 50.0)), "{s:?}");

        // From just outside the top-right corner, a quarter turn clockwise about the middle.
        let c = middle(s);
        let out = 8.0 / h.state().zoom;
        let from = (s.max.0 + out, s.min.1 - out);
        let to = (c.0 - (from.1 - c.1), c.1 + (from.0 - c.0));
        mouse_drag(&mut h, from, to);
        let r = shape_bounds(h.state());
        let half = ((s.max.1 - s.min.1) / 2.0, (s.max.0 - s.min.0) / 2.0);
        assert!(near(r.min, (c.0 - half.0, c.1 - half.1)) && near(r.max, (c.0 + half.0, c.1 + half.1)), "{r:?}");
        let Some(Element::Shape(shape)) = element(h.state(), h.state().selection[0]) else { panic!("not a shape") };
        assert!(shape.matrix.b > 0.99 && shape.matrix.c < -0.99, "turned by its matrix: {:?}", shape.matrix);

        // Each drag is one undo step.
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.step();
        assert!(near(shape_bounds(h.state()).max, s.max));
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.step();
        assert!(near(shape_bounds(h.state()).max, b.max));
    }

    #[test]
    fn the_transformation_point_snaps_and_menus_turn_about_it() {
        let mut h = rectangle();
        let ctx = h.ctx.clone();
        let b = shape_bounds(h.state());
        let (w, ht) = (b.max.0 - b.min.0, b.max.1 - b.min.1);
        // Dropped near the top-left handle, the point snaps onto it.
        let px = 1.0 / h.state().zoom;
        mouse_drag(&mut h, middle(b), (b.min.0 + 2.0 * px, b.min.1 + px));
        assert_eq!(target(h.state()).map(|t| t.pivot), Some(b.min));
        h.state_mut().run(Action::Rotate90Cw, &ctx);
        let r = shape_bounds(h.state());
        assert!(close(r.min, (b.min.0 - ht, b.min.1)) && close(r.max, (b.min.0, b.min.1 + w)), "{r:?}");
        // The point stays on that corner, and the flip mirrors about it.
        h.state_mut().run(Action::FlipVertical, &ctx);
        let f = shape_bounds(h.state());
        assert!(close(f.min, (b.min.0 - ht, b.min.1 - w)) && close(f.max, b.min), "{f:?}");
        h.state_mut().run(Action::Undo, &ctx);
        h.state_mut().run(Action::Undo, &ctx);
        assert!(close(shape_bounds(h.state()).max, b.max));

        // Properties > Object: W and H scale about the top-left corner.
        let ed = h.state_mut();
        resize(ed, b, 2.0 * w, ht);
        let s = shape_bounds(ed);
        assert!(close(s.min, b.min) && close(s.max, (b.min.0 + 2.0 * w, b.max.1)), "{s:?}");
    }

    #[test]
    fn inside_a_symbol_edited_in_place() {
        let mut h = rectangle();
        let ctx = h.ctx.clone();
        h.key_press(Key::F8);
        h.step();
        h.step();
        h.key_press(Key::Enter);
        h.step();
        // Double the instance by its bottom-right handle, then edit the symbol in place:
        // it shows twice as big as it is.
        let b = shape_bounds(h.state());
        mouse_drag(&mut h, b.max, (2.0 * b.max.0 - b.min.0, 2.0 * b.max.1 - b.min.1));
        h.state_mut().run(Action::EditInPlace, &ctx);
        assert_eq!(h.state().editing.len(), 1);

        // Inside, the rectangle's box and handles are where it shows: dragging its
        // bottom-right handle 40 × 20 on stage scales it by 20 × 10 of its own units.
        mouse_click(&mut h, (b.min.0 + 20.0, b.min.1 + 20.0));
        assert_eq!(h.state().selection.len(), 1);
        let local = shape_bounds(h.state());
        let corner = h.state().to_scene(local.max);
        mouse_drag(&mut h, corner, (corner.0 + 40.0, corner.1 + 20.0));
        let s = shape_bounds(h.state());
        assert!(near(s.min, local.min) && near(s.max, (local.max.0 + 20.0, local.max.1 + 10.0)), "{local:?} became {s:?}");
    }

    /// The selection's bounds.
    fn picked(ed: &Editor) -> hit::Rect {
        hit::bounds(&ed.doc, ed.timeline(), ed.frame, &ed.selection).expect("a selection on stage")
    }

    /// Presses at `from` and moves to `to`, the button still down.
    fn press_and_move(h: &mut Harness<'_, Editor>, from: Point, to: Point) {
        let (a, b) = (on_stage(h, from), on_stage(h, to));
        h.hover_at(a);
        h.step();
        h.drag_at(a);
        h.step();
        h.hover_at(b);
        h.step();
    }

    /// Moves the held pointer to `to`, then lets go there.
    fn move_and_release(h: &mut Harness<'_, Editor>, to: Point) {
        let p = on_stage(h, to);
        h.hover_at(p);
        h.step();
        h.drop_at(p);
        h.step();
    }

    /// `rectangle()`, plus a second one the same size at (300, 100), drawn after it.
    fn two_rectangles() -> Harness<'static, Editor> {
        let mut h = rectangle();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (300.0, 100.0), (400.0, 150.0));
        h.state_mut().tool = Tool::FreeTransform;
        mouse_click(&mut h, (150.0, 125.0));
        h
    }

    #[test]
    fn a_drag_cut_short_by_a_tool_switch_is_dropped() {
        let mut h = two_rectangles();
        let a = picked(h.state());
        // Halfway through scaling the first rectangle, V picks the Selection tool, which then
        // selects the second one; back to Free Transform, the box is the second one's own.
        press_and_move(&mut h, a.max, (a.max.0 + 30.0, a.max.1 + 20.0));
        h.key_press(Key::V);
        move_and_release(&mut h, (a.max.0 + 40.0, a.max.1 + 25.0));
        mouse_click(&mut h, (350.0, 125.0));
        h.key_press(Key::Q);
        h.step();
        let b = picked(h.state());
        assert!(b.min.0 > 250.0, "the second rectangle is selected: {b:?}");
        let t = target(h.state()).expect("a box");
        assert!(t.bx == rect(b) && close(t.pivot, middle(b)), "{t:?} around {b:?}");
    }

    #[test]
    fn the_document_changing_mid_drag_ends_it() {
        let mut h = two_rectangles();
        let a = picked(h.state());
        let elements = |h: &Harness<'_, Editor>| h.state().timeline().layers[0].frames[0].elements.clone();
        let before = elements(&h);

        // Ctrl+Z halfway through a drag takes it back, and the rest of the drag leaves it so.
        press_and_move(&mut h, a.max, (a.max.0 + 30.0, a.max.1 + 20.0));
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        move_and_release(&mut h, (a.max.0 + 60.0, a.max.1 + 40.0));
        assert_eq!(elements(&h), before, "the undo stands");
        assert!(h.state().can_redo());

        // Delete halfway through: the second rectangle, now first in the list, stays as it was.
        mouse_click(&mut h, (150.0, 125.0));
        press_and_move(&mut h, a.max, (a.max.0 + 30.0, a.max.1 + 20.0));
        h.key_press(Key::Delete);
        move_and_release(&mut h, (a.max.0 + 60.0, a.max.1 + 40.0));
        assert_eq!(elements(&h), before[1..]);
    }

    #[test]
    fn a_new_drawing_starts_with_its_own_transformation_point() {
        let mut h = rectangle();
        let a = picked(h.state());
        mouse_drag(&mut h, middle(a), a.min);
        assert!(target(h.state()).is_some_and(|t| close(t.pivot, a.min)));
        // A new document, and a rectangle the same size, selected the same way.
        h.state_mut().new_document(550.0, 400.0, 24.0);
        h.run();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (300.0, 200.0), (400.0, 250.0));
        h.state_mut().tool = Tool::FreeTransform;
        mouse_click(&mut h, (350.0, 225.0));
        let b = picked(h.state());
        let t = target(h.state()).expect("a box");
        assert!(close(t.pivot, middle(b)), "{t:?} around {b:?}");
    }

    #[test]
    fn a_turned_selection_keeps_its_box_while_only_moved() {
        let same = |a: Option<Target>, b: Matrix| a.is_some_and(|a| [(0.0, 0.0), (1.0, 1.0)].iter().all(|&q| close(a.bx.apply(q), b.apply(q))));
        let mut h = two_rectangles();
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::SelectAll, &ctx);
        h.state_mut().run(Action::Rotate90Cw, &ctx);
        let turned = target(h.state()).expect("a box").bx;
        assert!(turned.b > 0.0, "the box turned with the selection: {turned:?}");
        // The arrow keys move it, box and all.
        h.key_press(Key::ArrowRight);
        h.step();
        assert!(same(target(h.state()), Matrix::translate(1.0, 0.0) * turned));
        // Undoing the move keeps the box; undoing the turn too leaves the plain one.
        h.state_mut().run(Action::Undo, &ctx);
        assert!(same(target(h.state()), turned));
        h.state_mut().run(Action::Undo, &ctx);
        assert!(same(target(h.state()), rect(picked(h.state()))));
    }

    #[test]
    fn properties_showing_a_hairthin_selection_edits_nothing() {
        use minnamate_core::doc::Paint;
        let mut h = blank();
        let ed = h.state_mut();
        let sliver = minnamate_core::draw::rectangle([(100.0, 100.0), (100.02, 150.0)], 0.0, None, Some(Paint::Solid([0.0, 0.0, 0.0, 1.0])));
        ed.timeline_mut().layers[0].frames[0].elements.push(Element::Shape(sliver));
        ed.selection = vec![ElementRef { layer: 0, keyframe: 0, element: 0 }];
        ed.props_tab = crate::app::PropsTab::Object;
        h.run();
        assert!(!h.state().can_undo(), "W and H only change what is typed or dragged into them");
    }

    #[test]
    fn handles_with_shift_and_alt() {
        // A 100 × 50 box at the origin, its transformation point in the middle.
        let from = Target { bx: Matrix::scale(100.0, 50.0), pivot: (50.0, 25.0) };
        let drag = |zone, start: Point, p: Point, shift, alt| {
            Gesture { zone, start, from, originals: Vec::new(), m: Matrix::IDENTITY, rev: 0 }.matrix(p, shift, alt, 1.0)
        };
        let corner = Zone::Scale((1.0, 1.0));
        let m = drag(corner, (100.0, 50.0), (200.0, 60.0), false, false);
        assert!(close(m.apply((100.0, 50.0)), (200.0, 60.0)) && close(m.apply((0.0, 0.0)), (0.0, 0.0)));
        let m = drag(corner, (100.0, 50.0), (200.0, 60.0), true, false);
        assert!(close(m.apply((100.0, 50.0)), (200.0, 100.0)), "Shift: both axes double");
        let m = drag(corner, (100.0, 50.0), (150.0, 75.0), false, true);
        assert!(close(m.apply((0.0, 0.0)), (-50.0, -25.0)), "Alt: about the middle");
        let m = drag(Zone::Scale((1.0, 0.5)), (100.0, 25.0), (150.0, 40.0), false, false);
        assert!(close(m.apply((100.0, 50.0)), (150.0, 50.0)), "a side handle scales one way");
        let m = drag(Zone::Scale((1.0, 0.5)), (100.0, 25.0), (150.0, 40.0), true, false);
        assert!(close(m.apply((100.0, 0.0)), (150.0, -12.5)), "with Shift, the other way about the middle");

        let m = drag(Zone::Skew((0.5, 0.0)), (50.0, 0.0), (70.0, 5.0), false, false);
        assert!(close(m.apply((0.0, 0.0)), (20.0, 0.0)) && close(m.apply((0.0, 50.0)), (0.0, 50.0)), "the top slides");
        let m = drag(Zone::Skew((1.0, 0.5)), (100.0, 25.0), (100.0, 35.0), false, true);
        assert!(close(m.apply((100.0, 0.0)), (100.0, 10.0)) && close(m.apply((0.0, 0.0)), (0.0, -10.0)), "Alt: about the middle");

        // A quarter turn clockwise about the middle; Shift snaps 80° to 90°; Alt turns about the opposite corner.
        let turned = |(x, y): Point, (cx, cy): Point, deg: f32| {
            let (sin, cos) = deg.to_radians().sin_cos();
            (cx + cos * (x - cx) - sin * (y - cy), cy + sin * (x - cx) + cos * (y - cy))
        };
        let rotate = Zone::Rotate((1.0, 0.0));
        let start = (110.0, -10.0);
        let m = drag(rotate, start, turned(start, (50.0, 25.0), 90.0), false, false);
        assert!(close(m.apply((100.0, 0.0)), (75.0, 75.0)) && close(m.apply((50.0, 25.0)), (50.0, 25.0)));
        let m = drag(rotate, start, turned(start, (50.0, 25.0), 80.0), true, false);
        assert!(close(m.apply((100.0, 0.0)), (75.0, 75.0)));
        let m = drag(rotate, start, turned(start, (0.0, 50.0), 90.0), false, true);
        assert!(close(m.apply((100.0, 0.0)), (50.0, 150.0)) && close(m.apply((0.0, 50.0)), (0.0, 50.0)));

        let m = drag(Zone::Pivot, (50.0, 25.0), (99.5, 0.5), false, false);
        assert!(close(m.apply((50.0, 25.0)), (100.0, 0.0)), "snaps to the corner handle");
        let m = drag(Zone::Pivot, (50.0, 25.0), (70.0, 30.0), false, false);
        assert!(close(m.apply((50.0, 25.0)), (70.0, 30.0)));
    }

    #[test]
    fn zones_around_the_box() {
        let bx = Matrix::translate(100.0, 100.0) * Matrix::scale(200.0, 100.0);
        let zone = |x, y| zone(&bx, (200.0, 150.0), pos2(x, y));
        assert_eq!(zone(201.0, 152.0), Some(Zone::Pivot));
        assert_eq!(zone(303.0, 197.0), Some(Zone::Scale((1.0, 1.0))));
        assert_eq!(zone(100.0, 150.0), Some(Zone::Scale((0.0, 0.5))));
        assert_eq!(zone(150.0, 102.0), Some(Zone::Skew((0.5, 0.0))));
        assert_eq!(zone(160.0, 130.0), Some(Zone::Move));
        assert_eq!(zone(310.0, 90.0), Some(Zone::Rotate((1.0, 0.0))));
        assert_eq!(zone(330.0, 70.0), None, "too far out");
        assert_eq!(zone(150.0, 90.0), None, "above a side");
    }

    /// Writes target/screenshots/free_transform.png: a turned rectangle in its box.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn free_transform_screenshot() {
        let builder = Harness::builder().with_size(vec2(1600.0, 900.0)).with_step_dt(1.0 / 60.0).wgpu();
        let mut h = builder.build_eframe(|cc| Editor::new(&cc.egui_ctx, None));
        h.state_mut().new_document(550.0, 400.0, 24.0);
        h.run();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (150.0, 120.0), (330.0, 240.0));
        h.state_mut().tool = Tool::FreeTransform;
        mouse_click(&mut h, (240.0, 180.0));
        let b = shape_bounds(h.state());
        let out = 8.0 / h.state().zoom;
        mouse_drag(&mut h, (b.max.0 + out, b.min.1 - out), (b.max.0 + out, b.min.1 + 40.0));
        h.run();
        screenshot(&mut h, "free_transform");
    }
}
