//! Eraser tool (E): rubs out fills and strokes of drawn shapes, never symbols,
//! groups or text. As in Animate, a drag paints the stage color over what it
//! sweeps and erases on release, on every layer unless "Erase on active layer"
//! is on.

use eframe::egui::{
    self, Color32, CursorIcon, Painter, PointerButton, Rect, Response, Stroke, StrokeKind, Ui, Vec2,
};
use minnamate_core::doc::{Element, LayerKind, Point, Shape};
use minnamate_core::erase::{self, Area, Target, Tip};
use minnamate_core::hit::ElementRef;

use crate::app::Editor;
use crate::stage::{local_point, screen_point};
use crate::theme;
use crate::widgets;

/// Animate's eraser modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Fills,
    Lines,
    SelectedFills,
    Inside,
}

impl Mode {
    pub(crate) const ALL: [Mode; 5] = [Mode::Normal, Mode::Fills, Mode::Lines, Mode::SelectedFills, Mode::Inside];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Mode::Normal => "Erase Normal",
            Mode::Fills => "Erase Fills",
            Mode::Lines => "Erase Lines",
            Mode::SelectedFills => "Erase Selected Fills",
            Mode::Inside => "Erase Inside",
        }
    }
}

/// What the tool remembers between frames.
pub struct State {
    pub mode: Mode,
    pub tip: Tip,
    /// Across, in stage pixels: it zooms with the stage, and keeps its size on
    /// stage inside a scaled symbol.
    pub size: f32,
    /// Click to delete a whole stroke or connected fill instead.
    pub faucet: bool,
    /// Erase on the selected layer only.
    pub active_layer: bool,
    /// The pointer's path during a drag, in the edited timeline's coordinates.
    points: Vec<Point>,
}

impl Default for State {
    fn default() -> Self {
        State { mode: Mode::Normal, tip: Tip::Round, size: 10.0, faucet: false, active_layer: false, points: Vec::new() }
    }
}

/// Pointer input on the stage while this tool is active. `stage` is the
/// stage's screen rectangle.
pub fn input(ui: &Ui, ed: &mut Editor, resp: &Response, stage: Rect) {
    let pointer = resp.interact_pointer_pos().map(|pos| local_point(ed, stage, pos));
    // A drag that another tool took over leaves no sweep behind.
    if !resp.dragged() && !resp.drag_stopped() {
        ed.eraser.points.clear();
    }
    if ed.eraser.faucet {
        if resp.clicked()
            && let Some(p) = pointer
        {
            faucet(ed, p);
        }
        return;
    }
    // The eraser's outline is the cursor; `overlay` draws it.
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::None);
    }
    // Only the primary button erases, as a click does.
    let primary = PointerButton::Primary;
    if resp.drag_started_by(primary)
        && let Some(origin) = ui.input(|i| i.pointer.press_origin())
    {
        ed.eraser.points = vec![local_point(ed, stage, origin)];
    }
    if resp.dragged_by(primary)
        && let Some(p) = pointer
        && ed.eraser.points.last() != Some(&p)
    {
        ed.eraser.points.push(p);
    }
    if resp.drag_stopped_by(primary) {
        let mut points = std::mem::take(&mut ed.eraser.points);
        // The last stretch of a quick drag arrives with the release.
        if let Some(p) = pointer
            && points.last() != Some(&p)
        {
            points.push(p);
        }
        apply(ed, &points);
    } else if resp.clicked()
        && let Some(p) = pointer
    {
        apply(ed, &[p]);
    }
}

/// Feedback drawn over the stage (screen space).
pub fn overlay(p: &Painter, ed: &Editor, stage: Rect) {
    let s = &ed.eraser;
    // What the drag has swept so far takes the stage color until it's erased on release.
    if p.ctx().input(|i| i.pointer.primary_down()) {
        let [r, g, b, a] = ed.doc.background.map(|c| (c * 255.0).round() as u8);
        let (color, on_stage) = (Color32::from_rgba_unmultiplied(r, g, b, a), p.with_clip_rect(stage));
        for poly in swept(ed, &s.points) {
            let corners = poly.into_iter().map(|q| screen_point(ed, stage, q)).collect();
            on_stage.add(egui::Shape::convex_polygon(corners, color, Stroke::NONE));
        }
    }
    let Some(pos) = p.ctx().pointer_hover_pos().filter(|&q| p.clip_rect().contains(q)) else { return };
    if s.faucet {
        return;
    }
    // A black outline with a white halo, to show on any color.
    let radius = s.size * ed.zoom / 2.0;
    for stroke in [Stroke::new(3.0, Color32::from_white_alpha(160)), Stroke::new(1.0, Color32::BLACK)] {
        match s.tip {
            Tip::Round => p.circle_stroke(pos, radius, stroke),
            Tip::Square => p.rect_stroke(Rect::from_center_size(pos, Vec2::splat(2.0 * radius)), 0.0, stroke, StrokeKind::Middle),
        };
    }
}

