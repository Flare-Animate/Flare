//! The stage: edit bar on top, then the document on its pasteboard, where the
//! tools select, move and draw; rulers, grid, guides and scrollbars around it.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

use eframe::egui::{
    self, Align, Color32, CursorIcon, FontId, Key, Layout, Margin, Modifiers, Painter, Pos2, Rect, RichText, Sense, Stroke,
    StrokeKind, Ui, emath::GuiRounding, pos2, vec2,
};
use minnamate_core::doc::{self, Element, Frame, Instance, Layer, LayerKind, Matrix, Paint, Point, Seg, Shape, TextKind, Timeline, Tween};
use minnamate_core::hit::{self, ElementRef, Grip};
use minnamate_core::render::render_timeline;
use minnamate_core::tiny_skia as sk;
use minnamate_core::{draw, text};

use crate::app::{Action, Editor, PropsTab, ViewPort, hash};
use crate::cursors::{self, Cursor};
use crate::icons::{self, Icon};
use crate::tools::Tool;
use crate::{eraser, free_transform, paint_tools, pen, text_tool, theme};

/// The zoom field's presets, which the zoom-step arrows walk through.
const ZOOMS: [f32; 6] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0];
/// The rulers' depth, in points.
const RULER: f32 = 18.0;
/// The scrollbar thumbs' thickness, in points.
const THUMB: f32 = 7.0;
/// Grid spacing in stage pixels, Animate's default.
const GRID_STEP: f32 = 10.0;

/// A mouse gesture on the stage, in document coordinates.
pub enum Drag {
    /// Selection tool, dragging the selection (or, with Alt, a copy of it) by
    /// `moved` so far since `start`.
    Move { start: Point, moved: Point, copy: bool },
    /// Selection tool, bending an edge of an unselected shape or moving one of
    /// its corners: `original` is the shape before the drag.
    Reshape { r: ElementRef, original: Shape, grip: Grip, start: Point },
    /// Selection tool, dragging a selection rectangle.
    Marquee { start: Point, end: Point },
    /// Rectangle, Oval, Line or PolyStar tool.
    Shape { start: Point, end: Point },
    /// Pencil or Brush tool: the pointer's path.
    Stroke { points: Vec<Point> },
    /// Lasso tool: the loop drawn so far.
    Lasso { points: Vec<Point> },
}

/// View > Rulers, Grid, Guides and Show Shape Hints.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Toggle {
    Rulers,
    Grid,
    Guides,
    LockGuides,
    ShapeHints,
}

/// A ruler guide, in stage pixels from the stage's top-left corner.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Guide {
    /// A vertical line at this x.
    X(f32),
    /// A horizontal line at this y.
    Y(f32),
}

/// The stage view's own settings: rulers, grid and guides.
pub struct State {
    pub rulers: bool,
    pub grid: bool,
    pub guides_shown: bool,
    pub guides_locked: bool,
    pub shape_hints: bool,
    // ponytail: guides belong to the view, not the document, so they aren't saved or undone.
    // Animate keeps them in the document; move them there when the XFL writer learns them.
    pub guides: Vec<Guide>,
    /// The guide being dragged and where it is now: a new one out of a ruler
    /// (None) or `guides[i]` being moved.
    dragging: Option<(Option<usize>, Guide)>,
}

impl Default for State {
    fn default() -> Self {
        State { rulers: false, grid: false, guides_shown: true, guides_locked: false, shape_hints: true, guides: Vec::new(), dragging: None }
    }
}

impl State {
    pub fn on(&self, t: Toggle) -> bool {
        match t {
            Toggle::Rulers => self.rulers,
            Toggle::Grid => self.grid,
            Toggle::Guides => self.guides_shown,
            Toggle::LockGuides => self.guides_locked,
            Toggle::ShapeHints => self.shape_hints,
        }
    }

    pub fn toggle(&mut self, t: Toggle) {
        let flag = match t {
            Toggle::Rulers => &mut self.rulers,
            Toggle::Grid => &mut self.grid,
            Toggle::Guides => &mut self.guides_shown,
            Toggle::LockGuides => &mut self.guides_locked,
            Toggle::ShapeHints => &mut self.shape_hints,
        };
        *flag = !*flag;
    }
}

pub fn panel(ui: &mut Ui, ed: &mut Editor) {
    edit_bar(ui, ed);
    ed.prune_selection();

    // The rulers, when shown, take a strip off the top and left of the view.
    let full = ui.available_rect_before_wrap();
    let inset = if ed.stage.rulers { RULER } else { 0.0 };
    let rect = Rect::from_min_max(full.min + vec2(inset, inset), full.max);
    let resp = ui.allocate_rect(rect, Sense::click_and_drag());
    ed.view_size = rect.size();
    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point();

    // Navigation: wheel pans, Ctrl+wheel or pinch zooms, Hand (or Space) drags, Zoom clicks.
    // Space types a space while a text field has the keyboard.
    let typing = ctx.egui_wants_keyboard_input();
    let (scroll, zoom, pointer, shift, alt, space) = ui.input(|i| {
        (i.smooth_scroll_delta, i.zoom_delta(), i.pointer.hover_pos(), i.modifiers.shift, i.modifiers.alt, i.key_down(Key::Space) && !typing)
    });
    if resp.hovered() {
        if zoom != 1.0 {
            zoom_at(ed, rect, ed.zoom * zoom, pointer);
        } else {
            ed.pan += scroll;
        }
    }
    let hand = ed.tool == Tool::Hand || space;
    if hand && resp.dragged() {
        ed.pan += resp.drag_delta();
    }
    if ed.tool == Tool::Zoom && resp.clicked() {
        let target = if alt { ed.zoom / 2.0 } else { ed.zoom * 2.0 };
        zoom_at(ed, rect, target, resp.interact_pointer_pos());
    }
    // The scrollbars pan too.
    for (i, (thumb, rate)) in thumbs(ed, rect, ppp).into_iter().enumerate() {
        let bar = ui.interact(thumb, ui.id().with(("scrollbar", i)), Sense::drag());
        if bar.dragged() {
            let d = bar.drag_delta() * rate;
            if i == 0 {
                ed.pan.y -= d.y;
            } else {
                ed.pan.x -= d.x;
            }
        }
    }

    let stage = stage_rect(ed, rect, ppp);
    ed.stage_rect = stage;
    let over_guide = guides_input(ui, ed, &resp, full, rect, stage);
    // During a rectangle drag the Up and Down arrows round its corners, a pixel a press.
    if matches!(ed.drag, Some(Drag::Shape { .. })) && ed.tool == Tool::Rectangle {
        let (up, down) = ui.input_mut(|i| (i.count_and_consume_key(Modifiers::NONE, Key::ArrowUp), i.count_and_consume_key(Modifiers::NONE, Key::ArrowDown)));
        if up + down > 0 {
            ed.corner_radius = (ed.corner_radius + up as f32 - down as f32).clamp(-100.0, 999.0);
        }
    }
    if resp.hovered() {
        pointer_glyph(&ctx, ed, &resp, stage, hand, alt, over_guide);
    }

    // Choosing another tool ends a Pen path.
    if ed.tool != Tool::Pen {
        pen::finish(ed, false);
    }
    // Another tool ends typing.
    if ed.tool != Tool::Text {
        text_tool::commit(ed);
    }
    if !hand && ed.tool != Tool::Zoom && ed.stage.dragging.is_none() {
        match ed.tool {
            Tool::FreeTransform => free_transform::input(ui, ed, &resp, stage),
            Tool::GradientTransform => crate::gradient_transform::input(ui, ed, &resp, stage),
            Tool::Pen => pen::input(ui, ed, &resp, stage),
            Tool::Text => text_tool::input(ui, ed, &resp, stage),
            Tool::Eraser => eraser::input(ui, ed, &resp, stage),
            Tool::PaintBucket | Tool::InkBottle | Tool::Eyedropper => paint_tools::input(ui, ed, &resp, stage),
            _ => tool_input(ui, ed, &resp, stage),
        }
    }

    // Everything renders into a canvas covering the visible view, so artwork off
    // the stage shows on the pasteboard unless "Clip content" is on.
    let canvas = ((rect.width() * ppp).ceil().clamp(1.0, 4096.0), (rect.height() * ppp).ceil().clamp(1.0, 4096.0));
    let s = ed.zoom * ppp;
    let vp = ViewPort {
        matrix: Matrix { a: s, d: s, tx: (stage.left() - rect.left()) * ppp, ty: (stage.top() - rect.top()) * ppp, ..Matrix::IDENTITY },
        size: canvas,
    };
    let view = Rect::from_min_size(rect.min, vec2(canvas.0, canvas.1) / ppp);
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    let p = ui.painter_at(rect);
    let [r, g, b, _] = ed.doc.background.map(|c| (c * 255.0) as u8);
    let stage_color = Color32::from_rgb(r, g, b);
    // A symbol edited on its own (from the Library) has no stage around it, just its color.
    let isolated = ed.editing.last().is_some_and(|t| t.isolated);
    // Document Settings' "Apply to pasteboard" gives the pasteboard the stage's colour.
    p.rect_filled(rect, 0.0, if ed.home.apply_to_pasteboard || isolated { stage_color } else { theme::PASTEBOARD });
    if !isolated {
        p.rect_filled(stage, 0.0, stage_color);
        if ed.stage.grid {
            grid(&p, stage, ed.zoom, ppp);
        }
    }
    let art = if ed.clip_to_stage && !isolated { p.with_clip_rect(stage) } else { p.clone() };
    // Editing a symbol in place: the rest of the stage shows at half strength
    // over the stage colour, the symbol live on top.
    if !ed.editing.is_empty() && !isolated && let Some(tex) = ed.scene_texture(&ctx, &vp) {
        art.image(tex, view, uv, Color32::from_white_alpha(128));
    }
    if ed.onion {
        // Onion skin: earlier frames tinted blue and later ones green, fainter
        // further away, each layer's ghosts under its own current frame.
        for layer in (0..ed.timeline().layers.len()).rev() {
            for (f, tint, opacity) in crate::timeline::onion_frames(ed, layer) {
                if let Some(tex) = ed.layer_texture(&ctx, layer, f, Some(tint), &vp) {
                    art.image(tex, view, uv, Color32::from_white_alpha((opacity * 255.0) as u8));
                }
            }
            if let Some(tex) = ed.layer_texture(&ctx, layer, ed.frame, None, &vp) {
                art.image(tex, view, uv, Color32::WHITE);
            }
        }
    } else {
        let tex = if ed.editing.is_empty() { ed.scene_texture(&ctx, &vp) } else { ed.symbol_texture(&ctx, &vp) };
        if let Some(tex) = tex {
            art.image(tex, view, uv, Color32::WHITE);
        }
    }
    // Animate outlines the stage so its edge shows whatever the colors.
    if !isolated {
        p.rect_stroke(stage, 0.0, Stroke::new(1.0, theme::STAGE_OUTLINE), StrokeKind::Outside);
    }
    if !ed.editing.is_empty() {
        // The edited symbol's origin: a 16 px crosshair.
        let origin = screen_point(ed, stage, (0.0, 0.0)).round_to_pixel_center(ppp);
        for (stroke, len) in [(Stroke::new(3.0, Color32::WHITE), 9.0), (Stroke::new(1.0, Color32::BLACK), 8.0)] {
            p.hline(origin.x - len..=origin.x + len, origin.y, stroke);
            p.vline(origin.x, origin.y - len..=origin.y + len, stroke);
        }
    }

    if let Some(tex) = dots(ed, &ctx, &vp) {
        art.image(tex, view, uv, Color32::WHITE);
    }
    crate::tweens::paint(&p, ed, stage);
    highlights(&p, ed, stage);
    // What a shape drag will make, in its real stroke and fill.
    if let Some(Drag::Shape { start, end }) = ed.drag
        && let Some(shape) = shape_for(ed, start, end, shift, alt)
    {
        let key = hash((format!("{shape:?}"), vp.bits()));
        if let Some(tex) = ed.canvas_texture(&ctx, "preview", key, |ed| render_elements(ed, vec![Element::Shape(shape)], &vp)) {
            art.image(tex, view, uv, Color32::WHITE);
        }
    }
    drag_feedback(&p, ed, stage);
    match ed.tool {
        Tool::FreeTransform => free_transform::overlay(&p, ed, stage),
        Tool::GradientTransform => crate::gradient_transform::overlay(&p, ed, stage),
        Tool::Pen => pen::overlay(&p, ed, stage),
        Tool::Text => text_tool::overlay(&p, ed, stage),
        Tool::Eraser => eraser::overlay(&p, ed, stage),
        Tool::PaintBucket | Tool::InkBottle | Tool::Eyedropper => paint_tools::overlay(&p, ed, stage),
        // The brush's tip, at its size and in the fill colour, is its pointer.
        Tool::Brush if !hand && ctx.output(|o| o.cursor_icon == CursorIcon::None) => {
            if let Some(pos) = pointer {
                p.circle_filled(pos, (ed.brush_size * ed.zoom / 2.0).max(1.0), crate::color::solid(&ed.fill()));
            }
        }
        _ => {}
    }
    paint_guides(&p, ed, stage, ppp);
    for (thumb, _) in thumbs(ed, rect, ppp) {
        p.rect_filled(thumb, THUMB / 2.0, theme::SCROLLBAR_THUMB);
    }
    if ed.stage.rulers {
        rulers(&ui.painter_at(full), ed, full, rect, stage, ppp);
    }
}

