//! Pen tool (P): click for corner points, drag for curves.

use std::f32::consts::FRAC_PI_4;

use eframe::egui::{
    self, Color32, Key, Modifiers, Painter, Rect, Response, Stroke, StrokeKind, Ui, emath::GuiRounding,
    epaint::CubicBezierShape, vec2,
};
use minnamate_core::doc::Point;
use minnamate_core::pen::{Anchor, pen_shape};

use crate::app::Editor;
use crate::stage::{local_point, screen_point};
use crate::theme;

/// The path being drawn, in the coordinates of the timeline being edited.
#[derive(Default)]
pub struct State {
    anchors: Vec<Anchor>,
    /// While the button is down: the point whose handles follow the pointer.
    dragging: Option<usize>,
    /// The pointer over the stage, for the preview.
    pointer: Option<Point>,
}

impl State {
    /// Whether pressing point `i` closes the path: only its first point does.
    fn closes(&self, i: usize) -> bool {
        i == 0 && self.anchors.len() > 1
    }
}

/// A click adds a corner point; a drag adds a smooth one, its outgoing handle
/// under the pointer. Shift keeps segments and handles to multiples of 45°.
/// Clicking or dragging on the first point closes the path; a double-click
/// or Ctrl+click ends it open.
pub fn input(ui: &Ui, ed: &mut Editor, resp: &Response, stage: Rect) {
    let (shift, command, origin) = ui.input(|i| (i.modifiers.shift, i.modifiers.command, i.pointer.press_origin()));
    ed.pen.pointer = resp.hover_pos().map(|pos| local_point(ed, stage, pos));
    // egui counts a third quick click as a triple-click, even where the first went elsewhere.
    if resp.double_clicked() || resp.triple_clicked() || (resp.clicked() && command) {
        finish(ed, false);
    } else if resp.clicked()
        && let Some(pos) = resp.interact_pointer_pos()
    {
        if press(ed, stage, local_point(ed, stage, pos), shift).is_some_and(|i| ed.pen.closes(i)) {
            finish(ed, true);
        }
    } else if resp.drag_started() && !command {
        // Start where the button went down, not where the drag was recognized.
        let at = origin.or(resp.interact_pointer_pos()).map(|pos| local_point(ed, stage, pos));
        ed.pen.dragging = at.and_then(|p| press(ed, stage, p, shift));
    }
    if resp.dragged()
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let out = local_point(ed, stage, pos);
        if let Some(a) = ed.pen.dragging.and_then(|i| ed.pen.anchors.get_mut(i)) {
            *a = Anchor::smooth(a.point, if shift { snap45(a.point, out) } else { out });
        }
    }
    if resp.drag_stopped() && ed.pen.dragging.take().is_some_and(|i| ed.pen.closes(i)) {
        finish(ed, true);
    }
}

/// Enter ends an open path. Runs before the editor's shortcuts, which would
/// take it for Play. (Escape ends it as Deselect All does.)
pub fn keys(ctx: &egui::Context, ed: &mut Editor) {
    if !ed.pen.anchors.is_empty() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
        finish(ed, false);
    }
}

/// Adds the path being drawn to the stage as one undo step, and starts afresh.
/// `closed` runs it back to its first point and fills the inside.
// ponytail: the path lands on the layer and frame selected when it ends, so picking another
// mid-path takes it along, where Animate leaves what was drawn in place. Remember them at
// the first point (layers would need stable ids to survive reordering) if that bites.
pub fn finish(ed: &mut Editor, closed: bool) {
    let anchors = std::mem::take(&mut ed.pen.anchors);
    ed.pen.dragging = None;
    if anchors.len() > 1 {
        ed.add_shape(pen_shape(&anchors, closed, Some(ed.stroke()), Some(ed.fill())));
    }
}

/// The button went down at `p`: on the path's first point that closes it, on
/// its other points it does nothing, elsewhere it adds a corner point. Returns
/// the point pressed.
fn press(ed: &mut Editor, stage: Rect, p: Point, shift: bool) -> Option<usize> {
    match anchor_at(ed, stage, p) {
        Some(i) => ed.pen.closes(i).then_some(i),
        None => {
            let p = target(ed, p, shift);
            ed.pen.anchors.push(Anchor::corner(p));
            Some(ed.pen.anchors.len() - 1)
        }
    }
}