/// The tool's section of Properties > Tool.
pub fn options(ui: &mut Ui, ed: &mut Editor) {
    let s = &mut ed.eraser;
    theme::section(ui, "Eraser Options", |ui| {
        widgets::row(ui, |ui| {
            widgets::label(ui, "Mode", widgets::WORD_W);
            let modes: Vec<_> = Mode::ALL.iter().map(|&m| (m, m.name(), true)).collect();
            if let Some(mode) = widgets::pick(ui, "Eraser mode", s.mode, &modes, ui.available_width() - 4.0) {
                s.mode = mode;
            }
        });
        widgets::row(ui, |ui| {
            widgets::label(ui, "Shape", widgets::WORD_W);
            let round = theme::icon_button(ui, theme::ICON_BUTTON, s.tip == Tip::Round, true, "Round", |p, r, c| {
                p.circle_filled(r.center(), 5.0, c);
            });
            let square = theme::icon_button(ui, theme::ICON_BUTTON, s.tip == Tip::Square, true, "Square", |p, r, c| {
                p.rect_filled(Rect::from_center_size(r.center(), Vec2::splat(9.0)), 0.0, c);
            });
            if round.clicked() {
                s.tip = Tip::Round;
            }
            if square.clicked() {
                s.tip = Tip::Square;
            }
        });
        widgets::row(ui, |ui| {
            widgets::label(ui, "Size", widgets::WORD_W);
            ui.add(widgets::Number::new("Size", &mut s.size, 1.0..=200.0).speed(0.5));
        });
        if widgets::slider(ui, "Size slider", &mut s.size, 1.0..=200.0).changed() {
            s.size = s.size.round();
        }
        widgets::row(ui, |ui| widgets::checkbox(ui, &mut s.faucet, "Faucet").on_hover_text("Click a stroke or fill to delete all of it"));
        widgets::row(ui, |ui| widgets::checkbox(ui, &mut s.active_layer, "Erase on active layer"));
    });
}

/// What a drag along `points` sweeps, both in the edited timeline's
/// coordinates. The tip is shaped on stage, so inside a scaled or rotated
/// symbol it still covers what its outline does.
fn swept(ed: &Editor, points: &[Point]) -> Vec<Vec<Point>> {
    let on_stage: Vec<Point> = points.iter().map(|&p| ed.to_scene(p)).collect();
    let mut polys = erase::sweep(&on_stage, ed.eraser.tip, ed.eraser.size);
    polys.iter_mut().flatten().for_each(|p| *p = ed.to_local(*p));
    polys
}

/// Erases what a drag along `points` swept: one undo step.
fn apply(ed: &mut Editor, points: &[Point]) {
    let Some(&start) = points.first() else { return };
    let (mode, area) = (ed.eraser.mode, Area::new(&swept(ed, points)));
    let target = match mode {
        Mode::Normal => Target::All,
        Mode::Fills | Mode::SelectedFills => Target::Fills,
        Mode::Lines => Target::Strokes,
        Mode::Inside => Target::FillAt(start),
    };
    let mut erased = Vec::new();
    for (r, mut shape) in targets(ed, mode == Mode::SelectedFills) {
        if erase::erase(&mut shape, &area, target) {
            erased.push((r, shape));
            // Erase Inside keeps to the one fill it started in.
            if mode == Mode::Inside {
                break;
            }
        }
    }
    commit(ed, erased);
}

/// The Faucet: a click deletes the stroke or connected fill under it.
fn faucet(ed: &mut Editor, p: Point) {
    // A few screen pixels of slack, measured in the edited timeline, which may be a scaled symbol.
    let (a, b) = (ed.to_local((0.0, 0.0)), ed.to_local((3.0 / ed.zoom, 0.0)));
    let tolerance = (b.0 - a.0).hypot(b.1 - a.1);
    let hit = targets(ed, false).into_iter().find_map(|(r, mut s)| erase::faucet(&mut s, p, tolerance).then_some((r, s)));
    commit(ed, hit.into_iter().collect());
}