/// Selection, shape and stroke tools: click, drag and release on the stage.
/// Other tools can fall back to it for selecting.
pub fn tool_input(ui: &Ui, ed: &mut Editor, resp: &egui::Response, stage: Rect) {
    // A drag on a selected target's motion path is the path's.
    if crate::tweens::input(ui, ed, resp, stage) {
        return;
    }
    let (shift, alt) = ui.input(|i| (i.modifiers.shift, i.modifiers.alt));
    if resp.drag_started() {
        // Start where the button went down, not where the drag was recognized.
        if let Some(origin) = ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos()) {
            let p = local_point(ed, stage, origin);
            start(ed, p, shift, alt);
        }
    } else if resp.clicked()
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let p = local_point(ed, stage, pos);
        click(ed, p, shift);
        // A double-click is a click too; on an instance it then edits the
        // symbol in place, on text it types there with the Text tool, and on
        // empty stage it leaves the symbol being edited.
        if resp.double_clicked() && selecting(ed.tool) {
            match hit_at(ed, p) {
                Some(r) if matches!(ed.element(r), Some(Element::Instance(_))) => ed.enter_symbol(r),
                Some(r) if matches!(ed.element(r), Some(Element::Text(_))) => text_tool::edit_at(ed, r, p),
                None => ed.exit_symbol(),
                _ => {}
            }
        }
    }
    if resp.dragged()
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let p = local_point(ed, stage, pos);
        update(ed, p, shift);
    }
    if resp.drag_stopped() {
        finish(ed, shift, alt);
    }
}

/// The point under a screen position, in the coordinates of the timeline
/// being edited (the scene's, or the symbol's while editing one in place).
pub fn local_point(ed: &Editor, stage: Rect, pos: Pos2) -> Point {
    // ponytail: no stage rotation (the edit bar's Rotation tool): the stage fill, Clip
    // content and the Text tool's field assume an upright stage. Rotate here, in
    // screen_point and in the ViewPort matrix when they can follow.
    ed.to_local(((pos.x - stage.left()) / ed.zoom, (pos.y - stage.top()) / ed.zoom))
}

/// Where a point of the edited timeline shows on screen.
pub fn screen_point(ed: &Editor, stage: Rect, p: Point) -> Pos2 {
    let (x, y) = ed.to_scene(p);
    stage.min + vec2(x, y) * ed.zoom
}

fn selecting(tool: Tool) -> bool {
    matches!(tool, Tool::Selection | Tool::Subselection | Tool::FreeTransform)
}

/// The topmost element under `p`, with a few screen pixels of slack.
pub(crate) fn hit_at(ed: &Editor, p: Point) -> Option<ElementRef> {
    hit::hit_test(&ed.doc, ed.timeline(), ed.frame, p, 3.0 / ed.zoom)
}

/// Whether the stage can select `r`: not on a hidden or locked layer.
pub fn selectable(ed: &Editor, r: ElementRef) -> bool {
    ed.timeline().layers.get(r.layer).is_some_and(|l| l.visible() && !l.locked())
}

/// A shape drawn in Object Drawing mode: selected, it gets a box, not dots.
pub(crate) fn drawing_object(s: &Shape) -> bool {
    s.extra.get("isDrawingObject") == Some("true")
}

/// A raw (Merge Drawing) shape: selected, it shows dots.
fn raw_shape(ed: &Editor, r: ElementRef) -> bool {
    matches!(ed.element(r), Some(Element::Shape(s)) if !drawing_object(s))
}

/// What a Selection-tool drag from `p` takes hold of in `r`: a corner or an
/// edge of an unselected raw shape. Selected, or anything else, moves.
fn grip_at(ed: &Editor, r: ElementRef, p: Point) -> Option<Grip> {
    match ed.element(r)? {
        Element::Shape(s) if !drawing_object(s) && !ed.selection.contains(&r) => hit::grip(s, p, 6.0 / ed.zoom, 3.0 / ed.zoom),
        _ => None,
    }
}

pub(crate) fn show_selection_props(ed: &mut Editor) {
    ed.props_tab = if ed.selection.is_empty() { PropsTab::Doc } else { PropsTab::Object };
}

fn click(ed: &mut Editor, p: Point, shift: bool) {
    if selecting(ed.tool) {
        // A click selects what's under it; Shift adds or removes; empty space deselects.
        match (hit_at(ed, p), shift) {
            (Some(h), true) => match ed.selection.iter().position(|r| *r == h) {
                Some(i) => {
                    ed.selection.remove(i);
                }
                None => ed.selection.push(h),
            },
            (Some(h), false) => ed.selection = vec![h],
            (None, false) => ed.selection.clear(),
            (None, true) => {}
        }
        if let Some(&last) = ed.selection.last() {
            ed.layer = last.layer;
        }
        show_selection_props(ed);
    } else if matches!(ed.tool, Tool::Pencil | Tool::Brush) {
        // A click with a brush leaves a dot.
        ed.drag = Some(Drag::Stroke { points: vec![p] });
        finish(ed, false, false);
    }
}

fn start(ed: &mut Editor, p: Point, shift: bool, alt: bool) {
    ed.drag = match ed.tool {
        tool if selecting(tool) => match hit_at(ed, p) {
            // An unselected shape's edge bends, and its corners move.
            Some(h) if let Some(grip) = grip_at(ed, h, p).filter(|_| tool == Tool::Selection) => {
                let Some(Element::Shape(original)) = ed.element(h).cloned() else { return };
                ed.selection.clear();
                ed.layer = h.layer;
                show_selection_props(ed);
                Some(Drag::Reshape { r: h, original, grip, start: p })
            }
            // Dragging an object moves it, with the rest of the selection if it was part of it.
            Some(h) => {
                if !ed.selection.contains(&h) {
                    if !shift {
                        ed.selection.clear();
                    }
                    ed.selection.push(h);
                }
                ed.layer = h.layer;
                show_selection_props(ed);
                // Alt drags a copy, leaving the original where it was.
                if alt {
                    duplicate_selection(ed);
                }
                Some(Drag::Move { start: p, moved: (0.0, 0.0), copy: alt })
            }
            None => {
                if !shift {
                    ed.selection.clear();
                }
                Some(Drag::Marquee { start: p, end: p })
            }
        },
        Tool::Rectangle | Tool::Oval | Tool::Line | Tool::PolyStar => Some(Drag::Shape { start: p, end: p }),
        Tool::Pencil | Tool::Brush => Some(Drag::Stroke { points: vec![p] }),
        Tool::Lasso => {
            if !shift {
                ed.selection.clear();
            }
            Some(Drag::Lasso { points: vec![p] })
        }
        _ => None,
    };
}

/// Puts a copy of each selected element on top of its keyframe and selects
/// the copies, in the same undo step as the move that follows.
fn duplicate_selection(ed: &mut Editor) {
    ed.checkpoint_gesture("move");
    let mut copies = Vec::new();
    for r in ed.selection.clone() {
        let Some(el) = ed.element(r).cloned() else { continue };
        let key = &mut ed.timeline_mut().layers[r.layer].frames[r.keyframe];
        key.elements.push(el);
        copies.push(ElementRef { element: key.elements.len() - 1, ..r });
    }
    ed.selection = copies;
}

