//! Paint Bucket (K), Ink Bottle (S) and Eyedropper (I): click-to-apply color tools.

use eframe::egui::{Color32, CursorIcon, Painter, PointerButton, Rect, Response, Ui, Vec2, vec2};
use minnamate_core::doc::{Element, Paint, Point, Rgba, Stroke};
use minnamate_core::paint::{self, Picked};

use crate::app::Editor;
use crate::icons;
use crate::stage::local_point;
use crate::theme;
use crate::tools::Tool;
use crate::widgets;

/// The Paint Bucket's options, and its drag in progress.
#[derive(Default)]
pub struct State {
    pub gap: Gap,
    /// Animate's Lock Fill: a gradient keeps the place the last fill gave it,
    /// rather than spanning each area filled.
    pub lock_fill: bool,
    /// Animate's "Fill all regions": a drag fills every area it crosses, not
    /// only those filled like the one it started in.
    pub fill_all: bool,
    /// While the Paint Bucket drags: the fill its press replaced (None: an empty area).
    replacing: Option<Option<Paint>>,
}

/// How wide a gap in an outline the Paint Bucket closes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Gap {
    DontClose,
    #[default]
    Small,
    Medium,
    Large,
}

impl Gap {
    pub(crate) const ALL: [Gap; 4] = [Gap::DontClose, Gap::Small, Gap::Medium, Gap::Large];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Gap::DontClose => "Don't Close Gaps",
            Gap::Small => "Close Small Gaps",
            Gap::Medium => "Close Medium Gaps",
            Gap::Large => "Close Large Gaps",
        }
    }

    /// The widest gap closed, in screen pixels.
    // ponytail: guessed sizes; measure Animate's and tune.
    fn pixels(self) -> f32 {
        match self {
            Gap::DontClose => 0.0,
            Gap::Small => 4.0,
            Gap::Medium => 10.0,
            Gap::Large => 20.0,
        }
    }
}

/// Pointer input on the stage while one of these tools is active, on the
/// timeline being edited. Each acts once per press, where it went down: on
/// release, or as soon as the press turns into a drag (egui reports a press
/// that moves or lingers as a drag, never a click). Dragging on, the Paint
/// Bucket fills each area it crosses that was filled like the first, as
/// Animate's does.
pub fn input(ui: &Ui, ed: &mut Editor, resp: &Response, stage: Rect) {
    // A gesture another tool started, cut short by a tool key, ends here.
    ed.drag = None;
    let press = if resp.drag_started_by(PointerButton::Primary) {
        ui.input(|i| i.pointer.press_origin())
    } else {
        resp.interact_pointer_pos().filter(|_| resp.clicked())
    };
    if let Some(pos) = press {
        let p = local_point(ed, stage, pos);
        // Strokes get a few screen pixels of slack, as with the Selection tool.
        let slack = 3.0 / ed.zoom;
        match ed.tool {
            Tool::Eyedropper => {
                if let Some(picked) = paint::pick(&ed.doc, ed.timeline(), ed.frame, p, slack) {
                    eyedrop(ed, picked);
                }
            }
            Tool::PaintBucket => {
                if resp.dragged() {
                    ed.paint.replacing = Some(paint::area_fill(&ed.doc, ed.timeline(), ed.frame, p, gap(ed)).flatten());
                }
                // Each press is an undo step of its own, its drag included.
                ed.end_gesture();
                fill(ed, p);
            }
            Tool::InkBottle => {
                if let Some((r, shape)) = paint::ink(&ed.doc, ed.timeline(), ed.frame, p, slack, ed.stroke()) {
                    ed.checkpoint();
                    if let Some(el) = ed.element_mut(r) {
                        *el = Element::Shape(shape);
                    }
                }
            }
            _ => {}
        }
    }
    if ed.tool == Tool::PaintBucket
        && resp.dragged_by(PointerButton::Primary)
        && resp.drag_delta() != Vec2::ZERO
        && let (Some(replacing), Some(pos)) = (ed.paint.replacing.clone(), resp.interact_pointer_pos())
    {
        let p = local_point(ed, stage, pos);
        if ed.paint.fill_all || paint::area_fill(&ed.doc, ed.timeline(), ed.frame, p, gap(ed)) == Some(replacing) {
            fill(ed, p);
        }
    }
    if !resp.dragged() {
        ed.paint.replacing = None;
    }
}