/// Copies of the drawn shapes the eraser reaches at the playhead, topmost first.
fn targets(ed: &Editor, selected_only: bool) -> Vec<(ElementRef, Shape)> {
    let layers = &ed.timeline().layers;
    // Shown, unlocked normal layers: guides and masks don't show on stage.
    let reached = |l: usize| {
        let layer = &layers[l];
        layer.kind == LayerKind::Normal && layer.visible() && !layer.locked() && (l == ed.layer || !ed.eraser.active_layer)
    };
    let shape = |r: &ElementRef| match &layers[r.layer].frames[r.keyframe].elements[r.element] {
        Element::Shape(s) => Some(s.clone()),
        _ => None,
    };
    let all = ed.all_elements().into_iter().rev();
    all.filter(|r| reached(r.layer) && (!selected_only || ed.selection.contains(r))).filter_map(|r| Some((r, shape(&r)?))).collect()
}

/// Writes erased shapes back as one undo step, deleting those erased away.
fn commit(ed: &mut Editor, erased: Vec<(ElementRef, Shape)>) {
    if erased.is_empty() {
        return;
    }
    ed.checkpoint();
    let mut gone = Vec::new();
    for (r, shape) in erased {
        if shape.edges.is_empty() {
            gone.push(r);
        }
        if let Some(el) = ed.element_mut(r) {
            *el = Element::Shape(shape);
        }
    }
    // Highest index first, so earlier removals don't shift later ones. The selection follows.
    gone.sort_by_key(|r| std::cmp::Reverse(r.element));
    for r in &gone {
        ed.timeline_mut().layers[r.layer].frames[r.keyframe].elements.remove(r.element);
    }
    ed.selection.retain(|r| !gone.contains(r));
    for r in &mut ed.selection {
        r.element -= gone.iter().filter(|g| (g.layer, g.keyframe) == (r.layer, r.keyframe) && g.element < r.element).count();
    }
}

#[cfg(test)]
mod tests {
    use minnamate_core::doc::SymbolKind;
    use minnamate_core::{hit, symbols};

    use super::*;
    use crate::app::testkit::*;
    use crate::tools::Tool;

    /// Is anything drawn within a pixel of `p`?
    fn ink_at(h: &Harness<'_, Editor>, p: Point) -> bool {
        let ed = h.state();
        hit::hit_test(&ed.doc, ed.timeline(), ed.frame, p, 1.0).is_some()
    }

    fn undo(h: &mut Harness<'_, Editor>) {
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
        h.step();
    }

    #[test]
    fn erase_undo_and_faucet() {
        let mut h = blank();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        h.key_press(egui::Key::E);
        h.step();
        assert_eq!(h.state().tool, Tool::Eraser);
        h.state_mut().eraser.size = 20.0;

        // Down through the middle: a 20 px stripe, outline and fill.
        mouse_drag(&mut h, (150.0, 80.0), (150.0, 170.0));
        assert!(!ink_at(&h, (150.0, 125.0)) && !ink_at(&h, (150.0, 100.0)), "erased");
        assert!(ink_at(&h, (130.0, 125.0)) && ink_at(&h, (170.0, 100.0)), "either side stays");
        assert_eq!(h.state().all_elements().len(), 1);
        undo(&mut h);
        assert!(ink_at(&h, (150.0, 125.0)), "one undo step puts it back");
        assert_eq!(h.state().all_elements().len(), 1);

        // A click erases a dot.
        mouse_click(&mut h, (150.0, 125.0));
        assert!(!ink_at(&h, (150.0, 125.0)) && ink_at(&h, (150.0, 138.0)));

        // The faucet deletes the fill around the dot, then the whole outline.
        h.state_mut().eraser.faucet = true;
        mouse_click(&mut h, (120.0, 125.0));
        assert!(!ink_at(&h, (120.0, 125.0)) && ink_at(&h, (100.0, 125.0)), "the fill goes, the outline stays");
        mouse_click(&mut h, (100.0, 125.0));
        assert!(h.state().all_elements().is_empty(), "nothing left: the shape is gone");
    }