fn update(ed: &mut Editor, p: Point, shift: bool) {
    ed.drag = match ed.drag.take() {
        Some(Drag::Move { start, moved, copy }) => {
            // Shift keeps the move horizontal, vertical or at 45°.
            let d = (p.0 - start.0, p.1 - start.1);
            let d = if shift { constrain(d) } else { d };
            ed.nudge(d.0 - moved.0, d.1 - moved.1, "move");
            Some(Drag::Move { start, moved: d, copy })
        }
        Some(Drag::Reshape { r, original, grip, start }) => {
            let d = (p.0 - start.0, p.1 - start.1);
            if d != (0.0, 0.0) {
                ed.checkpoint_gesture("reshape");
                if let Some(el) = ed.element_mut(r) {
                    *el = Element::Shape(hit::reshape(&original, grip, d));
                }
            }
            Some(Drag::Reshape { r, original, grip, start })
        }
        Some(Drag::Marquee { start, .. }) => Some(Drag::Marquee { start, end: p }),
        Some(Drag::Shape { start, .. }) => Some(Drag::Shape { start, end: p }),
        Some(Drag::Stroke { mut points }) => {
            if points.last() != Some(&p) {
                points.push(p);
            }
            Some(Drag::Stroke { points })
        }
        Some(Drag::Lasso { mut points }) => {
            if points.last() != Some(&p) {
                points.push(p);
            }
            Some(Drag::Lasso { points })
        }
        None => None,
    };
}

/// `d` along the nearest of the eight directions 45° apart.
fn constrain((dx, dy): Point) -> Point {
    let angle = (dy.atan2(dx) / FRAC_PI_4).round() * FRAC_PI_4;
    let (sin, cos) = angle.sin_cos();
    let along = dx * cos + dy * sin;
    // Exact zeros straight across or down, where cos or sin is a hair off.
    let clean = |v: f32| if v.abs() < 1e-4 { 0.0 } else { v };
    (clean(along * cos), clean(along * sin))
}

fn finish(ed: &mut Editor, shift: bool, alt: bool) {
    match ed.drag.take() {
        Some(Drag::Move { .. } | Drag::Reshape { .. }) => ed.end_gesture(),
        Some(Drag::Lasso { points }) => {
            // A shape is lassoed when the loop takes in its centre or a corner of its box,
            // an object when it takes in its whole box.
            select_where(ed, |b, raw| {
                let corners = [b.min, b.max, (b.min.0, b.max.1), (b.max.0, b.min.1)];
                match raw {
                    true => corners.into_iter().chain([((b.min.0 + b.max.0) / 2.0, (b.min.1 + b.max.1) / 2.0)]).any(|q| inside(&points, q)),
                    false => corners.into_iter().all(|q| inside(&points, q)),
                }
            });
        }
        Some(Drag::Marquee { start, end }) => {
            // Shapes the rectangle touches are selected; objects only when it encloses them.
            // ponytail: a shape is taken whole, where Animate cuts it along the rectangle.
            let area = hit::Rect { min: (start.0.min(end.0), start.1.min(end.1)), max: (start.0.max(end.0), start.1.max(end.1)) };
            select_where(ed, |b, raw| if raw { overlaps(b, area) } else { encloses(area, b) });
        }
        Some(Drag::Shape { start, end }) => {
            if let Some(shape) = shape_for(ed, start, end, shift, alt) {
                ed.add_shape(shape);
            }
        }
        Some(Drag::Stroke { points }) => {
            let shape = match ed.tool {
                // Drop wobble finer than a couple of screen pixels.
                Tool::Pencil => Some(draw::pencil(&points, ed.stroke(), 2.0 / ed.zoom)),
                Tool::Brush => Some(draw::brush(&points, ed.brush_size, ed.fill())),
                _ => None,
            };
            if let Some(shape) = shape {
                ed.add_shape(shape);
            }
        }
        None => {}
    }
}

/// Adds to the selection each selectable element that `taken` accepts, by its
/// box and whether it's a raw shape.
fn select_where(ed: &mut Editor, taken: impl Fn(hit::Rect, bool) -> bool) {
    let found: Vec<ElementRef> = (ed.all_elements().into_iter())
        .filter(|&r| selectable(ed, r) && !ed.selection.contains(&r))
        .filter(|&r| hit::bounds(&ed.doc, ed.timeline(), ed.frame, &[r]).is_some_and(|b| taken(b, raw_shape(ed, r))))
        .collect();
    ed.selection.extend(found);
    show_selection_props(ed);
}

/// Even-odd point-in-polygon; the polygon closes back to its first point.
fn inside(polygon: &[Point], (x, y): Point) -> bool {
    let mut odd = false;
    for (i, &(x1, y1)) in polygon.iter().enumerate() {
        let (x0, y0) = polygon[(i + polygon.len() - 1) % polygon.len()];
        if (y0 > y) != (y1 > y) && x < x0 + (y - y0) / (y1 - y0) * (x1 - x0) {
            odd = !odd;
        }
    }
    odd
}

pub(crate) fn overlaps(a: hit::Rect, b: hit::Rect) -> bool {
    a.min.0 <= b.max.0 && b.min.0 <= a.max.0 && a.min.1 <= b.max.1 && b.min.1 <= a.max.1
}

/// Whether `outer` holds all of `inner`.
fn encloses(outer: hit::Rect, inner: hit::Rect) -> bool {
    outer.min.0 <= inner.min.0 && outer.min.1 <= inner.min.1 && inner.max.0 <= outer.max.0 && inner.max.1 <= outer.max.1
}

/// The shape a Rectangle, Oval, Line or PolyStar drag from `start` to `end`
/// makes. Shift makes squares, circles and 45° lines; Alt draws from the center.
fn shape_for(ed: &Editor, start: Point, end: Point, shift: bool, alt: bool) -> Option<Shape> {
    let (dx, dy) = (end.0 - start.0, end.1 - start.1);
    if dx.abs().max(dy.abs()) * ed.zoom < 2.0 {
        return None;
    }
    match ed.tool {
        Tool::Line => {
            let end = if shift {
                let angle = (dy.atan2(dx) / FRAC_PI_4).round() * FRAC_PI_4;
                let len = dx.hypot(dy);
                (start.0 + len * angle.cos(), start.1 + len * angle.sin())
            } else {
                end
            };
            Some(draw::line(start, end, ed.stroke()))
        }
        Tool::PolyStar => {
            let star = ed.polystar_star.then_some(ed.polystar_point);
            Some(draw::polystar(start, dx.hypot(dy), ed.polystar_sides, star, dy.atan2(dx), Some(ed.stroke()), Some(ed.fill())))
        }
        Tool::Rectangle | Tool::Oval => {
            let (mut w, mut h) = (dx, dy);
            if shift {
                let side = w.abs().max(h.abs());
                (w, h) = (side * w.signum(), side * h.signum());
            }
            let (a, b) = if alt { ((start.0 - w, start.1 - h), (start.0 + w, start.1 + h)) } else { (start, (start.0 + w, start.1 + h)) };
            Some(if ed.tool == Tool::Rectangle {
                draw::rectangle([a, b], ed.corner_radius, Some(ed.stroke()), Some(ed.fill()))
            } else {
                draw::oval([a, b], Some(ed.stroke()), Some(ed.fill()))
            })
        }
        _ => None,
    }
}

/// `elements`, as one layer of the edited timeline, drawn into the view's canvas.
fn render_elements(ed: &Editor, elements: Vec<Element>, vp: &ViewPort) -> Option<sk::Pixmap> {
    let frame = Frame { index: 0, duration: 1, tween: Tween::None, elements, extra: Default::default() };
    let layer = Layer { name: String::new(), kind: LayerKind::Normal, frames: vec![frame], extra: Default::default() };
    let tl = Timeline { name: String::new(), layers: vec![layer], extra: Default::default() };
    let matrix = vp.matrix * ed.editing.last().map_or(Matrix::IDENTITY, |t| t.transform);
    render_timeline(&ed.doc, &tl, 0, &matrix, vp.size, 1.0).ok().map(|(pixmap, _)| pixmap)
}

/// Selected raw shapes show white dots: a 4 × 4 tile fixed to the screen, a
/// white pixel over a black one, inside exactly what the shapes draw.
fn dots(ed: &mut Editor, ctx: &egui::Context, vp: &ViewPort) -> Option<egui::TextureId> {
    let raw: Vec<ElementRef> = ed.selection.iter().copied().filter(|&r| raw_shape(ed, r)).collect();
    // The Gradient Transform tool shows its handles over the fill instead.
    if raw.is_empty() || ed.tool == Tool::GradientTransform {
        return None;
    }
    let ppp = ctx.pixels_per_point();
    let key = hash((ed.rev, &raw, ed.frame, ed.editing.len(), vp.bits(), ppp.to_bits()));
    ed.canvas_texture(ctx, "dots", key, |ed| {
        // The shapes in white, a hairline as a band at least 3 px wide so it shows.
        let white = Paint::Solid([1.0; 4]);
        let thinnest = 3.0 / ed.zoom;
        let shapes = raw.iter().filter_map(|&r| match ed.element(r)? {
            Element::Shape(s) => Some(Element::Shape(Shape {
                fills: vec![white.clone(); s.fills.len()],
                strokes: s.strokes.iter().map(|st| doc::Stroke { width: st.width.max(thinnest), paint: white.clone(), ..st.clone() }).collect(),
                ..s.clone()
            })),
            _ => None,
        });
        let mask = render_elements(ed, shapes.collect(), vp)?;
        let mask = sk::Mask::from_pixmap(mask.as_ref(), sk::MaskType::Alpha);
        let px = ppp.round().max(1.0);
        let mut tile = sk::Pixmap::new(4 * px as u32, 4 * px as u32)?;
        for (at, color) in [(0.0, sk::Color::WHITE), (px, sk::Color::BLACK)] {
            let mut paint = sk::Paint::default();
            paint.set_color(color);
            tile.fill_rect(sk::Rect::from_xywh(at, at, px, px)?, &paint, sk::Transform::identity(), None);
        }
        let mut out = sk::Pixmap::new(mask.width(), mask.height())?;
        let pattern = sk::Pattern::new(tile.as_ref(), sk::SpreadMode::Repeat, sk::FilterQuality::Nearest, 1.0, sk::Transform::identity());
        let paint = sk::Paint { shader: pattern, ..Default::default() };
        let all = sk::Rect::from_xywh(0.0, 0.0, out.width() as f32, out.height() as f32)?;
        out.fill_rect(all, &paint, sk::Transform::identity(), Some(&mask));
        Some(out)
    })
}