/// The point of the path being drawn under `p`, within a few screen pixels.
fn anchor_at(ed: &Editor, stage: Rect, p: Point) -> Option<usize> {
    let at = screen_point(ed, stage, p);
    ed.pen.anchors.iter().position(|a| screen_point(ed, stage, a.point).distance(at) <= 4.0)
}

/// Where a press at `p` puts the next point: with Shift, on a 45° line from the last one.
fn target(ed: &Editor, p: Point, shift: bool) -> Point {
    match ed.pen.anchors.last() {
        Some(last) if shift => snap45(last.point, p),
        _ => p,
    }
}

/// `to`, turned about `from` onto the nearest multiple of 45°.
fn snap45(from: Point, to: Point) -> Point {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let angle = (dy.atan2(dx) / FRAC_PI_4).round() * FRAC_PI_4;
    let len = dx.hypot(dy);
    (from.0 + len * angle.cos(), from.1 + len * angle.sin())
}

/// The path so far and the segment the next click adds, the newest point's
/// handles, and the points: hollow squares, the newest solid. All in the
/// layer's outline color, as Animate draws paths.
pub fn overlay(p: &Painter, ed: &Editor, stage: Rect) {
    let pen = &ed.pen;
    let Some(last) = pen.anchors.last() else { return };
    let color = theme::LAYER_COLORS[ed.layer % theme::LAYER_COLORS.len()];
    let line = Stroke::new(1.0, color);
    let to_screen = |q: Point| screen_point(ed, stage, q);
    let curve = |a: &Anchor, b: &Anchor| {
        let points = [a.point, a.handle_out, b.handle_in, b.point].map(to_screen);
        p.add(CubicBezierShape::from_points_stroke(points, false, Color32::TRANSPARENT, line));
    };
    for pair in pen.anchors.windows(2) {
        curve(&pair[0], &pair[1]);
    }
    let over = pen.pointer.and_then(|q| anchor_at(ed, stage, q));
    if pen.dragging.or(over).is_some_and(|i| pen.closes(i)) {
        curve(last, &pen.anchors[0]);
    } else if pen.dragging.is_none()
        && let Some(q) = pen.pointer
    {
        let shift = p.ctx().input(|i| i.modifiers.shift);
        curve(last, &Anchor::corner(target(ed, q, shift)));
    }

    let shown = pen.dragging.and_then(|i| pen.anchors.get(i)).unwrap_or(last);
    for handle in [shown.handle_in, shown.handle_out] {
        if handle != shown.point {
            p.line_segment([to_screen(shown.point), to_screen(handle)], line);
            p.circle_filled(to_screen(handle), 2.0, color);
        }
    }
    for (i, a) in pen.anchors.iter().enumerate() {
        let square = Rect::from_center_size(to_screen(a.point), vec2(5.0, 5.0)).round_to_pixels(p.pixels_per_point());
        let fill = if i + 1 == pen.anchors.len() { color } else { Color32::WHITE };
        p.rect(square, 0.0, fill, line, StrokeKind::Inside);
    }
    // Over the first point, a small circle by the pointer: a click there closes the path.
    if over.is_some_and(|i| pen.closes(i))
        && let Some(q) = pen.pointer
    {
        p.circle(to_screen(q) + vec2(10.0, 10.0), 3.0, Color32::WHITE, Stroke::new(1.0, Color32::BLACK));
    }
}

/// The tool's section of Properties > Tool.
pub fn options(ui: &mut Ui, ed: &mut Editor) {
    let _ = (ui, ed);
}

#[cfg(test)]
mod tests {
    use eframe::egui::Event;
    use minnamate_core::doc::{Element, Seg, Shape};
    use minnamate_core::hit;

    use super::*;
    use crate::app::testkit::*;
    use crate::tools::Tool;

    /// The shapes on the current frame, bottom first.
    fn shapes(ed: &Editor) -> Vec<&Shape> {
        let all = ed.all_elements();
        let el = |r: &hit::ElementRef| &ed.timeline().layers[r.layer].frames[r.keyframe].elements[r.element];
        all.iter().filter_map(|r| if let Element::Shape(s) = el(r) { Some(s) } else { None }).collect()
    }