/// The Paint Bucket's gap size in the edited timeline's units.
fn gap(ed: &Editor) -> f32 {
    ed.paint.gap.pixels() / ed.zoom
}

/// Paint Bucket at `p`, as part of the press's undo step. A gradient spans the
/// area it fills, or with Lock Fill lies where the last fill (or the Eyedropper)
/// left it, so areas filled one after another show one gradient.
fn fill(ed: &mut Editor, p: Point) {
    let mut paint = ed.fill();
    if let Paint::Gradient(g) = &mut paint
        && let Some((area, m)) = paint::area_box(&ed.doc, ed.timeline(), ed.frame, p, gap(ed))
    {
        if ed.paint.lock_fill
            && let Some(inverse) = m.inverse()
        {
            g.matrix = inverse * g.matrix;
        } else {
            g.fit(area);
            if let Some(default) = &mut ed.fill_gradient {
                default.matrix = m * g.matrix;
            }
        }
    }
    if let Some((r, shape)) = paint::bucket(&ed.doc, ed.timeline(), ed.frame, p, gap(ed), paint) {
        ed.checkpoint_gesture("paint-bucket");
        if let Some(el) = ed.element_mut(r) {
            *el = Element::Shape(shape);
        }
    }
}

/// Takes up a picked fill or stroke and hands over to the tool that applies it,
/// as Animate does: a fill to the Paint Bucket, with Lock Fill on; a stroke to
/// the Ink Bottle.
fn eyedrop(ed: &mut Editor, picked: Picked) {
    match picked {
        Picked::Fill(Paint::Solid(c)) => {
            (ed.fill_color, ed.fill_gradient, ed.paint.lock_fill, ed.tool) = (color32(c), None, true, Tool::PaintBucket);
        }
        // Lock Fill on, the bucket carries the gradient on from where it was picked.
        Picked::Fill(Paint::Gradient(g)) => (ed.fill_gradient, ed.paint.lock_fill, ed.tool) = (Some(g), true, Tool::PaintBucket),
        Picked::Stroke(Stroke { paint: Paint::Solid(c), width, .. }) => {
            (ed.stroke_color, ed.stroke_gradient, ed.stroke_width, ed.tool) = (color32(c), None, width, Tool::InkBottle);
        }
        Picked::Stroke(Stroke { paint: Paint::Gradient(g), width, .. }) => {
            (ed.stroke_gradient, ed.stroke_width, ed.tool) = (Some(g), width, Tool::InkBottle);
        }
        _ => ed.log("The Eyedropper can't pick bitmap fills yet."),
    }
}