/// Instance `r` as it shows at the current frame: a classic tween moves it
/// between its keyframes (along a motion guide), and parenting after its parent.
pub(crate) fn shown_instance(ed: &Editor, r: ElementRef) -> Option<Instance> {
    minnamate_core::layer_motion::shown_instance(ed.timeline(), r, ed.frame)
}

/// The Selection tool's look for every selected object but raw shapes (they
/// get `dots`): a 1 px box, turned with an instance or text, an instance's
/// registration "+" and transformation point, text's handle, a drawing
/// object's outline in its layer's colour. No handles: those are Free Transform's.
fn highlights(p: &Painter, ed: &Editor, stage: Rect) {
    let ppp = p.ctx().pixels_per_point();
    let at = |q: Point| screen_point(ed, stage, q).round_to_pixel_center(ppp);
    let line = Stroke::new(1.0, theme::SELECTION_BOX);
    let quad = |m: &Matrix, (x0, y0): Point, (x1, y1): Point| [(x0, y0), (x1, y0), (x1, y1), (x0, y1)].map(|c| at(m.apply(c))).to_vec();
    let tl = ed.timeline();
    let bounds = |r: ElementRef| hit::bounds(&ed.doc, tl, ed.frame, &[r]);
    // Free Transform draws a lone object's box and transformation point itself.
    let boxed = ed.tool == Tool::FreeTransform && ed.selection.len() == 1;
    for &r in &ed.selection {
        match ed.element(r) {
            None => {}
            Some(Element::Shape(s)) if !drawing_object(s) => {}
            Some(Element::Instance(_)) => {
                let Some(inst) = shown_instance(ed, r) else { continue };
                let local = inst.matrix.inverse().and_then(|inv| hit::bounds_through(&ed.doc, tl, ed.frame, &[r], inv));
                if let Some(b) = local.filter(|_| !boxed) {
                    p.add(egui::Shape::closed_line(quad(&inst.matrix, b.min, b.max), line));
                }
                registration(p, at(inst.matrix.apply((0.0, 0.0))));
                if !boxed {
                    transformation_point(p, at(inst.matrix.apply(inst.pivot)));
                }
            }
            _ if boxed => {}
            Some(Element::Text(t)) => {
                let layout = text::layout(t);
                let (w, h) = (layout.width, layout.height);
                // Where it shows: a motion tween moves it.
                let matrix = crate::tweens::shown_matrix(ed, r).unwrap_or(t.matrix);
                p.add(egui::Shape::closed_line(quad(&matrix, (0.0, 0.0), (w, h)), line));
                // Static text's handle is at the top right, dynamic and input text's at the bottom right:
                // round while the box grows with its text, square once its width is set.
                let corner = at(matrix.apply(if t.kind == TextKind::Static { (w, 0.0) } else { (w, h) }));
                if t.width.is_some() {
                    p.rect(Rect::from_center_size(corner, vec2(5.0, 5.0)), 0.0, Color32::WHITE, line, StrokeKind::Middle);
                } else {
                    p.circle(corner, 2.5, Color32::WHITE, line);
                }
            }
            Some(el) => {
                if let Some(b) = bounds(r) {
                    p.add(egui::Shape::closed_line(quad(&Matrix::IDENTITY, b.min, b.max), line));
                }
                // A drawing object also shows its outline and anchor points in its layer's colour.
                if let Element::Shape(s) = el {
                    let color = crate::timeline::layer_color(&tl.layers[r.layer], r.layer);
                    let to_screen = |q: Point| at(s.matrix.apply(q));
                    outline(p, s, &to_screen, Stroke::new(1.0, color));
                    for path in s.edges.iter().flat_map(|e| &e.paths) {
                        for q in std::iter::once(path.start).chain(path.segs.iter().map(|seg| seg.end())) {
                            p.circle_stroke(to_screen(q), 2.0, Stroke::new(1.0, color));
                        }
                    }
                }
            }
        }
    }
}

/// An instance's registration point: a 9 px "+".
fn registration(p: &Painter, at: Pos2) {
    let ink = Stroke::new(1.0, Color32::BLACK);
    p.hline(at.x - 4.5..=at.x + 4.5, at.y, ink);
    p.vline(at.x, at.y - 4.5..=at.y + 4.5, ink);
}

/// The transformation point: an 8 px white circle with a black ring.
pub(crate) fn transformation_point(p: &Painter, at: Pos2) {
    p.circle(at, 4.0, Color32::WHITE, Stroke::new(1.0, Color32::BLACK));
}

/// The marquee, the lasso and a brush stroke as they're dragged. A shape drag
/// shows as the shape itself (see `panel`).
fn drag_feedback(p: &Painter, ed: &Editor, stage: Rect) {
    let zoom = ed.zoom;
    let to_screen = |q: Point| screen_point(ed, stage, q);
    // Black on a light rim, so it shows on the pasteboard as on the stage.
    let lines = [Stroke::new(3.0, theme::MARQUEE_RIM), Stroke::new(1.0, theme::MARQUEE)];
    match &ed.drag {
        Some(Drag::Marquee { start, end }) => {
            // In the edited timeline's coordinates, as it shows on screen (a symbol
            // being edited in place may be rotated or skewed).
            let ((x0, y0), (x1, y1)) = (*start, *end);
            let ppp = p.ctx().pixels_per_point();
            let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)].map(|q| to_screen(q).round_to_pixel_center(ppp));
            for line in lines {
                p.add(egui::Shape::closed_line(corners.to_vec(), line));
            }
        }
        Some(Drag::Stroke { points }) => {
            let pts: Vec<Pos2> = points.iter().map(|&q| to_screen(q)).collect();
            let stroke = if ed.tool == Tool::Brush {
                Stroke::new(ed.brush_size * zoom, crate::color::solid(&ed.fill()))
            } else {
                Stroke::new((ed.stroke_width * zoom).max(1.0), crate::color::solid(&ed.stroke().paint))
            };
            p.add(egui::Shape::line(pts, stroke));
        }
        Some(Drag::Lasso { points }) => {
            let pts: Vec<Pos2> = points.iter().map(|&q| to_screen(q)).collect();
            for line in lines {
                p.add(egui::Shape::closed_line(pts.clone(), line));
            }
        }
        _ => {}
    }
}

/// Strokes every path of `shape`, curves flattened, in screen space.
pub(crate) fn outline(p: &Painter, shape: &Shape, to_screen: &dyn Fn(Point) -> Pos2, stroke: Stroke) {
    for path in shape.edges.iter().flat_map(|e| &e.paths) {
        let mut pts = vec![to_screen(path.start)];
        let mut from = path.start;
        for &seg in &path.segs {
            match seg {
                Seg::Line(to) => pts.push(to_screen(to)),
                Seg::Quad(c, to) => {
                    for i in 1..=8 {
                        let t = i as f32 / 8.0;
                        let u = 1.0 - t;
                        let q = (u * u * from.0 + 2.0 * u * t * c.0 + t * t * to.0, u * u * from.1 + 2.0 * u * t * c.1 + t * t * to.1);
                        pts.push(to_screen(q));
                    }
                }
            }
            from = seg.end();
        }
        p.add(egui::Shape::line(pts, stroke));
    }
}

/// The pointer over the stage: the Selection tool's arrow with a badge for
/// what a drag would do, the hand, the magnifier, the pencil, crosshairs for
/// the shape tools. Free Transform, the Eraser and the paint tools set their own.
fn pointer_glyph(ctx: &egui::Context, ed: &Editor, resp: &egui::Response, stage: Rect, hand: bool, alt: bool, over_guide: Option<Guide>) {
    if hand {
        return cursors::show(ctx, if resp.dragged() { Cursor::Grab } else { Cursor::Hand });
    }
    if let Some(guide) = over_guide {
        return ctx.set_cursor_icon(if matches!(guide, Guide::X(_)) { CursorIcon::ResizeHorizontal } else { CursorIcon::ResizeVertical });
    }
    let icon = match ed.tool {
        Tool::Zoom => return cursors::show(ctx, if alt { Cursor::ZoomOut } else { Cursor::ZoomIn }),
        Tool::Pencil => return cursors::show(ctx, Cursor::Pencil),
        Tool::Selection => {
            let Some(pos) = resp.hover_pos() else { return };
            let badge = crate::tweens::cursor(ed, stage, pos).unwrap_or_else(|| selection_badge(ed, local_point(ed, stage, pos), alt));
            return cursors::show(ctx, badge);
        }
        Tool::Text => CursorIcon::Text,
        Tool::Line | Tool::Rectangle | Tool::Oval | Tool::PolyStar | Tool::Pen => CursorIcon::Crosshair,
        // These draw the tool itself as the pointer.
        Tool::Brush | Tool::PaintBucket | Tool::InkBottle | Tool::Eyedropper => CursorIcon::None,
        _ => CursorIcon::Default,
    };
    ctx.set_cursor_icon(icon);
}

/// The Selection tool's badge at `p`: move over what a drag moves, an arc over
/// an edge it bends, a corner over a point it moves, "+" with Alt for a copy.
fn selection_badge(ed: &Editor, p: Point, alt: bool) -> Cursor {
    match &ed.drag {
        Some(Drag::Move { copy: true, .. }) => Cursor::Copy,
        Some(Drag::Move { .. }) => Cursor::Move,
        Some(Drag::Reshape { grip: Grip::Corner(_), .. }) => Cursor::Corner,
        Some(Drag::Reshape { .. }) => Cursor::Arc,
        Some(_) => Cursor::Arrow,
        None => match hit_at(ed, p) {
            None => Cursor::Arrow,
            Some(r) => match grip_at(ed, r, p) {
                Some(Grip::Corner(_)) => Cursor::Corner,
                Some(Grip::Edge { .. }) => Cursor::Arc,
                None if alt => Cursor::Copy,
                None => Cursor::Move,
            },
        },
    }
}