    #[test]
    fn click_corners_close_drag_curves() {
        let mut h = blank();
        h.state_mut().tool = Tool::Pen;
        for corner in [(100.0, 100.0), (200.0, 100.0), (150.0, 180.0)] {
            mouse_click(&mut h, corner);
        }
        assert_eq!(h.state().pen.anchors.len(), 3);
        assert!(h.state().all_elements().is_empty(), "nothing is drawn until the path ends");

        mouse_click(&mut h, (100.0, 100.0));
        let ed = h.state();
        assert!(ed.pen.anchors.is_empty(), "clicking the first point closes the path");
        let [triangle] = shapes(ed)[..] else { panic!("one shape") };
        assert_eq!((triangle.fills.len(), triangle.strokes.len()), (1, 1));
        assert!(hit::hit_test(&ed.doc, ed.timeline(), ed.frame, (150.0, 125.0), 0.0).is_some(), "filled inside");

        // A drag makes a smooth point; Escape ends the path open.
        mouse_drag(&mut h, (300.0, 100.0), (350.0, 100.0));
        mouse_click(&mut h, (400.0, 200.0));
        h.key_press(egui::Key::Escape);
        h.step();
        let ed = h.state();
        let [_, curve] = shapes(ed)[..] else { panic!("two shapes") };
        let segs = &curve.edges[0].paths[0].segs;
        assert!(curve.fills.is_empty() && segs.len() > 1 && segs.iter().all(|s| matches!(s, Seg::Quad(..))), "{segs:?}");
        // It leaves (300, 100) heading right, so halfway to (400, 200) it's at (368.75, 150), not on the chord.
        let stroke_at = |x, y| hit::hit_test(&ed.doc, ed.timeline(), ed.frame, (x, y), 1.0);
        assert!(stroke_at(368.75, 150.0).is_some() && stroke_at(350.0, 150.0).is_none());

        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
        h.step();
        assert_eq!(shapes(h.state()).len(), 1, "a whole path is one undo step");
    }

