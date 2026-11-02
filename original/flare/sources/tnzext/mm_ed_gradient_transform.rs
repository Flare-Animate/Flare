//! Gradient Transform tool (F), in the Free Transform group (docs/animate-ui.md
//! §6.3). A click takes the gradient fill under it; its handles then move the
//! gradient (the ringed middle), turn it (the black dot), stretch it along its
//! axis (the square) and, for a radial one, scale it evenly (the ringed arrow)
//! and slide its focal point (the triangle). Each drag is one undo step.

use std::f32::consts::{FRAC_PI_4, TAU};

use eframe::egui::{self, Color32, CursorIcon, Painter, Pos2, Rect, Response, Shape, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use minnamate_core::doc::{Element, Gradient, GradientKind, Matrix, Paint, Point};
use minnamate_core::gradient::HALF;
use minnamate_core::hit::ElementRef;
use minnamate_core::paint;

use crate::app::Editor;
use crate::cursors::{self, Cursor};
use crate::stage::local_point;

/// How near a handle grabs it, in screen points.
const GRAB: f32 = 6.0;
/// The focal point's triangle stands this far above it, clear of the middle's circle.
const FOCAL_LIFT: Vec2 = vec2(0.0, -7.0);

/// The fill being transformed, and the drag in progress.
#[derive(Default)]
pub struct State {
    /// A shape element of the edited timeline and its fill style, counted from 1.
    target: Option<(ElementRef, u32)>,
    drag: Option<Drag>,
}

struct Drag {
    handle: Handle,
    /// Where it began, in the shape's coordinates.
    start: Point,
    /// The gradient then.
    from: Gradient,
    /// `Editor::rev` once the drag last wrote: anything else changing the document ends the drag.
    rev: u32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Handle {
    Center,
    Focal,
    Width,
    Size,
    Rotate,
}

/// Where the handles sit on the gradient square: a linear gradient's on its
/// right end, a radial one's around its circle's lower right, as Animate has them.
// ponytail: placed from recall (width, then size, then rotation down the edge); measure Animate's.
fn handles(kind: GradientKind) -> Vec<(Handle, Point)> {
    let around = |angle: f32| (HALF * angle.cos(), HALF * angle.sin());
    match kind {
        GradientKind::Linear => vec![(Handle::Width, (HALF, 0.0)), (Handle::Rotate, (HALF, HALF)), (Handle::Center, (0.0, 0.0))],
        GradientKind::Radial { focal } => vec![
            (Handle::Width, around(0.0)),
            (Handle::Size, around(FRAC_PI_4)),
            (Handle::Rotate, around(2.0 * FRAC_PI_4)),
            (Handle::Focal, (focal * HALF, 0.0)),
            (Handle::Center, (0.0, 0.0)),
        ],
    }
}

fn on_screen(m: &Matrix, q: Point) -> Pos2 {
    let (x, y) = m.apply(q);
    pos2(x, y)
}

/// The handle at screen point `pos`, the gradient square going to the screen by `m`.
fn handle_at(m: &Matrix, kind: GradientKind, pos: Pos2) -> Option<Handle> {
    let lift = |h: Handle| if h == Handle::Focal { FOCAL_LIFT } else { Vec2::ZERO };
    handles(kind).into_iter().find(|&(h, q)| (on_screen(m, q) + lift(h)).distance(pos) <= GRAB).map(|(h, _)| h)
}

/// The target: its element, fill index, shape matrix and gradient, while its
/// shape is the lone selection and that fill a gradient.
// ponytail: fills only; a stroke's gradient keeps the matrix it was drawn or inked with. Take
// strokes too (paint::pick finds them) if they need transforming.
fn target(ed: &Editor) -> Option<(ElementRef, usize, Matrix, &Gradient)> {
    let (r, fill) = ed.gradient_transform.target?;
    if ed.selection != [r] {
        return None;
    }
    let Some(Element::Shape(s)) = ed.element(r) else { return None };
    let i = (fill as usize).checked_sub(1)?;
    let Paint::Gradient(g) = s.fills.get(i)? else { return None };
    Some((r, i, s.matrix, g))
}

/// Pointer input on the stage while this tool is active.
pub fn input(ui: &Ui, ed: &mut Editor, resp: &Response, stage: Rect) {
    let screen = crate::free_transform::screen(ed, stage);
    if let Some(drag) = ed.gradient_transform.drag.take()
        && drag.rev == ed.rev
    {
        if resp.drag_stopped() {
            return ed.end_gesture();
        }
        if resp.dragged() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let rev = update(ed, &drag, local_point(ed, stage, pos));
                ed.gradient_transform.drag = Some(Drag { rev, ..drag });
            }
            return;
        }
    }
    let shown = target(ed).map(|(_, _, s, g)| (screen * s * g.matrix, g.kind));
    let at = |pos: Pos2| shown.and_then(|(m, kind)| handle_at(&m, kind, pos));
    if resp.hovered()
        && let (Some(pos), Some((m, _))) = (resp.hover_pos(), shown)
        && let Some(h) = at(pos)
    {
        cursor(ui.ctx(), h, &m);
    }
    if resp.drag_started()
        && let Some(origin) = ui.input(|i| i.pointer.press_origin())
        && let Some(handle) = at(origin)
        && let Some((_, _, s, g)) = target(ed)
        && let Some(inverse) = s.inverse()
    {
        let start = inverse.apply(local_point(ed, stage, origin));
        let mut drag = Drag { handle, start, from: g.clone(), rev: ed.rev };
        if let Some(pos) = resp.interact_pointer_pos() {
            drag.rev = update(ed, &drag, local_point(ed, stage, pos));
        }
        ed.gradient_transform.drag = Some(drag);
        return;
    }
    // A click off the handles takes the gradient fill there, if any.
    if resp.clicked()
        && let Some(pos) = resp.interact_pointer_pos()
        && at(pos).is_none()
    {
        let found = paint::fill_at(&ed.doc, ed.timeline(), ed.frame, local_point(ed, stage, pos));
        let gradient = |&(r, fill): &(ElementRef, u32)| match ed.element(r) {
            Some(Element::Shape(s)) => matches!(s.fills.get(fill as usize - 1), Some(Paint::Gradient(_))),
            _ => false,
        };
        let found = found.filter(gradient);
        ed.gradient_transform.target = found;
        ed.selection = found.map(|(r, _)| vec![r]).unwrap_or_default();
        crate::stage::show_selection_props(ed);
    }
}