fn stage_rect(ed: &Editor, view: Rect, ppp: f32) -> Rect {
    let center = view.center() + ed.pan;
    // Whole device pixels keep the stage image sharp.
    let size = vec2(ed.doc.width, ed.doc.height) * ed.zoom;
    Rect::from_center_size(center, size).round_to_pixels(ppp)
}

/// Zooms keeping the document point under `anchor` still.
fn zoom_at(ed: &mut Editor, view: Rect, target: f32, anchor: Option<Pos2>) {
    let anchor = anchor.unwrap_or(view.center());
    let old = ed.zoom;
    ed.zoom_to(target);
    let center = view.center() + ed.pan;
    let doc_point = (anchor - center) / old;
    ed.pan = anchor - doc_point * ed.zoom - view.center();
}

/// The scrollbar thumbs, the right one then the bottom one, each with how far
/// the view pans per point it's dragged. The view scrolls over the stage and
/// a view's size of pasteboard all round it. A view too small for them has none.
// ponytail: the scroll range doesn't stretch to take in artwork far off the stage.
fn thumbs(ed: &Editor, view: Rect, ppp: f32) -> Vec<(Rect, f32)> {
    let all = stage_rect(ed, view, ppp).expand2(view.size()).union(view);
    let (gap, end) = (2.0, 2.0 + THUMB + 2.0);
    let (top, bottom) = (view.top() + gap, view.bottom() - end);
    let (left, right) = (view.left() + gap, view.right() - end);
    let ys = (bottom - top) / all.height();
    let xs = (right - left) / all.width();
    let y0 = top + (view.top() - all.top()) * ys;
    let x0 = left + (view.left() - all.left()) * xs;
    if ys <= 0.0 || xs <= 0.0 {
        return Vec::new();
    }
    vec![
        (Rect::from_min_max(pos2(view.right() - gap - THUMB, y0), pos2(view.right() - gap, y0 + view.height() * ys)), 1.0 / ys),
        (Rect::from_min_max(pos2(x0, view.bottom() - gap - THUMB), pos2(x0 + view.width() * xs, view.bottom() - gap)), 1.0 / xs),
    ]
}

/// View > Grid > Show Grid: lines every 10 stage pixels, under the artwork.
fn grid(p: &Painter, stage: Rect, zoom: f32, ppp: f32) {
    let clip = stage.intersect(p.clip_rect());
    if clip.is_negative() {
        return;
    }
    // Zoomed far out, every second (fourth...) line, so the grid doesn't fill in.
    let mut step = GRID_STEP * zoom;
    while step < 4.0 {
        step *= 2.0;
    }
    let ink = Stroke::new(1.0, theme::GRID);
    let first = |from: f32, to: f32| from + ((to - from) / step).ceil().max(1.0) * step;
    let mut x = first(stage.left(), clip.left());
    while x < clip.right() {
        p.vline(x.round_to_pixel_center(ppp), clip.y_range(), ink);
        x += step;
    }
    let mut y = first(stage.top(), clip.top());
    while y < clip.bottom() {
        p.hline(clip.x_range(), y.round_to_pixel_center(ppp), ink);
        y += step;
    }
}

/// Guides: dragged out of a ruler, moved with the Selection tool unless
/// locked, dropped back off the view to remove them. Returns the guide the
/// pointer is over or dragging, for its pointer.
fn guides_input(ui: &Ui, ed: &mut Editor, resp: &egui::Response, full: Rect, view: Rect, stage: Rect) -> Option<Guide> {
    let zoom = ed.zoom;
    let movable = ed.tool == Tool::Selection && ed.drag.is_none();
    let s = &mut ed.stage;
    if s.rulers {
        let top = Rect::from_min_max(pos2(view.left(), full.top()), pos2(view.right(), view.top()));
        let left = Rect::from_min_max(pos2(full.left(), view.top()), pos2(view.left(), view.bottom()));
        for (i, (rect, guide)) in [(top, Guide::Y(0.0)), (left, Guide::X(0.0))].into_iter().enumerate() {
            if ui.interact(rect, ui.id().with(("ruler", i)), Sense::drag()).drag_started() {
                (s.dragging, s.guides_shown) = (Some((None, guide)), true);
            }
        }
    }
    let near = |g: &Guide, pos: Pos2| match *g {
        Guide::X(x) => (stage.left() + x * zoom - pos.x).abs() <= 3.0,
        Guide::Y(y) => (stage.top() + y * zoom - pos.y).abs() <= 3.0,
    };
    let movable = movable && s.guides_shown && !s.guides_locked;
    let grabbed = |pos: Option<Pos2>| pos.filter(|_| movable).and_then(|pos| s.guides.iter().position(|g| near(g, pos)));
    if s.dragging.is_none()
        && resp.drag_started()
        && let Some(i) = grabbed(ui.input(|i| i.pointer.press_origin()))
    {
        s.dragging = Some((Some(i), s.guides[i]));
    }
    let Some((index, guide)) = s.dragging else { return grabbed(resp.hover_pos()).map(|i| s.guides[i]) };
    let (down, pos) = ui.input(|i| (i.pointer.primary_down(), i.pointer.interact_pos()));
    let Some(pos) = pos else { return Some(guide) };
    // On whole stage pixels.
    let guide = match guide {
        Guide::X(_) => Guide::X(((pos.x - stage.left()) / zoom).round()),
        Guide::Y(_) => Guide::Y(((pos.y - stage.top()) / zoom).round()),
    };
    if down {
        s.dragging = Some((index, guide));
        return Some(guide);
    }
    s.dragging = None;
    let kept = view.contains(pos);
    match index.filter(|&i| i < s.guides.len()) {
        Some(i) if kept => s.guides[i] = guide,
        Some(i) => {
            s.guides.remove(i);
        }
        None if kept && index.is_none() => s.guides.push(guide),
        None => {}
    }
    None
}

/// Guides across the view, in green, the one being dragged where it is now.
fn paint_guides(p: &Painter, ed: &Editor, stage: Rect, ppp: f32) {
    let s = &ed.stage;
    let moving = s.dragging.and_then(|(i, _)| i);
    let shown = s.guides.iter().enumerate().filter(|&(i, _)| s.guides_shown && Some(i) != moving).map(|(_, g)| *g);
    let view = p.clip_rect();
    let ink = Stroke::new(1.0, theme::GUIDE);
    for g in shown.chain(s.dragging.map(|(_, g)| g)) {
        match g {
            Guide::X(x) => p.vline((stage.left() + x * ed.zoom).round_to_pixel_center(ppp), view.y_range(), ink),
            Guide::Y(y) => p.hline(view.x_range(), (stage.top() + y * ed.zoom).round_to_pixel_center(ppp), ink),
        };
    }
}

/// View > Rulers: 18 px strips along the top and left of the view, counting
/// stage pixels from the stage's top-left corner, numbered at every long tick.
fn rulers(p: &Painter, ed: &Editor, full: Rect, view: Rect, stage: Rect, ppp: f32) {
    let top = Rect::from_min_max(pos2(view.left(), full.top()), pos2(view.right(), view.top()));
    let left = Rect::from_min_max(pos2(full.left(), view.top()), pos2(view.left(), view.bottom()));
    p.rect_filled(Rect::from_min_max(full.min, pos2(full.right(), view.top())), 0.0, theme::PANEL_BG);
    p.rect_filled(left, 0.0, theme::PANEL_BG);
    let ink = Stroke::new(1.0, theme::RULER_TEXT);
    let font = FontId::proportional(9.5);
    let pt = p.with_clip_rect(top);
    for (x, len, label) in ticks(top.x_range(), stage.left(), ed.zoom, ppp) {
        pt.vline(x, top.bottom() - len..=top.bottom(), ink);
        if let Some(label) = label {
            pt.text(pos2(x + 2.0, top.top() + 1.0), egui::Align2::LEFT_TOP, label, font.clone(), theme::RULER_TEXT);
        }
    }
    let pl = p.with_clip_rect(left);
    for (y, len, label) in ticks(left.y_range(), stage.top(), ed.zoom, ppp) {
        pl.hline(left.right() - len..=left.right(), y, ink);
        if let Some(label) = label {
            // Read upward, as rulers number their left side.
            let galley = pl.layout_no_wrap(label, font.clone(), theme::RULER_TEXT);
            pl.add(egui::epaint::TextShape::new(pos2(left.left() + 1.0, y - 2.0), galley, theme::RULER_TEXT).with_angle(-FRAC_PI_2));
        }
    }
    let edge = Stroke::new(1.0, theme::BORDER);
    p.hline(full.x_range(), view.top() - 0.5, edge);
    p.vline(view.left() - 0.5, view.y_range(), edge);
}

/// The ticks along a ruler spanning `span` on screen, stage pixel 0 being at
/// `from`: each one's place, length and number. Numbered ticks are at least
/// 50 points apart, on 1, 2 or 5 times a power of ten, with ten (or five, or
/// two) short ones between, the middle one longer.
fn ticks(span: egui::Rangef, from: f32, zoom: f32, ppp: f32) -> Vec<(f32, f32, Option<String>)> {
    let major = (0..6).flat_map(|k| [1.0, 2.0, 5.0].map(|m| m * 10f32.powi(k))).find(|s| s * zoom >= 50.0).unwrap_or(100_000.0);
    let parts = [10, 5, 2].into_iter().find(|&n| major / n as f32 * zoom >= 4.0).unwrap_or(1);
    let minor = major / parts as f32;
    let (a, b) = ((span.min - from) / zoom / minor, (span.max - from) / zoom / minor);
    (a.ceil() as i64..=b.floor() as i64)
        .map(|i| {
            let at = (from + i as f32 * minor * zoom).round_to_pixel_center(ppp);
            match i.rem_euclid(parts) {
                0 => (at, RULER, Some(((i as f32 * minor).round() as i64).to_string())),
                h if parts % 2 == 0 && h == parts / 2 => (at, 7.0, None),
                _ => (at, 4.0, None),
            }
        })
        .collect()
}