/// The document's straight RGBA as an egui color.
fn color32(c: Rgba) -> Color32 {
    let [r, g, b, a] = c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// The tool itself as the pointer, as Animate's cursors for these tools are:
/// black with a white rim, acting at the bucket's drip, the bottle's base or
/// the dropper's tip. stage.rs hides the system pointer over the stage for
/// these tools, and the glyph shows exactly then.
pub fn overlay(p: &Painter, ed: &Editor, _stage: Rect) {
    let ctx = p.ctx();
    let hidden = ctx.output(|o| o.cursor_icon == CursorIcon::None);
    let Some(pos) = ctx.pointer_hover_pos().filter(|_| hidden) else { return };
    // On the icons' 18-unit grid, drawn here at one unit per point.
    let tip = match ed.tool {
        Tool::PaintBucket => vec2(13.6, 16.4),
        Tool::InkBottle => vec2(13.2, 14.4),
        _ => vec2(2.2, 15.8),
    };
    let glyph = Rect::from_min_size(pos - tip, vec2(18.0, 18.0));
    for dx in [-1.0, 0.0, 1.0] {
        for dy in [-1.0, 0.0, 1.0] {
            icons::paint(p, glyph.translate(vec2(dx, dy)), ed.tool.icon(), Color32::WHITE);
        }
    }
    icons::paint(p, glyph, ed.tool.icon(), Color32::BLACK);
}

/// The Paint Bucket's section of Properties > Tool. The Ink Bottle's stroke
/// color and size are in Color and Style above it; the Eyedropper has no options.
pub fn options(ui: &mut Ui, ed: &mut Editor) {
    if ed.tool != Tool::PaintBucket {
        return;
    }
    theme::section(ui, "Tool Options", |ui| {
        widgets::row(ui, |ui| {
            widgets::label(ui, "Gap size", widgets::WORD_W);
            let gaps: Vec<_> = Gap::ALL.iter().map(|&g| (g, g.name(), true)).collect();
            if let Some(gap) = widgets::pick(ui, "Gap size", ed.paint.gap, &gaps, ui.available_width() - 4.0) {
                ed.paint.gap = gap;
            }
        });
        widgets::row(ui, |ui| widgets::checkbox(ui, &mut ed.paint.lock_fill, "Lock fill"));
        widgets::row(ui, |ui| widgets::checkbox(ui, &mut ed.paint.fill_all, "Fill all regions"));
    });
}

#[cfg(test)]
mod tests {
    use eframe::egui::{self, Color32, Key, Modifiers, PointerButton, vec2};
    use minnamate_core::doc::{Element, Paint, Shape};

    use crate::app::testkit::*;
    use crate::app::{Editor, PropsTab};
    use crate::tools::Tool;

    const RED: Paint = Paint::Solid([1.0, 0.0, 0.0, 1.0]);

    fn rectangle(ed: &Editor) -> &Shape {
        let Some(Element::Shape(s)) = ed.timeline().layers[0].frames[0].elements.first() else { panic!("no rectangle") };
        s
    }

    #[test]
    fn eyedrop_bucket_and_ink_a_rectangle() {
        let mut h = blank();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        let drawn = rectangle(h.state()).clone();
        let (blue, inside, on_stroke) = (h.state().fill_color, (150.0, 125.0), (100.0, 125.0));

        // The Eyedropper picks up the fill and hands it to the Paint Bucket, Lock Fill on.
        h.state_mut().tool = Tool::Eyedropper;
        h.state_mut().fill_color = Color32::RED;
        mouse_click(&mut h, inside);
        let ed = h.state();
        assert_eq!((ed.tool, ed.fill_color, ed.paint.lock_fill), (Tool::PaintBucket, blue, true));

        h.state_mut().fill_color = Color32::RED;
        mouse_click(&mut h, inside);
        assert_eq!(rectangle(h.state()).fills, [RED]);

        // Picking the stroke hands its color and size to the Ink Bottle.
        h.state_mut().tool = Tool::Eyedropper;
        (h.state_mut().stroke_color, h.state_mut().stroke_width) = (Color32::GREEN, 5.0);
        mouse_click(&mut h, on_stroke);
        let ed = h.state();
        assert_eq!((ed.tool, ed.stroke_color, ed.stroke_width), (Tool::InkBottle, Color32::BLACK, 1.0));

        (h.state_mut().stroke_color, h.state_mut().stroke_width) = (Color32::GREEN, 4.0);
        mouse_click(&mut h, on_stroke);
        let stroke = &rectangle(h.state()).strokes[0];
        assert_eq!((stroke.width, &stroke.paint), (4.0, &Paint::Solid([0.0, 1.0, 0.0, 1.0])));

        // Each click was one undo step.
        for _ in 0..2 {
            h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
            h.step();
        }
        assert_eq!(*rectangle(h.state()), drawn);
    }

    #[test]
    fn paints_inside_a_symbol_edited_in_place() {
        let mut h = blank();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        h.state_mut().tool = Tool::Selection;
        mouse_click(&mut h, (150.0, 125.0));
        h.key_press(Key::F8);
        h.step();
        h.step();
        h.key_press(Key::Enter);
        h.step();
        let instance = h.state().selection[0];
        h.state_mut().enter_symbol(instance);

        // The symbol's origin is the rectangle's top left, so stage point (150, 125) is (50, 25) inside it.
        (h.state_mut().tool, h.state_mut().fill_color) = (Tool::PaintBucket, Color32::RED);
        mouse_click(&mut h, (150.0, 125.0));
        assert_eq!(rectangle(h.state()).fills, [RED]);
        assert_eq!(h.state().timeline().name, "Symbol 1");
    }

    #[test]
    fn a_press_that_moves_or_lingers_still_paints() {
        let mut h = blank();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        (h.state_mut().tool, h.state_mut().fill_color) = (Tool::PaintBucket, Color32::RED);
        // Moved further than a click may: egui calls it a drag, not a click.
        mouse_drag(&mut h, (120.0, 120.0), (140.0, 130.0));
        assert_eq!(rectangle(h.state()).fills, [RED]);

        // Held longer than a click may.
        h.state_mut().fill_color = Color32::GREEN;
        let p = on_stage(&h, (150.0, 125.0));
        h.hover_at(p);
        h.step();
        h.drag_at(p);
        for _ in 0..60 {
            h.step();
        }
        h.drop_at(p);
        h.step();
        let green = [Paint::Solid([0.0, 1.0, 0.0, 1.0])];
        assert_eq!(rectangle(h.state()).fills, green);

        // A right-button drag doesn't paint, as a right click doesn't.
        h.state_mut().fill_color = Color32::BLUE;
        let (a, b) = (on_stage(&h, (120.0, 120.0)), on_stage(&h, (140.0, 130.0)));
        let right = |pressed, pos| egui::Event::PointerButton { pos, button: PointerButton::Secondary, pressed, modifiers: Modifiers::NONE };
        h.event(right(true, a));
        h.step();
        h.hover_at(b);
        h.step();
        h.event(right(false, b));
        h.step();
        assert_eq!(rectangle(h.state()).fills, green);
    }

    #[test]
    fn dragging_the_bucket_fills_the_areas_it_crosses() {
        let mut h = blank();
        h.state_mut().tool = Tool::Rectangle;
        // Two rectangles in the default blue, then a green one.
        mouse_drag(&mut h, (50.0, 50.0), (100.0, 100.0));
        mouse_drag(&mut h, (150.0, 50.0), (200.0, 100.0));
        h.state_mut().fill_color = Color32::GREEN;
        mouse_drag(&mut h, (250.0, 50.0), (300.0, 100.0));
        let fills = |h: &Harness<'_, Editor>| -> Vec<Paint> {
            let elements = &h.state().timeline().layers[0].frames[0].elements;
            elements.iter().map(|el| match el {
                Element::Shape(s) => s.fills[0].clone(),
                _ => panic!("not a shape"),
            }).collect()
        };
        let before = fills(&h);
        let green = before[2].clone();

        // As in Animate, a drag fills the areas filled like the first one.
        (h.state_mut().tool, h.state_mut().fill_color) = (Tool::PaintBucket, Color32::RED);
        mouse_path(&mut h, &[(75.0, 75.0), (125.0, 75.0), (175.0, 75.0), (225.0, 75.0), (275.0, 75.0)]);
        assert_eq!(fills(&h), [RED, RED, green.clone()]);
        // The whole drag is one undo step.
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.step();
        assert_eq!(fills(&h), before);

        // With Fill all regions, every area it crosses.
        h.state_mut().paint.fill_all = true;
        mouse_path(&mut h, &[(75.0, 75.0), (175.0, 75.0), (275.0, 75.0)]);
        assert_eq!(fills(&h), [RED, RED, RED]);
    }

    #[test]
    fn gradients_pour_into_each_area_or_lock_across_them() {
        use minnamate_core::doc::{Gradient, GradientKind, Matrix};
        let mut h = blank();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        mouse_drag(&mut h, (300.0, 100.0), (400.0, 200.0));
        let gradient = |h: &Harness<'_, Editor>, i: usize| -> Gradient {
            match &h.state().timeline().layers[0].frames[0].elements[i] {
                Element::Shape(s) => match &s.fills[0] {
                    Paint::Gradient(g) => g.clone(),
                    other => panic!("{other:?}"),
                },
                other => panic!("{other:?}"),
            }
        };
        let fit = |(x0, y0): (f32, f32), (x1, y1): (f32, f32)| {
            let mut g = Gradient::new(GradientKind::Linear);
            g.fit(minnamate_core::hit::Rect { min: (x0, y0), max: (x1, y1) });
            g.matrix
        };
        // Each area takes the gradient across itself.
        (h.state_mut().tool, h.state_mut().fill_gradient) = (Tool::PaintBucket, Some(Gradient::new(GradientKind::Linear)));
        mouse_click(&mut h, (150.0, 125.0));
        mouse_click(&mut h, (350.0, 150.0));
        assert_eq!((gradient(&h, 0).matrix, gradient(&h, 1).matrix), (fit((100.0, 100.0), (200.0, 150.0)), fit((300.0, 100.0), (400.0, 200.0))));
        // With Lock Fill, where the last fill left it: one gradient across both.
        h.state_mut().paint.lock_fill = true;
        mouse_click(&mut h, (150.0, 125.0));
        assert_eq!(gradient(&h, 0).matrix, gradient(&h, 1).matrix);

        // The Eyedropper picks a gradient up where it lies on stage (both shapes moved), for the bucket to go on with.
        let ctx = h.ctx.clone();
        h.state_mut().run(crate::app::Action::SelectAll, &ctx);
        h.state_mut().nudge(10.0, 0.0, "test");
        (h.state_mut().tool, h.state_mut().fill_gradient, h.state_mut().paint.lock_fill) = (Tool::Eyedropper, None, false);
        mouse_click(&mut h, (360.0, 150.0));
        let ed = h.state();
        assert_eq!((ed.tool, ed.paint.lock_fill), (Tool::PaintBucket, true));
        assert_eq!(ed.fill_gradient.as_ref().map(|g| g.matrix), Some(Matrix::translate(10.0, 0.0) * gradient(&h, 1).matrix));

        // The Ink Bottle's gradient spans the shape it outlines.
        (h.state_mut().tool, h.state_mut().stroke_gradient) = (Tool::InkBottle, Some(Gradient::new(GradientKind::Linear)));
        mouse_click(&mut h, (110.0, 100.0));
        let Element::Shape(s) = &h.state().timeline().layers[0].frames[0].elements[0] else { panic!("not a shape") };
        let Paint::Gradient(stroke) = &s.strokes[0].paint else { panic!("{:?}", s.strokes) };
        assert_eq!(stroke.matrix, fit((100.0, 100.0), (200.0, 150.0)));
    }

    #[test]
    fn switching_to_a_paint_tool_mid_drag_drops_the_marquee() {
        let mut h = blank();
        let (a, b) = (on_stage(&h, (50.0, 50.0)), on_stage(&h, (150.0, 150.0)));
        h.hover_at(a);
        h.step();
        h.drag_at(a);
        h.step();
        h.hover_at(b);
        h.step();
        assert!(matches!(h.state().drag, Some(crate::stage::Drag::Marquee { .. })));
        h.key_press(Key::K);
        h.step();
        h.drop_at(b);
        h.step();
        assert_eq!(h.state().tool, Tool::PaintBucket);
        assert!(h.state().drag.is_none(), "the Selection tool's marquee would stay on screen");
    }

    /// Writes target/screenshots/paint_tools.png: Paint Bucket options in Properties.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn paint_tools_screenshot() {
        let builder = Harness::builder().with_size(vec2(1600.0, 900.0)).with_step_dt(1.0 / 60.0).wgpu();
        let mut h = builder.build_eframe(|cc| Editor::new(&cc.egui_ctx, None));
        h.state_mut().new_document(550.0, 400.0, 24.0);
        h.run();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (300.0, 250.0));
        (h.state_mut().tool, h.state_mut().props_tab, h.state_mut().fill_color) = (Tool::PaintBucket, PropsTab::Tool, Color32::RED);
        mouse_click(&mut h, (200.0, 175.0));
        h.hover_at(on_stage(&h, (350.0, 200.0)));
        h.run();
        screenshot(&mut h, "paint_tools");
        // The Eyedropper over the dark pasteboard, where the cursor's white rim matters.
        h.state_mut().tool = Tool::Eyedropper;
        h.hover_at(on_stage(&h, (-60.0, 200.0)));
        h.run();
        screenshot(&mut h, "paint_tools_eyedropper");
    }
}