/// Drags the handle to `p` (in the edited timeline's coordinates), live, as
/// one undo step. Returns `Editor::rev` after it.
fn update(ed: &mut Editor, drag: &Drag, p: Point) -> u32 {
    let Some((r, i, s, now)) = target(ed) else { return ed.rev };
    let Some(p) = s.inverse().map(|inverse| inverse.apply(p)) else { return ed.rev };
    let g = drag.dragged(p);
    if g == *now {
        return ed.rev;
    }
    ed.checkpoint_gesture("gradient-transform");
    if let Some(Element::Shape(s)) = ed.element_mut(r) {
        s.fills[i] = Paint::Gradient(g);
    }
    ed.rev
}

impl Drag {
    /// The gradient with the handle dragged to `p`, in the shape's coordinates.
    /// Stretching and scaling keep the middle where it is; turning is about it.
    fn dragged(&self, p: Point) -> Gradient {
        let mut g = self.from.clone();
        let m = self.from.matrix;
        let Some(inverse) = m.inverse() else { return g };
        // The pointer now and at the start, in the gradient square.
        let (q, q0) = (inverse.apply(p), inverse.apply(self.start));
        // Never quite flat, so the matrix stays invertible.
        let ratio = |a: f32, b: f32| {
            let k = if b.abs() > 1e-6 { a / b } else { 1.0 };
            if k.abs() < 0.01 { 0.01f32.copysign(k) } else { k }
        };
        match self.handle {
            Handle::Center => g.matrix = Matrix::translate(p.0 - self.start.0, p.1 - self.start.1) * m,
            Handle::Width => g.matrix = m * Matrix::scale(ratio(q.0, q0.0), 1.0),
            Handle::Size => {
                let k = ratio(q.0.hypot(q.1), q0.0.hypot(q0.1));
                g.matrix = m * Matrix::scale(k, k);
            }
            Handle::Rotate => {
                let c = m.apply((0.0, 0.0));
                let angle = |p: Point| (p.1 - c.1).atan2(p.0 - c.0);
                g.matrix = Matrix::rotate(angle(p) - angle(self.start)).about(c) * m;
            }
            Handle::Focal => {
                if let GradientKind::Radial { focal } = g.kind {
                    g.kind = GradientKind::Radial { focal: (focal + (q.0 - q0.0) / HALF).clamp(-0.99, 0.99) };
                }
            }
        }
        g
    }
}