/// A flat edit-bar button with a glyph and a small "v": it opens a menu, and
/// shows pressed while that's open.
fn menu_button(ui: &mut Ui, icon: Icon, tooltip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 24.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tooltip));
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp));
    let p = ui.painter_at(rect);
    if open || resp.is_pointer_button_down_on() {
        p.rect_filled(rect, 4.0, theme::BUTTON_ACTIVE);
    } else if resp.hovered() {
        p.rect_filled(rect, 4.0, theme::BUTTON_HOVER);
    }
    let color = if open || resp.hovered() { Color32::WHITE } else { theme::TEXT };
    icons::paint(&p, Rect::from_min_size(rect.min + vec2(4.0, 4.0), vec2(16.0, 16.0)), icon, color);
    // A 6 px chevron (docs/animate-ui.md §4.4): the glyph's 14 px box.
    icons::paint(&p, Rect::from_center_size(rect.min + vec2(24.0, 12.0), vec2(14.0, 14.0)), Icon::ChevronDown, theme::TEXT_DIM);
    resp.on_hover_text(tooltip)
}

/// A 16 × 24 zoom-step arrow.
fn step_button(ui: &mut Ui, icon: Icon, enabled: bool, tooltip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(16.0, 24.0), if enabled { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, tooltip));
    if enabled && resp.hovered() {
        ui.painter().rect_filled(rect, 2.0, theme::BUTTON_HOVER);
    }
    let color = if !enabled { theme::TEXT_DISABLED } else if resp.hovered() { Color32::WHITE } else { theme::TEXT };
    // A 14 px box: a 3.5 × 7 px chevron, big enough to show at full strength.
    icons::paint(ui.painter(), Rect::from_center_size(rect.center(), vec2(14.0, 14.0)), icon, color);
    resp.on_hover_text(tooltip)
}

/// The glyph a symbol's breadcrumb shows: its kind.
fn symbol_icon(ed: &Editor, name: &str) -> Icon {
    match ed.doc.library.get(name).map(|s| s.kind) {
        Some(doc::SymbolKind::MovieClip) => Icon::MovieClip,
        Some(doc::SymbolKind::Button) => Icon::Button,
        _ => Icon::Graphic,
    }
}

fn edit_bar(ui: &mut Ui, ed: &mut Editor) {
    let mut action = None;
    let (mut go_scene, mut edit_symbol) = (None, None);
    let bar = egui::Frame::NONE.fill(theme::PANEL_BG).inner_margin(Margin::symmetric(8, 0)).show(ui, |ui| {
        ui.set_height(30.0);
        ui.set_width(ui.available_width());
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            // Back goes up one level; it only does something inside a symbol.
            let editing = !ed.editing.is_empty();
            let back = theme::icon_button(ui, 24.0, false, editing, "Back", |p, r, c| icons::paint(p, r.shrink(5.0), Icon::Back, c));
            if back.clicked() {
                ed.exit_symbol();
            }
            let scenes = menu_button(ui, Icon::Scene, "Edit Scene");
            egui::Popup::menu(&scenes).style(theme::menu_style).show(|ui| {
                for (i, scene) in ed.doc.scenes.iter().enumerate() {
                    let item = theme::menu_item(ui, true, &scene.name, "");
                    if i == ed.scene {
                        theme::menu_check(ui, &item);
                    }
                    if item.clicked() {
                        go_scene = Some(i);
                    }
                }
            });
            let symbols = menu_button(ui, Icon::Club, "Edit Symbols");
            egui::Popup::menu(&symbols).style(theme::menu_style).show(|ui| {
                let mut names: Vec<&String> = ed.doc.library.keys().collect();
                names.sort();
                if names.is_empty() {
                    theme::menu_item(ui, false, "(no symbols)", "");
                }
                for name in names {
                    if theme::menu_item(ui, true, name, "").clicked() {
                        edit_symbol = Some(name.clone());
                    }
                }
            });
            ui.add_space(4.0);
            // The right-hand group goes in first, in a child that takes no room
            // here, so the breadcrumb gets what's left: cut short, and clipped to it.
            // ponytail: cut from the right, so a deep path hides the current symbol
            // first; elide the middle crumbs instead if that bites.
            let mut right = ui.new_child(egui::UiBuilder::new().max_rect(ui.available_rect_before_wrap()).layout(Layout::right_to_left(Align::Center)));
            {
                let ui = &mut right;
                ui.spacing_mut().item_spacing.x = 4.0;
                zoom_field(ui, ed, &mut action);
                // Zoom-step arrows walk the presets.
                let step = |up: bool| {
                    let next = if up { ZOOMS.iter().find(|&&z| z > ed.zoom * 1.001) } else { ZOOMS.iter().rev().find(|&&z| z < ed.zoom * 0.999) };
                    next.copied()
                };
                let (up, down) = (step(true), step(false));
                ui.spacing_mut().item_spacing.x = 0.0;
                if step_button(ui, Icon::ChevronRight, up.is_some(), "Zoom in").clicked() {
                    action = up.map(Action::Zoom);
                }
                if step_button(ui, Icon::ChevronLeft, down.is_some(), "Zoom out").clicked() {
                    action = down.map(Action::Zoom);
                }
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.add_space(8.0);
                let clip = theme::icon_button(ui, 24.0, ed.clip_to_stage, true, "Clip content outside the stage", |p, r, c| icons::paint(p, r.shrink(5.0), Icon::ClipContent, c));
                if clip.clicked() {
                    ed.clip_to_stage = !ed.clip_to_stage;
                }
                if theme::icon_button(ui, 24.0, false, true, "Center Stage", |p, r, c| icons::paint(p, r.shrink(5.0), Icon::CenterStage, c)).clicked() {
                    ed.pan = egui::Vec2::ZERO;
                }
                // Disabled until the stage can turn: see local_point.
                theme::icon_button(ui, 24.0, false, false, "Rotation Tool (Shift+H)", |p, r, c| icons::paint(p, r.shrink(5.0), Icon::StageRotation, c));
            }
            let room = Rect::from_x_y_ranges(ui.cursor().left()..=right.min_rect().left() - 8.0, ui.max_rect().y_range());
            let mut go_to = None;
            ui.scope(|ui| {
                ui.set_clip_rect(room.intersect(ui.clip_rect()));
                ui.set_max_width(room.width().max(0.0));
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                // Breadcrumb: the scene, then each symbol being edited, each with its
                // glyph; clicking one goes back to it.
                let mut crumbs = vec![(Icon::Scene, ed.doc.scenes[ed.scene].name.clone())];
                crumbs.extend(ed.editing.iter().map(|t| (symbol_icon(ed, &t.symbol), t.symbol.clone())));
                let last = crumbs.len() - 1;
                for (depth, (icon, name)) in crumbs.into_iter().enumerate() {
                    if depth > 0 {
                        ui.label(RichText::new(">").size(10.0).color(theme::TEXT_DIM));
                    }
                    let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                    icons::paint(ui.painter(), r, icon, theme::TEXT_DIM);
                    let text = RichText::new(name).size(12.0).color(if depth == last { Color32::WHITE } else { theme::TEXT });
                    if ui.add(egui::Label::new(text).sense(Sense::click())).clicked() && depth < last {
                        go_to = Some(depth);
                    }
                }
            });
            if let Some(depth) = go_to {
                while ed.editing.len() > depth {
                    ed.exit_symbol();
                }
            }
        });
    });
    let rect = bar.response.rect;
    ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, theme::BORDER));
    if let Some(i) = go_scene {
        ed.go_to_scene(i);
    }
    if let Some(name) = edit_symbol {
        ed.edit_library_symbol(&name);
    }
    if let Some(action) = action {
        ed.run(action, ui.ctx());
    }
}