    #[test]
    fn layers_selection_and_modes() {
        let mut h = blank();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        // A second rectangle on a new layer above, which becomes the selected one.
        h.state_mut().new_layer();
        mouse_drag(&mut h, (100.0, 200.0), (200.0, 250.0));
        h.state_mut().tool = Tool::Eraser;
        h.state_mut().eraser.size = 20.0;
        let across = |h: &mut Harness<'_, Editor>| mouse_drag(h, (150.0, 80.0), (150.0, 270.0));

        across(&mut h);
        assert!(!ink_at(&h, (150.0, 125.0)) && !ink_at(&h, (150.0, 225.0)), "every layer, as in Animate");
        undo(&mut h);

        h.state_mut().eraser.active_layer = true;
        across(&mut h);
        assert!(ink_at(&h, (150.0, 125.0)) && !ink_at(&h, (150.0, 225.0)), "only the active layer");
        undo(&mut h);
        h.state_mut().eraser.active_layer = false;

        // Erase Selected Fills: the lower rectangle's fill only.
        h.state_mut().tool = Tool::Selection;
        mouse_click(&mut h, (120.0, 125.0));
        assert_eq!(h.state().selection.len(), 1);
        h.state_mut().tool = Tool::Eraser;
        h.state_mut().eraser.mode = Mode::SelectedFills;
        across(&mut h);
        assert!(!ink_at(&h, (150.0, 125.0)) && ink_at(&h, (150.0, 100.0)), "its fill, not its outline");
        assert!(ink_at(&h, (150.0, 225.0)), "not the other rectangle");

        // Erasing a whole shape deletes it, and the selection follows what it held.
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 300.0), (200.0, 330.0));
        h.state_mut().tool = Tool::Selection;
        mouse_click(&mut h, (150.0, 315.0));
        assert_eq!(h.state().selection, [ElementRef { layer: 1, keyframe: 0, element: 1 }]);
        h.state_mut().tool = Tool::Eraser;
        h.state_mut().eraser.mode = Mode::Normal;
        h.state_mut().eraser.size = 120.0;
        mouse_click(&mut h, (150.0, 125.0));
        let ed = h.state();
        assert_eq!(ed.all_elements().len(), 2, "the lower rectangle is gone");
        assert_eq!(ed.selection, [ElementRef { layer: 1, keyframe: 0, element: 0 }]);
        assert_eq!(hit::hit_test(&ed.doc, ed.timeline(), ed.frame, (150.0, 315.0), 0.0), Some(ed.selection[0]));
    }

    #[test]
    fn a_quick_flick_erases_up_to_the_release() {
        let mut h = blank();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (300.0, 150.0));
        h.state_mut().tool = Tool::Eraser;
        // The last stretch of a fast drag arrives in the same frame as the release.
        let (a, b, c) = (on_stage(&h, (80.0, 125.0)), on_stage(&h, (150.0, 125.0)), on_stage(&h, (280.0, 125.0)));
        h.hover_at(a);
        h.step();
        h.drag_at(a);
        h.step();
        h.hover_at(b);
        h.step();
        h.drop_at(c);
        h.step();
        assert!(!ink_at(&h, (150.0, 125.0)) && !ink_at(&h, (250.0, 125.0)));
        assert!(ink_at(&h, (295.0, 125.0)), "the release point is the end");
    }

    #[test]
    fn right_button_drags_erase_nothing() {
        let mut h = blank();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        h.state_mut().tool = Tool::Eraser;
        let (a, b) = (on_stage(&h, (150.0, 80.0)), on_stage(&h, (150.0, 170.0)));
        let right = |pos, pressed| {
            egui::Event::PointerButton { pos, button: PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE }
        };
        h.hover_at(a);
        h.step();
        h.event(right(a, true));
        h.step();
        h.hover_at(b);
        h.step();
        h.event(right(b, false));
        h.step();
        assert!(ink_at(&h, (150.0, 125.0)) && ink_at(&h, (150.0, 100.0)));
    }

    #[test]
    fn faucet_slack_is_on_screen_inside_a_scaled_symbol() {
        let mut h = blank();
        let ed = h.state_mut();
        let line = minnamate_core::draw::line((100.0, 100.0), (300.0, 100.0), ed.stroke());
        ed.add_shape(line);
        let all = ed.all_elements();
        let r = symbols::convert_to_symbol(&mut ed.doc, 0, 0, &all, "Line", SymbolKind::Graphic, (100.0, 100.0)).unwrap();
        // A quarter size: on stage the line runs from (100, 100) to (150, 100).
        if let Some(Element::Instance(i)) = ed.element_mut(r) {
            (i.matrix.a, i.matrix.d) = (0.25, 0.25);
        }
        ed.enter_symbol(r);
        ed.tool = Tool::Eraser;
        ed.eraser.faucet = true;
        // Two and a half screen pixels below it, within the usual few pixels of slack.
        let below = 100.0 + 2.5 / h.state().zoom;
        mouse_click(&mut h, (125.0, below));
        assert!(h.state().all_elements().is_empty(), "the faucet took the line");
    }

    #[test]
    fn an_interrupted_drag_leaves_no_preview_behind() {
        let mut h = blank();
        h.state_mut().tool = Tool::Eraser;
        let (a, b) = (on_stage(&h, (100.0, 100.0)), on_stage(&h, (200.0, 100.0)));
        h.hover_at(a);
        h.step();
        h.drag_at(a);
        h.step();
        h.hover_at(b);
        h.step();
        // V picks the Selection tool mid-drag, so the eraser never sees the release.
        h.key_press(egui::Key::V);
        h.step();
        h.drop_at(b);
        h.step();
        assert_eq!(h.state().tool, Tool::Selection);

        // Back to the eraser: pressing again shows none of the old sweep.
        h.state_mut().tool = Tool::Eraser;
        h.hover_at(a);
        h.step();
        h.drag_at(a);
        h.step();
        assert!(swept(h.state(), &h.state().eraser.points).is_empty());
        h.drop_at(a);
        h.step();
    }

    #[test]
    fn erases_inside_a_symbol_edited_in_place() {
        let mut h = blank();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        let ed = h.state_mut();
        let all = ed.all_elements();
        let r = symbols::convert_to_symbol(&mut ed.doc, 0, 0, &all, "Box", SymbolKind::Graphic, (100.0, 100.0)).unwrap();
        // Doubled from its registration point: on stage the box covers 100..300 × 100..200.
        if let Some(Element::Instance(i)) = ed.element_mut(r) {
            (i.matrix.a, i.matrix.d) = (2.0, 2.0);
        }
        ed.enter_symbol(r);
        ed.tool = Tool::Eraser;
        ed.eraser.size = 20.0;

        // 20 px across on stage is 10 px across in the symbol, around its (50, 25).
        mouse_click(&mut h, (200.0, 150.0));
        assert!(!ink_at(&h, (50.0, 25.0)) && !ink_at(&h, (50.0, 21.0)), "erased");
        assert!(ink_at(&h, (50.0, 18.0)) && ink_at(&h, (40.0, 25.0)), "only within the eraser's outline");

        // The faucet takes the fill, then the outline: the emptied shape leaves the symbol, not the scene.
        h.state_mut().eraser.faucet = true;
        mouse_click(&mut h, (120.0, 120.0));
        assert!(!ink_at(&h, (10.0, 10.0)) && ink_at(&h, (0.0, 25.0)));
        mouse_click(&mut h, (100.0, 150.0));
        let ed = h.state();
        assert!(ed.all_elements().is_empty());
        assert_eq!(ed.doc.scenes[0].layers[0].frames[0].elements.len(), 1, "the instance stays");
    }

    /// Writes target/screenshots/eraser.png: a stripe erased, another being dragged.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn eraser_screenshot() {
        let mut h = Harness::builder().with_size(egui::vec2(1600.0, 900.0)).wgpu().build_eframe(|cc| Editor::new(&cc.egui_ctx, None));
        h.state_mut().new_document(550.0, 400.0, 24.0);
        h.run();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (300.0, 250.0));
        h.state_mut().tool = Tool::Oval;
        mouse_drag(&mut h, (250.0, 150.0), (450.0, 300.0));
        h.state_mut().tool = Tool::Eraser;
        h.state_mut().eraser.size = 24.0;
        h.state_mut().props_tab = crate::app::PropsTab::Tool;
        mouse_drag(&mut h, (80.0, 180.0), (470.0, 180.0));
        // Mid-drag: the swept area shows in the stage color.
        let (a, b) = (on_stage(&h, (200.0, 60.0)), on_stage(&h, (300.0, 320.0)));
        h.hover_at(a);
        h.step();
        h.drag_at(a);
        h.step();
        h.hover_at(a.lerp(b, 0.5));
        h.step();
        h.hover_at(b);
        h.step();
        screenshot(&mut h, "eraser");
    }
}