/// The pointer over `handle`, the gradient square going to the screen by `m`:
/// arrows along the way it stretches, the turning arrows to rotate.
fn cursor(ctx: &egui::Context, handle: Handle, m: &Matrix) {
    // An arrow along the direction (x, y) on screen.
    let along = |(x, y): Point| match ((y.atan2(x) / FRAC_PI_4).round() as i32).rem_euclid(4) {
        0 => CursorIcon::ResizeHorizontal,
        1 => CursorIcon::ResizeNwSe,
        2 => CursorIcon::ResizeVertical,
        _ => CursorIcon::ResizeNeSw,
    };
    match handle {
        Handle::Center => ctx.set_cursor_icon(CursorIcon::Move),
        Handle::Width | Handle::Focal => ctx.set_cursor_icon(along((m.a, m.b))),
        Handle::Size => ctx.set_cursor_icon(along((m.a + m.c, m.b + m.d))),
        Handle::Rotate => cursors::show(ctx, Cursor::Rotate),
    }
}

/// The target's extent and handles, over the stage (screen space): a linear
/// gradient's two ends, a radial one's circle, black on a white rim.
// ponytail: the look is our own guess at Animate's (lines and handle shapes from recall);
// match a 2024 capture.
pub fn overlay(p: &Painter, ed: &Editor, stage: Rect) {
    let Some((_, _, s, g)) = target(ed) else { return };
    let m = crate::free_transform::screen(ed, stage) * s * g.matrix;
    let at = |q: Point| on_screen(&m, q);
    let lines: Vec<Vec<Pos2>> = match g.kind {
        GradientKind::Linear => [-HALF, HALF].iter().map(|&x| vec![at((x, -HALF)), at((x, HALF))]).collect(),
        GradientKind::Radial { .. } => vec![(0..=72).map(|k| k as f32 / 72.0 * TAU).map(|a| at((HALF * a.cos(), HALF * a.sin()))).collect()],
    };
    for (width, color) in [(3.0, Color32::WHITE), (1.0, Color32::BLACK)] {
        for line in &lines {
            p.add(Shape::line(line.clone(), Stroke::new(width, color)));
        }
    }
    let ink = Stroke::new(1.0, Color32::BLACK);
    for (handle, q) in handles(g.kind) {
        let c = at(q);
        match handle {
            Handle::Center => {
                p.circle(c, 5.0, Color32::WHITE, ink);
                p.circle_filled(c, 1.5, Color32::BLACK);
            }
            Handle::Focal => {
                let c = c + FOCAL_LIFT;
                let triangle = vec![c + vec2(-4.5, -3.0), c + vec2(4.5, -3.0), c + vec2(0.0, 5.0)];
                p.add(Shape::convex_polygon(triangle, Color32::WHITE, ink));
            }
            Handle::Width => {
                p.rect(Rect::from_center_size(c, vec2(8.0, 8.0)), 0.0, Color32::WHITE, ink, StrokeKind::Middle);
                p.hline(c.x - 2.5..=c.x + 2.5, c.y, ink);
            }
            Handle::Size => {
                p.circle(c, 4.5, Color32::WHITE, ink);
                p.line_segment([c - vec2(2.0, 2.0), c + vec2(2.0, 2.0)], ink);
                p.line(vec![c + vec2(-0.5, 2.0), c + vec2(2.0, 2.0), c + vec2(2.0, -0.5)], ink);
            }
            Handle::Rotate => {
                p.circle(c, 4.0, Color32::BLACK, Stroke::new(1.5, Color32::WHITE));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Key, Modifiers};
    use minnamate_core::doc::Shape as DocShape;

    use super::*;
    use crate::app::testkit::*;
    use crate::tools::Tool;

    fn shape(ed: &Editor) -> DocShape {
        match ed.timeline().layers[0].frames[0].elements.first() {
            Some(Element::Shape(s)) => s.clone(),
            other => panic!("{other:?}"),
        }
    }

    fn gradient(ed: &Editor) -> Gradient {
        match &shape(ed).fills[0] {
            Paint::Gradient(g) => g.clone(),
            other => panic!("{other:?}"),
        }
    }

    /// Where a handle of the drawn gradient is on screen.
    fn handle(h: &Harness<'_, Editor>, which: Handle) -> Pos2 {
        let ed = h.state();
        let (s, g) = (shape(ed), gradient(ed));
        let m = crate::free_transform::screen(ed, ed.stage_rect) * s.matrix * g.matrix;
        let (_, q) = handles(g.kind).into_iter().find(|&(k, _)| k == which).unwrap();
        on_screen(&m, q) + if which == Handle::Focal { FOCAL_LIFT } else { Vec2::ZERO }
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.02
    }

    /// A 200 × 100 rectangle at (100, 100) drawn with a `kind` gradient, taken by the Gradient Transform tool.
    fn drawn(kind: GradientKind) -> Harness<'static, Editor> {
        let mut h = blank();
        h.state_mut().fill_gradient = Some(Gradient::new(kind));
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (300.0, 200.0));
        h.key_press(Key::F);
        h.step();
        assert_eq!(h.state().tool, Tool::GradientTransform);
        mouse_click(&mut h, (150.0, 150.0));
        assert_eq!(h.state().selection.len(), 1, "the click takes the fill");
        h
    }

    #[test]
    fn handles_move_turn_and_stretch_a_linear_gradient() {
        let mut h = drawn(GradientKind::Linear);
        let fitted = gradient(h.state());
        assert_eq!((fitted.matrix.tx, fitted.matrix.ty, fitted.matrix.a * 2.0 * HALF), (200.0, 150.0, 200.0), "across the rectangle");
        let zoom = h.state().zoom;

        // The middle moves it.
        let (a, b) = (handle(&h, Handle::Center), handle(&h, Handle::Center) + vec2(30.0, 10.0) * zoom);
        screen_drag(&mut h, a, b, Modifiers::NONE);
        let moved = gradient(h.state());
        assert!(close(moved.matrix.tx, 230.0) && close(moved.matrix.ty, 160.0), "{:?}", moved.matrix);
        // The square stretches it along its axis, about its middle: its end follows the pointer.
        let (a, b) = (handle(&h, Handle::Width), handle(&h, Handle::Width) + vec2(50.0, 0.0) * zoom);
        screen_drag(&mut h, a, b, Modifiers::NONE);
        let wide = gradient(h.state());
        assert!(close(wide.matrix.a * 2.0 * HALF, 300.0) && close(wide.matrix.tx, 230.0), "{:?}", wide.matrix);
        // The dot turns it about its middle: a quarter turn clockwise.
        let (a, c) = (handle(&h, Handle::Rotate), on_stage(&h, (230.0, 160.0)));
        screen_drag(&mut h, a, c + vec2(c.y - a.y, a.x - c.x), Modifiers::NONE);
        let turned = gradient(h.state());
        assert!(close(turned.matrix.a, 0.0) && turned.matrix.b > 0.1, "{:?}", turned.matrix);

        // Each drag was one undo step.
        let ctx = h.ctx.clone();
        for want in [&wide, &moved, &fitted] {
            h.state_mut().run(crate::app::Action::Undo, &ctx);
            assert_eq!(gradient(h.state()).matrix, want.matrix);
        }
    }

    #[test]
    fn a_radial_gradient_scales_and_moves_its_focal_point() {
        let mut h = drawn(GradientKind::Radial { focal: 0.0 });
        let zoom = h.state().zoom;
        // The circle reaches the rectangle's longer sides: radius 100.
        let (a, b) = (handle(&h, Handle::Size), handle(&h, Handle::Size) - vec2(50.0, 50.0) * std::f32::consts::FRAC_1_SQRT_2 * zoom);
        screen_drag(&mut h, a, b, Modifiers::NONE);
        let smaller = gradient(h.state());
        assert!(close(smaller.matrix.a * HALF, 50.0) && close(smaller.matrix.d * HALF, 50.0), "evenly: {:?}", smaller.matrix);
        // The triangle slides the focal point along the axis, here half way out.
        let (a, b) = (handle(&h, Handle::Focal), handle(&h, Handle::Focal) + vec2(25.0, 0.0) * zoom);
        screen_drag(&mut h, a, b, Modifiers::NONE);
        let GradientKind::Radial { focal } = gradient(h.state()).kind else { panic!("not radial") };
        assert!(close(focal, 0.5), "{focal}");
        // A click on a solid fill leaves the tool nothing to transform.
        h.state_mut().fill_gradient = None;
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (350.0, 100.0), (400.0, 150.0));
        h.state_mut().tool = Tool::GradientTransform;
        mouse_click(&mut h, (375.0, 125.0));
        assert!(h.state().selection.is_empty() && target(h.state()).is_none());
    }

    /// Writes target/screenshots/gradient_fill.png: a radial and a linear fill,
    /// the radial one's Gradient Transform handles showing, its focal point
    /// moved; and gradient_fill_linear.png, the linear one's.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn gradient_fill_screenshot() {
        let builder = Harness::builder().with_size(vec2(1600.0, 900.0)).with_step_dt(1.0 / 60.0).wgpu();
        let mut h = builder.build_eframe(|cc| Editor::new(&cc.egui_ctx, None));
        h.state_mut().new_document(550.0, 400.0, 24.0);
        h.run();
        let presets = crate::color::preset_gradients();
        (h.state_mut().tool, h.state_mut().fill_gradient) = (Tool::Oval, Some(presets[2].clone()));
        mouse_drag(&mut h, (60.0, 80.0), (260.0, 280.0));
        (h.state_mut().tool, h.state_mut().fill_gradient) = (Tool::Rectangle, Some(presets[5].clone()));
        mouse_drag(&mut h, (300.0, 120.0), (500.0, 240.0));
        h.key_press(Key::F);
        h.step();
        mouse_click(&mut h, (160.0, 180.0));
        let a = on_stage(&h, (160.0, 180.0)) + FOCAL_LIFT;
        let b = a + vec2(-40.0, 0.0) * h.state().zoom;
        screen_drag(&mut h, a, b, Modifiers::NONE);
        h.hover_at(on_stage(&h, (300.0, 330.0)));
        h.run();
        screenshot(&mut h, "gradient_fill");
        mouse_click(&mut h, (400.0, 180.0));
        h.run();
        screenshot(&mut h, "gradient_fill_linear");
    }
}