/// The zoom field, 64 × 22: type a percentage (8%–2000%) and press Enter, or
/// pick from its menu.
fn zoom_field(ui: &mut Ui, ed: &mut Editor, action: &mut Option<Action>) {
    let (rect, _) = ui.allocate_exact_size(vec2(64.0, 22.0), Sense::hover());
    ui.painter().rect(rect, 4.0, theme::INSET, Stroke::new(1.0, theme::OUTLINE), StrokeKind::Inside);
    let chevron = Rect::from_min_max(pos2(rect.right() - 16.0, rect.top()), rect.max);
    let menu = ui.interact(chevron, ui.id().with("zoom-presets"), Sense::click()).on_hover_text("Zoom presets");
    let color = if menu.hovered() { Color32::WHITE } else { theme::TEXT_DIM };
    icons::paint(ui.painter(), Rect::from_center_size(chevron.center(), vec2(10.0, 10.0)), Icon::ChevronDown, color);
    egui::Popup::menu(&menu).style(theme::menu_style).show(|ui| {
        for (label, act) in [("Fit in Window", Action::FitInWindow), ("Show Frame", Action::FitInWindow), ("Show All", Action::ShowAll)] {
            if theme::menu_item(ui, true, label, "").clicked() {
                *action = Some(act);
            }
        }
        ui.separator();
        for z in ZOOMS {
            let item = theme::menu_item(ui, true, &format!("{:.0}%", z * 100.0), "");
            if (ed.zoom - z).abs() < 1e-4 {
                theme::menu_check(ui, &item);
            }
            if item.clicked() {
                *action = Some(Action::Zoom(z));
            }
        }
    });
    let id = ui.id().with("zoom-text");
    let mut text = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_default();
    let field = Rect::from_min_max(rect.min + vec2(4.0, 1.0), pos2(chevron.left(), rect.bottom() - 1.0));
    let edit = egui::TextEdit::singleline(&mut text).frame(egui::Frame::NONE).horizontal_align(Align::Max).vertical_align(Align::Center);
    let resp = ui.put(field, edit);
    if !resp.has_focus() {
        text = format!("{:.0}%", ed.zoom * 100.0);
    }
    if resp.lost_focus()
        && ui.input(|i| i.key_pressed(Key::Enter))
        && let Ok(percent) = text.trim().trim_end_matches('%').trim().parse::<f32>()
    {
        *action = Some(Action::Zoom(percent / 100.0));
    }
    ui.data_mut(|d| d.insert_temp(id, text));
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Event, Key, Modifiers};

    use super::*;
    use crate::app::testkit::*;

    /// A new document with rectangles drawn from each `a` to `b`, the Selection tool picked.
    fn rectangles(rects: &[(Point, Point)]) -> Harness<'static, Editor> {
        let mut h = blank();
        h.state_mut().tool = Tool::Rectangle;
        for &(a, b) in rects {
            mouse_drag(&mut h, a, b);
        }
        h.state_mut().tool = Tool::Selection;
        h
    }

    /// Runs `gesture` with `mods` held.
    fn holding(h: &mut Harness<'static, Editor>, mods: Modifiers, gesture: impl FnOnce(&mut Harness<'static, Editor>)) {
        h.event(Event::ModifiersChanged(mods));
        gesture(h);
        h.event(Event::ModifiersChanged(Modifiers::NONE));
        h.step();
    }

    fn element_bounds(ed: &Editor, r: ElementRef) -> hit::Rect {
        hit::bounds(&ed.doc, ed.timeline(), ed.frame, &[r]).expect("on stage")
    }

    fn el(element: usize) -> ElementRef {
        ElementRef { layer: 0, keyframe: 0, element }
    }

    /// Selects what's at `at` and converts it to a symbol (F8, Enter).
    fn convert(h: &mut Harness<'static, Editor>, at: Point) {
        mouse_click(h, at);
        h.key_press(Key::F8);
        h.step();
        h.step();
        h.key_press(Key::Enter);
        h.step();
        assert!(h.state().convert.is_none());
    }

    fn double_click(h: &mut Harness<'static, Editor>, at: Point) {
        // Long enough after any earlier click that this isn't a triple click.
        h.run_steps(40);
        let p = on_stage(h, at);
        h.hover_at(p);
        h.step();
        for _ in 0..2 {
            h.drag_at(p);
            h.drop_at(p);
            h.step();
        }
    }

    /// Presses at screen point `a`, moves to `b` and releases there.
    fn drag_between(h: &mut Harness<'static, Editor>, a: egui::Pos2, b: egui::Pos2) {
        h.hover_at(a);
        h.step();
        h.drag_at(a);
        h.step();
        h.hover_at(a.lerp(b, 0.5));
        h.step();
        h.hover_at(b);
        h.step();
        h.drop_at(b);
        h.step();
    }

    /// The editor on a new document, with a GPU renderer for `screenshot`.
    fn gpu() -> Harness<'static, Editor> {
        let mut h = Harness::builder().with_size(egui::vec2(1600.0, 900.0)).with_step_dt(1.0 / 60.0).wgpu().build_eframe(|cc| Editor::new(&cc.egui_ctx, None));
        h.state_mut().new_document(550.0, 400.0, 24.0);
        h.run();
        h
    }

    #[test]
    fn shift_click_toggles_an_element() {
        let mut h = rectangles(&[((100.0, 100.0), (200.0, 150.0)), ((300.0, 100.0), (400.0, 150.0))]);
        mouse_click(&mut h, (150.0, 125.0));
        assert_eq!(h.state().selection, [el(0)]);
        holding(&mut h, Modifiers::SHIFT, |h| mouse_click(h, (350.0, 125.0)));
        assert_eq!(h.state().selection, [el(0), el(1)], "Shift+click adds");
        holding(&mut h, Modifiers::SHIFT, |h| mouse_click(h, (150.0, 125.0)));
        assert_eq!(h.state().selection, [el(1)], "Shift+click on a selected element removes it");
        mouse_click(&mut h, (150.0, 125.0));
        assert_eq!(h.state().selection, [el(0)], "a plain click replaces the selection");
    }

    #[test]
    fn alt_drag_moves_a_copy() {
        let mut h = rectangles(&[((100.0, 100.0), (200.0, 150.0))]);
        let ctx = h.ctx.clone();
        holding(&mut h, Modifiers::ALT, |h| mouse_drag(h, (150.0, 125.0), (150.0, 225.0)));
        let ed = h.state();
        assert_eq!(ed.all_elements().len(), 2, "a copy");
        assert!((element_bounds(ed, el(0)).min.1 - 99.5).abs() < 1.0, "the original stays");
        assert!((element_bounds(ed, el(1)).min.1 - 199.5).abs() < 1.0, "the copy moved");
        assert_eq!(ed.selection, [el(1)], "the copy is selected");
        h.state_mut().run(Action::Undo, &ctx);
        assert_eq!(h.state().all_elements().len(), 1, "copying and moving are one undo step");
    }

    #[test]
    fn shift_drag_moves_at_45_degree_steps() {
        let mut h = rectangles(&[((100.0, 100.0), (200.0, 150.0))]);
        holding(&mut h, Modifiers::SHIFT, |h| mouse_drag(h, (150.0, 125.0), (210.0, 135.0)));
        let b = element_bounds(h.state(), el(0));
        assert!((b.min.0 - 159.5).abs() < 1.0 && (b.min.1 - 99.5).abs() < 1e-3, "straight across: {b:?}");
        holding(&mut h, Modifiers::SHIFT, |h| mouse_drag(h, (200.0, 125.0), (238.0, 165.0)));
        let b = element_bounds(h.state(), el(0));
        assert!((b.min.0 - 198.5).abs() < 1.5 && (b.min.1 - 138.5).abs() < 1.5, "on the diagonal: {b:?}");
    }

    #[test]
    fn arrow_keys_nudge_the_selection() {
        let mut h = rectangles(&[((100.0, 100.0), (200.0, 150.0))]);
        mouse_click(&mut h, (150.0, 125.0));
        let before = element_bounds(h.state(), el(0));
        h.key_press(Key::ArrowRight);
        h.key_press_modifiers(Modifiers::SHIFT, Key::ArrowDown);
        h.step();
        let after = element_bounds(h.state(), el(0));
        assert!((after.min.0 - before.min.0 - 1.0).abs() < 1e-3 && (after.min.1 - before.min.1 - 10.0).abs() < 1e-3, "{after:?}");
    }

    #[test]
    fn shift_draws_squares_and_alt_from_the_centre() {
        let mut h = rectangles(&[]);
        h.state_mut().tool = Tool::Rectangle;
        holding(&mut h, Modifiers::SHIFT, |h| mouse_drag(h, (100.0, 100.0), (200.0, 140.0)));
        let b = element_bounds(h.state(), el(0));
        let (w, ht) = (b.max.0 - b.min.0, b.max.1 - b.min.1);
        assert!((w - ht).abs() < 1e-3 && (w - 101.0).abs() < 1.0, "a square: {b:?}");
        h.state_mut().tool = Tool::Oval;
        holding(&mut h, Modifiers::ALT, |h| mouse_drag(h, (300.0, 300.0), (350.0, 320.0)));
        let b = element_bounds(h.state(), el(1));
        assert!((b.min.0 - 249.5).abs() < 1.0 && (b.min.1 - 279.5).abs() < 1.0 && (b.max.1 - 320.5).abs() < 1.0, "around the start: {b:?}");
    }

    #[test]
    fn arrows_round_a_rectangle_while_dragging() {
        let mut h = rectangles(&[]);
        h.state_mut().tool = Tool::Rectangle;
        let (a, b) = (on_stage(&h, (100.0, 100.0)), on_stage(&h, (200.0, 200.0)));
        h.hover_at(a);
        h.step();
        h.drag_at(a);
        h.step();
        h.hover_at(b);
        h.step();
        for _ in 0..5 {
            h.key_press(Key::ArrowUp);
        }
        h.key_press(Key::ArrowDown);
        h.step();
        assert_eq!(h.state().corner_radius, 4.0);
        h.drop_at(b);
        h.step();
        let Some(Element::Shape(s)) = h.state().element(el(0)) else { panic!("a rectangle") };
        assert!(s.edges[0].paths[0].segs.iter().any(|s| matches!(s, Seg::Quad(..))), "rounded corners");
    }

    #[test]
    fn dragging_an_unselected_edge_bends_it() {
        let mut h = rectangles(&[((100.0, 100.0), (200.0, 150.0))]);
        let ctx = h.ctx.clone();
        let badge = |h: &Harness<'_, Editor>, p: Point, alt: bool| selection_badge(h.state(), p, alt);
        assert_eq!(badge(&h, (150.0, 100.0), false), Cursor::Arc);
        assert_eq!(badge(&h, (200.0, 150.0), false), Cursor::Corner);
        assert_eq!(badge(&h, (150.0, 125.0), false), Cursor::Move);
        assert_eq!(badge(&h, (150.0, 125.0), true), Cursor::Copy);
        assert_eq!(badge(&h, (300.0, 300.0), false), Cursor::Arrow);
        mouse_drag(&mut h, (150.0, 100.0), (150.0, 80.0));
        let b = element_bounds(h.state(), el(0));
        assert!(b.min.1 < 95.0 && (b.min.0 - 99.5).abs() < 1.0 && (b.max.1 - 150.5).abs() < 1.0, "bent up: {b:?}");
        h.state_mut().run(Action::Undo, &ctx);
        assert!((element_bounds(h.state(), el(0)).min.1 - 99.5).abs() < 1.0, "one undo step");
        // Selected, the same drag would move it.
        mouse_click(&mut h, (150.0, 125.0));
        assert_eq!(badge(&h, (150.0, 100.0), false), Cursor::Move);
    }

    #[test]
    fn marquee_takes_touched_shapes_and_enclosed_objects() {
        let mut h = rectangles(&[((100.0, 100.0), (200.0, 150.0)), ((300.0, 100.0), (400.0, 150.0))]);
        convert(&mut h, (350.0, 125.0));
        mouse_click(&mut h, (500.0, 300.0));
        assert!(h.state().selection.is_empty());
        // Half over each: the shape is taken, the instance isn't.
        mouse_drag(&mut h, (150.0, 50.0), (350.0, 200.0));
        assert_eq!(h.state().selection, [el(0)]);
        mouse_drag(&mut h, (50.0, 50.0), (450.0, 200.0));
        assert_eq!(h.state().selection.len(), 2, "both, enclosed");
        // Shift adds; locked layers can't be selected.
        mouse_click(&mut h, (500.0, 300.0));
        mouse_drag(&mut h, (50.0, 50.0), (250.0, 200.0));
        holding(&mut h, Modifiers::SHIFT, |h| mouse_drag(h, (250.0, 50.0), (450.0, 200.0)));
        assert_eq!(h.state().selection.len(), 2);
        h.state_mut().timeline_mut().layers[0].set_locked(true);
        mouse_drag(&mut h, (50.0, 50.0), (450.0, 200.0));
        assert!(h.state().selection.is_empty());
    }

    #[test]
    fn rulers_grid_and_guides() {
        let mut h = rectangles(&[]);
        let stage = h.state().stage_rect;
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT | Modifiers::ALT, Key::R);
        h.key_press_modifiers(Modifiers::COMMAND, Key::Quote);
        h.step();
        h.step();
        assert!(h.state().stage.rulers && h.state().stage.grid);
        assert_eq!(h.state().stage_rect, stage.translate(egui::vec2(9.0, 9.0)), "the rulers take 18 points; the stage stays centred");
        // Out of the top ruler, onto the stage's y = 50.
        let ruler = egui::pos2(on_stage(&h, (275.0, 0.0)).x, h.state().stage_rect.center().y - h.state().view_size.y / 2.0 - 9.0);
        let to = on_stage(&h, (100.0, 50.0));
        drag_between(&mut h, ruler, to);
        assert_eq!(h.state().stage.guides, [Guide::Y(50.0)]);
        // The Selection tool moves it, unless guides are locked; off the view removes it.
        mouse_drag(&mut h, (300.0, 50.0), (300.0, 80.0));
        assert_eq!(h.state().stage.guides, [Guide::Y(80.0)]);
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, Key::Semicolon);
        h.step();
        mouse_drag(&mut h, (300.0, 80.0), (300.0, 120.0));
        assert_eq!(h.state().stage.guides, [Guide::Y(80.0)], "locked: that was a marquee");
        h.state_mut().stage.toggle(Toggle::LockGuides);
        let from = on_stage(&h, (300.0, 80.0));
        drag_between(&mut h, from, ruler);
        assert!(h.state().stage.guides.is_empty());
    }

    #[test]
    fn scrollbars_pan_the_view() {
        let mut h = rectangles(&[]);
        let view = Rect::from_center_size(h.state().stage_rect.center(), h.state().view_size);
        let [_, (bottom, rate)] = thumbs(h.state(), view, 1.0)[..] else { panic!("two thumbs") };
        drag_between(&mut h, bottom.center(), bottom.center() + egui::vec2(40.0, 0.0));
        assert!((h.state().pan.x + 40.0 * rate).abs() < 2.0 * rate, "dragged right, the stage goes left: {:?}", h.state().pan);
        // A view too short for a scrollbar has none, rather than a pan of NaN.
        let short = Rect::from_min_size(view.min, egui::vec2(view.width(), 13.0));
        assert!(thumbs(h.state(), short, 1.0).is_empty());
    }

    #[test]
    fn view_toggles_wait_while_typing() {
        let mut h = rectangles(&[]);
        h.state_mut().tool = Tool::Text;
        mouse_click(&mut h, (100.0, 100.0));
        h.step();
        assert!(h.ctx.egui_wants_keyboard_input(), "typing");
        h.key_press_modifiers(Modifiers::COMMAND, Key::Quote);
        h.step();
        assert!(!h.state().stage.grid);
    }

    #[test]
    fn every_way_out_of_a_symbol() {
        use egui_kittest::kittest::Queryable;
        let mut h = rectangles(&[((100.0, 100.0), (200.0, 150.0))]);
        convert(&mut h, (150.0, 125.0));
        let enter = |h: &mut Harness<'static, Editor>| {
            double_click(h, (150.0, 125.0));
            assert_eq!(h.state().editing.len(), 1, "double-click edits in place");
        };
        enter(&mut h);
        double_click(&mut h, (400.0, 300.0));
        assert!(h.state().editing.is_empty(), "double-click outside");
        enter(&mut h);
        h.key_press_modifiers(Modifiers::COMMAND, Key::E);
        h.step();
        assert!(h.state().editing.is_empty(), "Ctrl+E: Edit Document");
        mouse_click(&mut h, (150.0, 125.0));
        h.key_press_modifiers(Modifiers::COMMAND, Key::E);
        h.step();
        assert_eq!(h.state().editing.len(), 1, "Ctrl+E on an instance: Edit Symbols");
        h.get_by_label("Scene 1").click();
        h.run();
        assert!(h.state().editing.is_empty(), "the scene's crumb");
        // Edit Scene's menu, picking the scene open: the playhead comes back where it was.
        h.key_press(Key::F5);
        h.key_press(Key::F5);
        h.step();
        h.state_mut().set_frame(2);
        enter(&mut h);
        h.state_mut().go_to_scene(0);
        assert_eq!((h.state().editing.len(), h.state().frame), (0, 2), "Edit Scene");
    }

    /// The onion skin draws the scene shown, and the symbol being edited.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn onion_skin_in_each_scene_and_symbol() {
        let pixel = |h: &mut Harness<'static, Editor>, p: Point| {
            let at = on_stage(h, p) * h.ctx.pixels_per_point();
            h.render().expect("render").get_pixel(at.x as u32, at.y as u32).0
        };
        let mut h = gpu();
        let ctx = h.ctx.clone();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        h.state_mut().run(Action::InsertScene, &ctx);
        h.state_mut().onion = true;
        h.run();
        h.state_mut().run(Action::GoToScene(crate::app::Go::First), &ctx);
        h.run();
        assert_eq!(pixel(&mut h, (150.0, 125.0)), [0x00, 0x99, 0xFF, 0xFF], "Scene 1's rectangle, not empty Scene 2");

        // A new symbol, edited on its own, two frames long: from the second, the first shows faintly.
        h.key_press_modifiers(Modifiers::COMMAND, Key::F8);
        h.step();
        h.step();
        h.key_press(Key::Enter);
        h.step();
        mouse_drag(&mut h, (100.0, 250.0), (200.0, 300.0));
        h.key_press(Key::F7);
        h.step();
        mouse_drag(&mut h, (300.0, 250.0), (400.0, 300.0));
        h.run();
        assert_eq!((h.state().editing.len(), h.state().frame), (1, 1));
        assert_ne!(pixel(&mut h, (150.0, 275.0)), [0xFF; 4], "frame 1's rectangle, as a ghost");
    }

    /// A long breadcrumb is cut short before the edit bar's right-hand group,
    /// which keeps its room (docs/animate-ui.md §4.4).
    #[test]
    fn a_long_breadcrumb_stops_short_of_the_right_group() {
        use egui_kittest::kittest::Queryable;
        let mut h = blank();
        let ed = h.state_mut();
        for name in ["character_walk_cycle_body", "left_arm_upper_rig"] {
            minnamate_core::symbols::new_symbol(&mut ed.doc, name, doc::SymbolKind::MovieClip).unwrap();
        }
        ed.edit_library_symbol("character_walk_cycle_body");
        let transform = minnamate_core::doc::Matrix::IDENTITY;
        ed.editing.push(crate::app::EditTarget { symbol: "left_arm_upper_rig".into(), transform, frame: 0, layer: 0, isolated: true });
        h.set_size(egui::vec2(960.0, 600.0));
        h.run();
        let group = h.get_by_label("Rotation Tool (Shift+H)").rect();
        for (text, shown) in painted_texts(&h) {
            if shown.is_positive() && group.y_range().contains(shown.center().y) && shown.left() < group.left() {
                assert!(shown.right() <= group.left(), "{text:?} runs under the right-hand group");
            }
        }
    }

    /// The marquee shows on the dark pasteboard as well as on the stage: a
    /// black line with a light rim (docs/animate-ui-interaction.md §1.0).
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn the_marquee_shows_on_the_pasteboard() {
        let mut h = gpu();
        h.state_mut().tool = Tool::Selection;
        let (a, b) = (on_stage(&h, (-150.0, 100.0)), on_stage(&h, (-30.0, 200.0)));
        h.hover_at(a);
        h.step();
        h.drag_at(a);
        h.step();
        h.hover_at(b);
        h.step();
        h.step();
        assert!(matches!(h.state().drag, Some(Drag::Marquee { .. })));
        let image = rendered(&mut h);
        // Across its top edge, halfway along.
        let x = ((a.x + b.x) / 2.0) as usize;
        let across: Vec<u8> = (a.y as usize - 3..=a.y as usize + 3).map(|y| image[(x, y)].r()).collect();
        assert!(across.iter().any(|&c| c < 0x10) && across.iter().any(|&c| c >= 0x7A), "{across:X?}");
        h.drop_at(b);
        h.step();
    }

    /// Writes target/screenshots/stage_selection.png: raw shapes selected (white
    /// dots, a hairline's band) and an instance (its box, registration "+" and
    /// transformation point).
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn stage_selection_screenshot() {
        let mut h = gpu();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (60.0, 80.0), (240.0, 220.0));
        mouse_drag(&mut h, (320.0, 120.0), (470.0, 260.0));
        (h.state_mut().tool, h.state_mut().stroke_width) = (Tool::Line, 0.25);
        mouse_drag(&mut h, (80.0, 300.0), (240.0, 360.0));
        h.state_mut().tool = Tool::Selection;
        convert(&mut h, (400.0, 190.0));
        mouse_click(&mut h, (150.0, 150.0));
        holding(&mut h, Modifiers::SHIFT, |h| mouse_click(h, (400.0, 190.0)));
        holding(&mut h, Modifiers::SHIFT, |h| mouse_click(h, (160.0, 330.0)));
        assert_eq!(h.state().selection.len(), 3);
        h.run();
        screenshot(&mut h, "stage_selection");
    }

    /// Writes target/screenshots/stage_rulers.png: rulers, grid and a guide each way.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn stage_rulers_screenshot() {
        let mut h = gpu();
        h.state_mut().tool = Tool::Oval;
        mouse_drag(&mut h, (100.0, 100.0), (250.0, 220.0));
        let ed = h.state_mut();
        (ed.stage.rulers, ed.stage.grid, ed.tool) = (true, true, Tool::Selection);
        ed.stage.guides = vec![Guide::X(100.0), Guide::Y(220.0)];
        h.run();
        screenshot(&mut h, "stage_rulers");
    }

    /// Writes target/screenshots/stage_edit_in_place.png: inside a symbol, the rest dimmed.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn stage_edit_in_place_screenshot() {
        let mut h = gpu();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (60.0, 80.0), (240.0, 220.0));
        (h.state_mut().tool, h.state_mut().fill_color) = (Tool::Oval, Color32::from_rgb(0xE0, 0x40, 0x40));
        mouse_drag(&mut h, (300.0, 120.0), (470.0, 300.0));
        h.state_mut().tool = Tool::Selection;
        convert(&mut h, (385.0, 210.0));
        double_click(&mut h, (385.0, 210.0));
        assert_eq!(h.state().editing.len(), 1);
        h.run();
        screenshot(&mut h, "stage_edit_in_place");
    }
}