    #[test]
    fn double_click_or_another_tool_ends_the_path() {
        // Short frames, so two clicks in a row are a double-click.
        let builder = Harness::builder().with_size(vec2(1600.0, 900.0)).with_step_dt(0.05);
        let mut h = builder.build_eframe(|cc| Editor::new(&cc.egui_ctx, None));
        h.state_mut().new_document(550.0, 400.0, 24.0);
        h.run();
        h.state_mut().tool = Tool::Pen;
        mouse_click(&mut h, (100.0, 100.0));
        mouse_click(&mut h, (200.0, 100.0));
        mouse_click(&mut h, (200.0, 100.0));
        let ed = h.state();
        assert!(ed.pen.anchors.is_empty());
        let [line] = shapes(ed)[..] else { panic!("one shape") };
        let [Seg::Line((x, y))] = line.edges[0].paths[0].segs[..] else { panic!("one straight segment") };
        assert!((x - 200.0).abs() < 0.01 && (y - 100.0).abs() < 0.01 && line.fills.is_empty());

        // Shift-click keeps the segment to 45°; picking another tool ends the path.
        mouse_click(&mut h, (100.0, 200.0));
        h.event(Event::ModifiersChanged(egui::Modifiers::SHIFT));
        mouse_click(&mut h, (190.0, 280.0));
        h.event(Event::ModifiersChanged(egui::Modifiers::NONE));
        h.state_mut().tool = Tool::Selection;
        h.step();
        let ed = h.state();
        let [_, diagonal] = shapes(ed)[..] else { panic!("two shapes") };
        let Seg::Line((x, y)) = diagonal.edges[0].paths[0].segs[0] else { panic!() };
        assert!((x - 100.0 - (y - 200.0)).abs() < 0.01, "on the diagonal: {x}, {y}");

        // So does Edit > Deselect All.
        h.state_mut().tool = Tool::Pen;
        mouse_click(&mut h, (300.0, 300.0));
        mouse_click(&mut h, (400.0, 300.0));
        h.key_press_modifiers(egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT), egui::Key::A);
        h.step();
        assert_eq!((shapes(h.state()).len(), h.state().pen.anchors.len()), (3, 0));
    }

    /// Editing in place an empty symbol placed at (100, 50) at twice its size, with the Pen.
    fn editing_a_symbol() -> Harness<'static, Editor> {
        use minnamate_core::doc::{ColorTransform, Extra, Instance, Loop, Matrix, Symbol, SymbolKind, Timeline};
        let mut h = blank();
        let ed = h.state_mut();
        let timeline = Timeline { name: "Symbol 1".into(), ..ed.doc.scenes[0].clone() };
        ed.doc.library.insert("Symbol 1".into(), Symbol { kind: SymbolKind::Graphic, timeline, extra: Extra::default() });
        let matrix = Matrix { a: 2.0, d: 2.0, tx: 100.0, ty: 50.0, ..Matrix::IDENTITY };
        let instance = Instance {
            symbol: "Symbol 1".into(),
            kind: SymbolKind::Graphic,
            matrix,
            pivot: (0.0, 0.0),
            color: ColorTransform::IDENTITY,
            looping: Loop::Loop,
            first_frame: 0,
            extra: Extra::default(),
        };
        ed.doc.scenes[0].layers[0].frames[0].elements.push(Element::Instance(instance));
        ed.enter_symbol(hit::ElementRef { layer: 0, keyframe: 0, element: 0 });
        ed.tool = Tool::Pen;
        h.step();
        h
    }

    #[test]
    fn draws_in_the_symbol_being_edited() {
        let mut h = editing_a_symbol();
        // Scene points; the symbol's own are (scene - (100, 50)) / 2.
        for corner in [(100.0, 50.0), (200.0, 50.0), (150.0, 150.0)] {
            mouse_click(&mut h, corner);
        }
        let near = |a: Point, b: Point| (a.0 - b.0).hypot(a.1 - b.1) < 0.75;
        let points: Vec<Point> = h.state().pen.anchors.iter().map(|a| a.point).collect();
        assert!(points.len() == 3 && near(points[1], (50.0, 0.0)) && near(points[2], (25.0, 50.0)), "{points:?}");
        // The first point's hot spot is a few screen pixels across, whatever the symbol's scale.
        let seven_px = 7.0 / h.state().zoom;
        mouse_click(&mut h, (100.0 + seven_px, 50.0));
        assert_eq!(h.state().pen.anchors.len(), 4, "7 px off the first point adds a point");
        mouse_click(&mut h, (100.0, 50.0));
        let ed = h.state();
        let symbol = &ed.doc.library["Symbol 1"].timeline;
        let drawn = hit::bounds(&ed.doc, symbol, 0, &[hit::ElementRef { layer: 0, keyframe: 0, element: 0 }]);
        assert!(drawn.is_some_and(|b| near(b.min, (-0.5, -0.5)) && near(b.max, (50.5, 50.5))), "{drawn:?}");
        assert_eq!(ed.doc.scenes[0].layers[0].frames[0].elements.len(), 1, "the scene keeps just the instance");

        // Leaving the symbol with a path open ends the path there.
        mouse_click(&mut h, (100.0, 200.0));
        mouse_click(&mut h, (200.0, 200.0));
        h.state_mut().exit_symbol();
        h.step();
        let ed = h.state();
        assert!(ed.pen.anchors.is_empty());
        assert_eq!(ed.doc.library["Symbol 1"].timeline.layers[0].frames[0].elements.len(), 2);
        assert_eq!(ed.doc.scenes[0].layers[0].frames[0].elements.len(), 1);
    }

    #[test]
    fn a_new_document_drops_the_path_being_drawn() {
        let mut h = blank();
        h.state_mut().tool = Tool::Pen;
        mouse_click(&mut h, (100.0, 100.0));
        mouse_click(&mut h, (200.0, 100.0));
        let ctx = h.ctx.clone();
        h.state_mut().run(crate::app::Action::New, &ctx);
        h.step();
        // Enter is New Document's Create.
        h.key_press(egui::Key::Enter);
        h.step();
        h.state_mut().tool = Tool::Selection;
        h.step();
        assert!(h.state().all_elements().is_empty());
    }

    /// Writes target/screenshots/pen.png, a path being drawn with its last point
    /// dragged smooth, and pen-close.png, the pointer over its first point.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn pen_screenshot() {
        let builder = Harness::builder().with_size(vec2(1600.0, 900.0)).wgpu();
        let mut h = builder.build_eframe(|cc| Editor::new(&cc.egui_ctx, None));
        h.state_mut().new_document(550.0, 400.0, 24.0);
        h.run();
        h.state_mut().tool = Tool::Pen;
        mouse_click(&mut h, (100.0, 100.0));
        mouse_click(&mut h, (250.0, 80.0));
        mouse_drag(&mut h, (300.0, 200.0), (360.0, 230.0));
        h.hover_at(on_stage(&h, (200.0, 300.0)));
        h.step();
        screenshot(&mut h, "pen");
        h.hover_at(on_stage(&h, (101.0, 101.0)));
        h.step();
        screenshot(&mut h, "pen-close");
    }
}
