//! Motion and shape tweens in the editor (docs/animate-ui.md §5.6, §5.8, §7.2.5):
//! a motion tween made of what isn't a symbol yet, the frame menu's property
//! keyframes and their diamonds in the timeline, the Frame tab's tween settings,
//! and on stage the selected target's motion path and a shape tween's hints,
//! dragged with the Selection tool. Moving or transforming a target inside its
//! span keys it (motion::edit_shown, which the stage's edits go through).

use std::collections::BTreeSet;

use eframe::egui::{self, Color32, Id, Painter, Pos2, Rect, Response, RichText, Stroke, Ui, pos2, vec2};
use minnamate_core::doc::{Element, Frame, LayerKind, Matrix, Point, Rotate, SymbolKind, Tween};
use minnamate_core::hit::{self, ElementRef};
use minnamate_core::motion::{self, Kind, Motion};
use minnamate_core::{display, shape_tween, symbols};

use crate::app::{Action, Editor};
use crate::cursors::Cursor;
use crate::icons::Icon;
use crate::stage::{local_point, screen_point};
use crate::theme;
use crate::tools::Tool;
use crate::widgets::{self, Number, WORD_W};

/// The properties the frame menu keys and shows, in its order.
const KINDS: [(Kind, &str); 5] = [(Kind::Position, "Position"), (Kind::Scale, "Scale"), (Kind::Skew, "Skew"), (Kind::Rotation, "Rotation"), (Kind::Color, "Color")];
/// Where egui keeps: the spans waiting on the convert question, View Keyframes, the
/// timeline tween button's kind and what the Selection tool drags (`Grab`).
const CONVERT: &str = "tweens-convert";
const VIEW: &str = "tweens-view-keyframes";
const BUTTON: &str = "tweens-button";
const DRAG: &str = "tweens-drag";

/// Create Motion Tween on the selected frames' spans. Spans holding what can't be
/// tweened as it is (shapes, groups, several objects) wait on a question: turn it
/// into a symbol first, as Animate asks.
pub fn create_motion_tween(ed: &mut Editor, ctx: &egui::Context) {
    crate::timeline::command(ed, Action::CreateMotionTween);
    let tl = ed.timeline();
    let spans: BTreeSet<(usize, u32)> = ed
        .tl
        .sel
        .iter()
        .filter_map(|&(l, f)| {
            let layer = tl.layers.get(l).filter(|l| l.kind == LayerKind::Normal)?;
            let key = &layer.frames[layer.keyframe_at(f)?];
            (!key.elements.is_empty() && !matches!(key.tween, Tween::Motion(_))).then_some((l, key.index))
        })
        .collect();
    if !spans.is_empty() {
        ctx.data_mut(|d| d.insert_temp(Id::new(CONVERT), spans.into_iter().collect::<Vec<_>>()));
    }
}

/// The question Create Motion Tween asks about what isn't a symbol yet. OK turns each
/// span's contents into a movie clip, as "Symbol N", and tweens it: one undo step.
pub fn dialogs(ctx: &egui::Context, ed: &mut Editor) {
    let id = Id::new(CONVERT);
    let Some(spans) = ctx.data(|d| d.get_temp::<Vec<(usize, u32)>>(id)) else { return };
    let mut answer = ctx.input(|i| i.key_pressed(egui::Key::Enter)).then_some(true);
    let modal = egui::Modal::new(id).show(ctx, |ui| {
        ui.set_width(400.0);
        ui.label(RichText::new("Motion Tween").heading().color(theme::TEXT));
        ui.add_space(6.0);
        ui.label("A motion tween moves a symbol instance or a text field. Convert what's selected into a movie clip symbol, and tween that?");
        ui.add_space(12.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            if ui.button("  Cancel  ").clicked() {
                answer = Some(false);
            }
            if theme::cta_button(ui, "    OK    ").clicked() {
                answer = Some(true);
            }
        });
    });
    if modal.should_close() {
        answer = answer.or(Some(false));
    }
    let Some(ok) = answer else { return };
    ctx.data_mut(|d| d.remove::<Vec<(usize, u32)>>(id));
    if !ok {
        return;
    }
    // ponytail: as Convert to Symbol, on a scene's timeline only.
    if !ed.editing.is_empty() {
        return ed.log("Converting to a symbol works on the scene; leave the symbol first.");
    }
    let (scene, fps) = (ed.scene, ed.doc.frame_rate);
    let mut doc = ed.doc.clone();
    let mut made = Vec::new();
    for (l, f) in spans {
        let tl = &doc.scenes[scene];
        let Some(k) = tl.layers.get(l).and_then(|layer| layer.keyframe_at(f)) else { continue };
        let refs: Vec<ElementRef> = (0..tl.layers[l].frames[k].elements.len()).map(|element| ElementRef { layer: l, keyframe: k, element }).collect();
        let Some(b) = hit::bounds(&doc, tl, f, &refs) else { continue };
        let name = symbols::next_symbol_name(&doc);
        match symbols::convert_to_symbol(&mut doc, scene, f, &refs, &name, SymbolKind::MovieClip, b.min) {
            Ok(r) => {
                doc.scenes[scene].layers[l].create_motion_tween(f, fps);
                made.push(r);
            }
            Err(e) => {
                ed.error = Some(e.to_string());
                return;
            }
        }
    }
    if !made.is_empty() {
        ed.checkpoint();
        ed.doc = doc;
        ed.selection = made;
    }
}

/// The motion tween on layer `l` holding frame `f`, and the frame into its span.
fn spot(ed: &Editor, (l, f): (usize, u32)) -> Option<(&Motion, u32)> {
    let layer = ed.timeline().layers.get(l)?;
    let key = &layer.frames[layer.keyframe_at(f)?];
    match &key.tween {
        Tween::Motion(m) => Some((m, f - key.index)),
        _ => None,
    }
}

/// One kind, or all of them.
fn kinds(kind: Option<Kind>) -> Vec<Kind> {
    kind.map_or(Kind::ALL.to_vec(), |k| vec![k])
}

/// Whether Insert Keyframe or Clear Keyframe by property can act on `cells`: frames of a
/// motion tween, Clear only where that's keyed. Split Motion: one frame of one, past its first.
pub fn keyed(ed: &Editor, cells: &BTreeSet<(usize, u32)>, action: Action) -> bool {
    cells.iter().any(|&cell| match (spot(ed, cell), action) {
        (Some((m, at)), Action::ClearPropertyKeyframe(kind)) => kinds(kind).into_iter().any(|k| m.frames(k).contains(&at)),
        (Some((_, at)), Action::SplitMotion) => cells.len() == 1 && at > 0,
        (spot, _) => spot.is_some(),
    })
}

/// Insert Keyframe and Clear Keyframe by property, on the selected frames of motion tweens.
pub fn property_keys(ed: &mut Editor, action: Action, cells: &BTreeSet<(usize, u32)>) {
    let objects = ed.selection.clone();
    ed.edit_timeline(|_, tl| {
        for &(l, f) in cells {
            let Some(key) = tl.layers.get_mut(l).and_then(|layer| layer.keyframe_at(f).map(|k| &mut layer.frames[k])) else { continue };
            let own = match key.elements.first() {
                Some(Element::Instance(i)) => i.color,
                _ => minnamate_core::doc::ColorTransform::IDENTITY,
            };
            let at = f - key.index;
            let Tween::Motion(m) = &mut key.tween else { continue };
            match action {
                Action::InsertPropertyKeyframe(kind) => kinds(kind).into_iter().for_each(|k| m.key(k, at, own)),
                Action::ClearPropertyKeyframe(kind) => kinds(kind).into_iter().for_each(|k| m.clear(k, at)),
                _ => {}
            }
        }
        objects
    });
}

/// The frame menu's keyframe block inside a motion tween: Insert Keyframe and Clear
/// Keyframe by property, Insert Blank Keyframe, View Keyframes. Returns the command picked.
pub fn keyframe_menu(ui: &mut Ui, ed: &Editor) -> Option<Action> {
    let mut picked = by_kind(ui, ed, "Insert Keyframe", Action::InsertPropertyKeyframe);
    if theme::menu_item(ui, crate::timeline::available(ed, Action::InsertBlankKeyframe), "Insert Blank Keyframe", "").clicked() {
        picked = Some(Action::InsertBlankKeyframe);
    }
    picked = picked.or(by_kind(ui, ed, "Clear Keyframe", Action::ClearPropertyKeyframe));
    ui.menu_button("View Keyframes", |ui| {
        let mut shown = shown_kinds(ui.ctx());
        for (i, (_, name)) in KINDS.iter().enumerate() {
            if theme::menu_toggle(ui, true, shown[i], name).clicked() {
                shown[i] = !shown[i];
            }
        }
        theme::menu_toggle(ui, false, false, "Filter");
        ui.separator();
        for (name, all) in [("All", true), ("None", false)] {
            if theme::menu_item(ui, true, name, "").clicked() {
                shown = [all; 5];
            }
        }
        ui.data_mut(|d| d.insert_temp(Id::new(VIEW), shown));
    });
    picked
}

/// The frame menu's motion blocks inside a motion tween, in place of the normal Copy Motion
/// block: Copy and Paste Motion, the properties' copy and paste, Split and Join Motions.
pub fn motion_menu(ui: &mut Ui, ed: &Editor) -> Option<Action> {
    // ponytail: copying and pasting motion or properties, and joining spans, aren't built.
    let off = |ui: &mut Ui, labels: &[&str]| labels.iter().for_each(|label| _ = theme::menu_item(ui, false, label, ""));
    off(ui, &["Copy Motion", "Paste Motion"]);
    ui.separator();
    off(ui, &["Copy Properties", "Paste Properties", "Paste Properties Special..."]);
    ui.separator();
    let split = theme::menu_item(ui, crate::timeline::available(ed, Action::SplitMotion), "Split Motion", "").clicked();
    off(ui, &["Join Motions"]);
    split.then_some(Action::SplitMotion)
}

/// Insert Keyframe's or Clear Keyframe's submenu: one property, or all.
fn by_kind(ui: &mut Ui, ed: &Editor, label: &str, make: fn(Option<Kind>) -> Action) -> Option<Action> {
    let mut picked = None;
    let mut item = |ui: &mut Ui, name: &str, kind: Option<Kind>| {
        if theme::menu_item(ui, crate::timeline::available(ed, make(kind)), name, "").clicked() {
            picked = Some(make(kind));
        }
    };
    ui.menu_button(label, |ui| {
        for (kind, name) in KINDS {
            item(ui, name, Some(kind));
        }
        // ponytail: filters aren't modelled, nor keyed.
        theme::menu_item(ui, false, "Filter", "");
        ui.separator();
        item(ui, "All", None);
    });
    picked
}

/// Which of `KINDS` the timeline shows diamonds for (View Keyframes).
// ponytail: one setting for the whole timeline, not saved; Animate keeps it per span, in the file.
fn shown_kinds(ctx: &egui::Context) -> [bool; 5] {
    ctx.data(|d| d.get_temp(Id::new(VIEW))).unwrap_or([true; 5])
}

/// A motion tween span's marks in the timeline: a diamond at each frame past the first
/// with a keyframe of a kind View Keyframes shows, and a thin light edge closing the span.
pub fn paint_span(p: &Painter, key: &Frame, x_of: &dyn Fn(u32) -> f32, cell_w: f32, dot_y: f32, span: Rect) {
    let Tween::Motion(m) = &key.tween else { return };
    let shown = shown_kinds(p.ctx());
    let keyed: BTreeSet<u32> = KINDS.iter().zip(shown).filter(|(_, on)| *on).flat_map(|((kind, _), _)| m.frames(*kind)).collect();
    for f in keyed.into_iter().filter(|&f| f > 0 && f < key.duration) {
        let (c, r) = (pos2(x_of(key.index + f) + cell_w / 2.0, dot_y), 2.8);
        let diamond = vec![c - vec2(0.0, r), c + vec2(r, 0.0), c + vec2(0.0, r), c - vec2(r, 0.0)];
        p.add(egui::Shape::convex_polygon(diamond, theme::KEYFRAME, Stroke::NONE));
    }
    p.vline(span.right() - 0.5, span.y_range(), Stroke::new(1.0, Color32::from_rgb(0xD8, 0xCC, 0x9A)));
}

/// Whether a classic or shape tween plays to the next keyframe; one that doesn't is drawn
/// broken, dashed.
pub fn plays(key: &Frame, next: Option<&Frame>) -> bool {
    match key.tween {
        Tween::Classic { .. } => display::tween_at(key, next, 0).is_some(),
        Tween::Shape { .. } => shape_tween::valid(key, next),
        _ => next.is_some_and(|n| n.index == key.index + key.duration),
    }
}

/// The timeline's tween button: what it creates (the kind last picked from its menu),
/// its glyph and its tooltip.
pub fn tween_button(ctx: &egui::Context) -> (Action, Icon, &'static str) {
    match ctx.data(|d| d.get_temp::<Action>(Id::new(BUTTON))) {
        Some(Action::CreateMotionTween) => (Action::CreateMotionTween, Icon::MotionTween, "Create Motion Tween"),
        Some(Action::CreateShapeTween) => (Action::CreateShapeTween, Icon::ShapeTween, "Create Shape Tween"),
        _ => (Action::CreateClassicTween, Icon::TlClassicTween, "Create Classic Tween"),
    }
}

/// The tween button's menu: the kind it creates from now on.
pub fn pick_tween(ctx: &egui::Context, action: Action) {
    ctx.data_mut(|d| d.insert_temp(Id::new(BUTTON), action));
}

/// Properties > Frame, Tweening, for a motion tween: its ease, turns and orientation.
pub fn motion_settings(ui: &mut Ui, m: &mut Motion, let_go: &mut bool) {
    ease_row(ui, &mut m.ease, let_go);
    widgets::row(ui, |ui| {
        widgets::label(ui, "Rotate", WORD_W);
        let (way, mut times) = match m.rotate {
            Rotate::Auto => (0, 1),
            Rotate::Cw(n) => (1, n),
            Rotate::Ccw(n) => (2, n),
        };
        let ways = [(0, "None", true), (1, "CW", true), (2, "CCW", true)];
        let way = widgets::pick(ui, "Rotate", way, &ways, 80.0).unwrap_or(way);
        if way > 0 {
            let mut t = times as f32;
            let field = ui.add(Number::new("Rotations", &mut t, 0.0..=99.0).speed(0.1).suffix(" times").width(64.0));
            if field.changed() {
                times = t.round() as u32;
            }
            *let_go |= field.drag_stopped() || field.lost_focus();
        }
        m.rotate = match way {
            1 => Rotate::Cw(times),
            2 => Rotate::Ccw(times),
            _ => Rotate::Auto,
        };
    });
    widgets::row(ui, |ui| *let_go |= widgets::checkbox(ui, &mut m.orient, "Orient to path").changed());
    // ponytail: graphic symbols in a motion tween play on as in any span; syncing isn't modelled.
    ui.add_enabled_ui(false, |ui| widgets::row(ui, |ui| widgets::checkbox(ui, &mut true, "Sync graphic symbols")));
}

/// Properties > Frame, Tweening, for a shape tween: its ease and blend.
pub fn shape_settings(ui: &mut Ui, ease: &mut f32, angular: &mut bool, let_go: &mut bool) {
    ease_row(ui, ease, let_go);
    widgets::row(ui, |ui| {
        widgets::label(ui, "Blend", WORD_W);
        let blends = [(false, "Distributive", true), (true, "Angular", true)];
        if let Some(a) = widgets::pick(ui, "Blend", *angular, &blends, ui.available_width() - 4.0) {
            (*angular, *let_go) = (a, true);
        }
    });
}

fn ease_row(ui: &mut Ui, ease: &mut f32, let_go: &mut bool) {
    widgets::row(ui, |ui| {
        widgets::label(ui, "Ease", WORD_W);
        let mut percent = (*ease * 100.0).round();
        let field = ui.add(Number::new("Ease", &mut percent, -100.0..=100.0));
        if field.changed() {
            *ease = percent / 100.0;
        }
        *let_go |= field.drag_stopped() || field.lost_focus();
    });
}

/// Sets instance `r`'s color effect, or keys it where the playhead is inside its motion tween.
pub fn set_color(ed: &mut Editor, r: ElementRef, c: minnamate_core::doc::ColorTransform) {
    let frame = ed.frame;
    motion::edit_shown(ed.timeline_mut(), r, frame, |el| {
        if let Element::Instance(i) = el {
            i.color = c;
        }
    });
}

/// Element `r` as it shows at the current frame, when it's a motion tween's target.
pub fn shown_target(ed: &Editor, r: ElementRef) -> Option<Element> {
    let key = ed.timeline().layers.get(r.layer)?.frames.get(r.keyframe)?;
    motion::target_at(key, ed.frame.checked_sub(key.index)?).filter(|_| r.element == 0)
}

/// The matrix of element `r` where it shows at the current frame, when a motion tween moves it.
pub fn shown_matrix(ed: &Editor, r: ElementRef) -> Option<Matrix> {
    match shown_target(ed, r)? {
        Element::Text(t) => Some(t.matrix),
        Element::Instance(i) => Some(i.matrix),
        _ => None,
    }
}

/// The selected motion tween targets showing now, with their keyframes.
fn targets(ed: &Editor) -> Vec<(ElementRef, &Frame)> {
    let tl = ed.timeline();
    let tweening = ed.selection.iter().filter(|&&r| motion::tweening(tl, r, ed.frame));
    tweening.map(|&r| (r, &tl.layers[r.layer].frames[r.keyframe])).collect()
}

/// Over the stage: each selected target's motion path, and the shape hints of the tween
/// at the playhead.
pub fn paint(p: &Painter, ed: &Editor, stage: Rect) {
    // A thin line in the layer's color through where the transformation point is at each
    // frame, a dot at each frame, and a ring where a position is keyed.
    for (r, key) in targets(ed) {
        let Tween::Motion(m) = &key.tween else { continue };
        let color = crate::timeline::layer_color(&ed.timeline().layers[r.layer], r.layer);
        let points: Vec<Pos2> = motion::path_points(key).into_iter().map(|q| screen_point(ed, stage, q)).collect();
        p.add(egui::Shape::line(points.clone(), Stroke::new(1.0, color)));
        let keyed = m.frames(Kind::Position);
        for (f, &at) in points.iter().enumerate() {
            if keyed.contains(&(f as u32)) {
                p.circle(at, 3.5, Color32::WHITE, Stroke::new(1.5, color));
            } else {
                p.circle_filled(at, 2.0, color);
            }
        }
    }
    // A letter in a circle, red until it sits on an outline, then yellow on the first
    // keyframe and green on the last.
    let Some((k, end)) = hinted(ed).filter(|_| ed.stage.shape_hints) else { return };
    let layer = &ed.timeline().layers[ed.layer];
    let Tween::Shape { hints, .. } = &layer.frames[k].tween else { return };
    let elements = &layer.frames[k + usize::from(end)].elements;
    for (i, &(a, b)) in hints.iter().enumerate() {
        let q = if end { b } else { a };
        let fill = match (shape_tween::on_outline(elements, q, shape_tween::REACH), end) {
            (false, _) => HINT_LOOSE,
            (true, false) => HINT_FIRST,
            (true, true) => HINT_LAST,
        };
        let at = screen_point(ed, stage, q);
        p.circle(at, 7.0, fill, Stroke::new(1.0, Color32::BLACK));
        p.text(at, egui::Align2::CENTER_CENTER, hint_letter(i), egui::FontId::proportional(10.0), Color32::BLACK);
    }
}

/// Shape hint colors: off any outline, then on the first keyframe's and the last's.
const HINT_LOOSE: Color32 = Color32::from_rgb(0xE8, 0x3A, 0x2E);
const HINT_FIRST: Color32 = Color32::from_rgb(0xF5, 0xC8, 0x1A);
const HINT_LAST: Color32 = Color32::from_rgb(0x4C, 0xC2, 0x4C);

fn hint_letter(i: usize) -> String {
    char::from(b'a' + i.min(25) as u8).to_string()
}

/// The shape tween whose hints show on the active layer at the playhead: its keyframe,
/// and whether the playhead is on the keyframe it tweens to rather than its own.
fn hinted(ed: &Editor) -> Option<(usize, bool)> {
    let layer = ed.timeline().layers.get(ed.layer).filter(|l| l.visible())?;
    let k = layer.frames.iter().position(|key| key.index == ed.frame)?;
    let tweens = |k: usize| shape_tween::valid(&layer.frames[k], layer.frames.get(k + 1));
    if tweens(k) {
        return Some((k, false));
    }
    let before = k.checked_sub(1)?;
    tweens(before).then_some((before, true))
}

fn hints(ed: &Editor, k: usize) -> &[(Point, Point)] {
    match ed.timeline().layers.get(ed.layer).and_then(|l| l.frames.get(k)).map(|key| &key.tween) {
        Some(Tween::Shape { hints, .. }) => hints,
        _ => &[],
    }
}

/// Whether Modify > Shape > Add Shape Hint (on a shape tween's first keyframe, up to z)
/// or Remove All Hints (on its first or last) can run.
pub fn hint_available(ed: &Editor, action: Action) -> bool {
    match (hinted(ed), action) {
        (Some((k, false)), Action::AddShapeHint) => hints(ed, k).len() < 26,
        (Some((k, _)), Action::RemoveAllHints) => !hints(ed, k).is_empty(),
        _ => false,
    }
}

/// Add Shape Hint: the next letter, in the middle of both keyframes' shapes, to drag onto
/// their outlines; it turns View > Show Shape Hints on. Remove All Hints.
pub fn hint_command(ed: &mut Editor, action: Action) {
    let (Some((k, _)), true) = (hinted(ed), hint_available(ed, action)) else { return };
    let l = ed.layer;
    ed.stage.shape_hints |= action == Action::AddShapeHint;
    let middle = |k: usize| {
        let tl = ed.timeline();
        let key = &tl.layers[l].frames[k];
        let refs: Vec<ElementRef> = (0..key.elements.len()).map(|element| ElementRef { layer: l, keyframe: k, element }).collect();
        hit::bounds(&ed.doc, tl, key.index, &refs).map(|b| ((b.min.0 + b.max.0) / 2.0, (b.min.1 + b.max.1) / 2.0))
    };
    let hint = middle(k).zip(middle(k + 1));
    ed.checkpoint();
    if let Tween::Shape { hints, .. } = &mut ed.timeline_mut().layers[l].frames[k].tween {
        match (action, hint) {
            (Action::AddShapeHint, Some(hint)) => hints.push(hint),
            (Action::RemoveAllHints, _) => hints.clear(),
            _ => {}
        }
    }
}

/// The shape hint under screen point `at` (within its 7-point circle), the topmost:
/// (its keyframe, on the last keyframe, its letter's index, where it is).
fn hint_at(ed: &Editor, stage: Rect, at: Pos2) -> Option<(usize, bool, usize, Point)> {
    let (k, end) = hinted(ed).filter(|_| ed.stage.shape_hints && ed.timeline().layers.get(ed.layer).is_some_and(|l| !l.locked()))?;
    let placed = hints(ed, k).iter().map(|&(a, b)| if end { b } else { a }).enumerate();
    placed.rev().find(|&(_, q)| screen_point(ed, stage, q).distance(at) <= 7.0).map(|(i, q)| (k, end, i, q))
}

/// What the Selection tool drags on stage for a tween, as it was when the drag began: a
/// motion path point (a keyed one moves, another bends the path there) or a shape hint.
#[derive(Clone)]
enum Grab {
    Path { r: ElementRef, frame: u32, keyed: bool, from: Motion },
    Hint { l: usize, k: usize, end: bool, i: usize, from: Point },
}

/// The point of a selected target's path under screen point `at` (within 5 points), keyed
/// ones first, but the target's own (the frame showing) or where it covers it: (target,
/// frame into the span, keyed).
fn point_at(ed: &Editor, stage: Rect, at: Pos2) -> Option<(ElementRef, u32, bool)> {
    let targets = targets(ed);
    if targets.is_empty() {
        return None;
    }
    let over = crate::stage::hit_at(ed, local_point(ed, stage, at));
    let mut best: Option<(f32, (ElementRef, u32, bool))> = None;
    for (r, key) in targets.into_iter().filter(|(r, _)| over != Some(*r)) {
        let Tween::Motion(m) = &key.tween else { continue };
        let keyed = m.frames(Kind::Position);
        for (f, q) in motion::path_points(key).into_iter().enumerate() {
            let (f, is_key) = (f as u32, keyed.contains(&(f as u32)));
            let d = screen_point(ed, stage, q).distance(at) - if is_key { 2.0 } else { 0.0 };
            if key.index + f != ed.frame && d <= 5.0 && best.is_none_or(|(b, _)| d < b) {
                best = Some((d, (r, f, is_key)));
            }
        }
    }
    best.map(|(_, point)| point)
}

/// The Selection tool's pointer over a shape hint (a move), over a keyed motion path point
/// (a corner), or where a drag bends the path (an arc).
pub fn cursor(ed: &Editor, stage: Rect, at: Pos2) -> Option<Cursor> {
    if ed.tool != Tool::Selection || ed.drag.is_some() {
        return None;
    }
    if hint_at(ed, stage, at).is_some() {
        return Some(Cursor::Move);
    }
    point_at(ed, stage, at).map(|(_, _, keyed)| if keyed { Cursor::Corner } else { Cursor::Arc })
}

/// The Selection tool on a shape hint, or on a selected target's motion path: a drag moves
/// the hint (off the stage, it goes), a keyed point, or bends the path through another
/// frame's; one undo step. Returns whether the gesture is the tween's.
pub fn input(ui: &Ui, ed: &mut Editor, resp: &Response, stage: Rect) -> bool {
    let id = Id::new(DRAG);
    if let Some((grab, start)) = ui.data(|m| m.get_temp::<(Grab, Point)>(id)) {
        if let Some(pos) = resp.interact_pointer_pos().filter(|_| resp.dragged()) {
            let p = local_point(ed, stage, pos);
            drag(ed, &grab, Some((p.0 - start.0, p.1 - start.1)));
        }
        if !resp.dragged() {
            let off = ui.input(|i| i.pointer.interact_pos()).is_some_and(|p| !stage.contains(p));
            if off && matches!(grab, Grab::Hint { .. }) {
                drag(ed, &grab, None);
            }
            ui.data_mut(|m| m.remove::<(Grab, Point)>(id));
            ed.end_gesture();
        }
        return true;
    }
    if ed.tool != Tool::Selection || !resp.drag_started() {
        return false;
    }
    let Some(origin) = ui.input(|i| i.pointer.press_origin()) else { return false };
    let grab = match (hint_at(ed, stage, origin), point_at(ed, stage, origin)) {
        (Some((k, end, i, from)), _) => Grab::Hint { l: ed.layer, k, end, i, from },
        (None, Some((r, frame, keyed))) => {
            let Some(Tween::Motion(m)) = ed.timeline().layers.get(r.layer).and_then(|l| l.frames.get(r.keyframe)).map(|k| &k.tween) else { return false };
            Grab::Path { r, frame, keyed, from: (**m).clone() }
        }
        (None, None) => return false,
    };
    ui.data_mut(|m| m.insert_temp(id, (grab, local_point(ed, stage, origin))));
    true
}

/// Moves what `grab` holds by `d` from where it was, or with None removes the shape hint.
fn drag(ed: &mut Editor, grab: &Grab, d: Option<Point>) {
    let moved = |p: Point| d.map(|d| (p.0 + d.0, p.1 + d.1));
    let (l, k, tween) = match grab {
        Grab::Path { r, frame, keyed, from } => {
            let mut m = from.clone();
            match from.path.iter().find(|k| k.frame == *frame) {
                Some(k) if *keyed => m.set_point(*frame, moved(k.at).unwrap_or(k.at)),
                _ => m.bend(from, *frame, d.unwrap_or_default()),
            }
            (r.layer, r.keyframe, Tween::Motion(Box::new(m)))
        }
        Grab::Hint { l, k, end, i, from } => {
            let Some(layer) = ed.timeline().layers.get(*l) else { return };
            let Some(Tween::Shape { ease, angular, hints }) = layer.frames.get(*k).map(|k| k.tween.clone()) else { return };
            let mut hints = hints;
            // Dropped by an outline, where it pins, it goes onto it.
            let outline = layer.frames.get(k + usize::from(*end)).map_or(&[][..], |key| &key.elements[..]);
            let moved = |p: Point| moved(p).map(|q| shape_tween::snap(outline, q, shape_tween::REACH).unwrap_or(q));
            match (moved(*from), hints.get_mut(*i)) {
                (Some(q), Some(h)) => *(if *end { &mut h.1 } else { &mut h.0 }) = q,
                (None, Some(_)) => _ = hints.remove(*i),
                (_, None) => return,
            }
            (*l, *k, Tween::Shape { ease, angular, hints })
        }
    };
    let Some(key) = ed.timeline().layers.get(l).and_then(|layer| layer.frames.get(k)) else { return };
    if key.tween != tween {
        ed.checkpoint_gesture("tween-drag");
        ed.timeline_mut().layers[l].frames[k].tween = tween;
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::Modifiers;
    use egui_kittest::kittest::Queryable;

    use super::*;
    use crate::app::testkit::*;

    /// A new document holding a rectangle, the Selection tool picked.
    fn rectangle() -> Harness<'static, Editor> {
        let mut h = blank();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (150.0, 150.0));
        h.state_mut().tool = Tool::Selection;
        h
    }

    fn tween(h: &Harness<'_, Editor>, layer: usize) -> Tween {
        h.state().timeline().layers[layer].frames[0].tween.clone()
    }

    #[test]
    fn insert_menu_creates_each_tween() {
        use egui_kittest::kittest::NodeT;
        let mut h = rectangle();
        let ctx = h.ctx.clone();
        // A shape tween, then a classic one in its place, from the Insert menu.
        for (label, want) in [("Shape Tween", "shape"), ("Classic Tween", "classic")] {
            h.get_by_label("Insert").click();
            h.run();
            h.get_by_label(label).click();
            h.run();
            let got = match tween(&h, 0) {
                Tween::Shape { .. } => "shape",
                Tween::Classic { .. } => "classic",
                other => panic!("{other:?}"),
            };
            assert_eq!(got, want);
            h.state_mut().run(Action::RemoveTween, &ctx);
        }
        // A motion tween of a shape asks to turn it into a symbol first; OK does, and tweens it.
        h.get_by_label("Insert").click();
        h.run();
        assert!(!h.get_by_label("Motion Tween").accesskit_node().is_disabled());
        h.get_by_label("Motion Tween").click();
        h.run();
        assert_eq!(tween(&h, 0), Tween::None, "not yet");
        h.get_by_label("    OK    ").click();
        h.run();
        let ed = h.state();
        assert!(matches!(tween(&h, 0), Tween::Motion(_)), "{:?}", tween(&h, 0));
        assert!(matches!(ed.element(ed.selection[0]), Some(Element::Instance(i)) if i.symbol == "Symbol 1"));
        assert_eq!(ed.timeline().layers[0].frames[0].duration, 24, "a one-frame layer's span grows to a second");
        h.state_mut().run(Action::Undo, &ctx);
        assert!(h.state().doc.library.is_empty() && tween(&h, 0) == Tween::None, "converting and tweening are one step");
    }

    #[test]
    fn dragging_a_target_inside_its_span_keys_it() {
        let mut h = rectangle();
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::CreateMotionTween, &ctx);
        h.run();
        h.key_press(egui::Key::Enter);
        h.run();
        // At frame 21, drag it 200 px right: position keyframes at 1 and 21, the target staying put.
        let target = |h: &Harness<'_, Editor>| h.state().element(ElementRef { layer: 0, keyframe: 0, element: 0 }).cloned();
        let before = target(&h);
        assert!(matches!(before, Some(Element::Instance(_))));
        h.state_mut().set_frame(20);
        h.run();
        mouse_drag(&mut h, (125.0, 125.0), (325.0, 125.0));
        let Tween::Motion(m) = tween(&h, 0) else { panic!("motion") };
        assert_eq!(m.frames(Kind::Position), [0, 20]);
        assert_eq!(target(&h), before);
        let at = |h: &Harness<'_, Editor>, f| {
            let ed = h.state();
            hit::bounds(&ed.doc, ed.timeline(), f, &ed.selection).unwrap().min.0
        };
        assert!((at(&h, 10) - 199.5).abs() < 1.0 && (at(&h, 20) - 299.5).abs() < 1.0, "{} {}", at(&h, 10), at(&h, 20));
        // An arrow key nudges the keyframe there too; undo takes the drag back whole.
        h.key_press(egui::Key::ArrowDown);
        h.step();
        let Tween::Motion(m) = tween(&h, 0) else { panic!("motion") };
        assert_eq!(m.path.last().map(|k| k.at), Some((200.0, 1.0)));
        h.state_mut().run(Action::Undo, &ctx);
        h.state_mut().run(Action::Undo, &ctx);
        let Tween::Motion(m) = tween(&h, 0) else { panic!("motion") };
        assert!(m.path.is_empty());

        // Free Transform at frame 11: a quarter turn keys rotation.
        h.state_mut().set_frame(10);
        h.state_mut().selection = vec![ElementRef { layer: 0, keyframe: 0, element: 0 }];
        let ed = h.state_mut();
        crate::free_transform::about_pivot(ed, Matrix::rotate(std::f32::consts::FRAC_PI_2), None);
        let Tween::Motion(m) = tween(&h, 0) else { panic!("motion") };
        assert_eq!((m.frames(Kind::Rotation), m.frames(Kind::Position)), (vec![0, 10], vec![]));
    }

    #[test]
    fn align_and_remove_transform_key_the_target() {
        let mut h = rectangle();
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::CreateMotionTween, &ctx);
        h.run();
        h.key_press(egui::Key::Enter);
        h.run();
        // At frame 11, turned, then aligned to the stage's left edge: keyframes there, the target itself staying put.
        let r = ElementRef { layer: 0, keyframe: 0, element: 0 };
        let before = h.state().element(r).cloned();
        h.state_mut().set_frame(10);
        h.state_mut().selection = vec![r];
        crate::free_transform::about_pivot(h.state_mut(), Matrix::rotate(0.5), None);
        h.state_mut().align_to_stage = true;
        h.state_mut().run(Action::Align(minnamate_core::arrange::Align::Edges { y: false, at: 0.0 }), &ctx);
        let left = |h: &Harness<'_, Editor>, f| {
            let ed = h.state();
            hit::bounds(&ed.doc, ed.timeline(), f, &[r]).unwrap().min.0
        };
        let Tween::Motion(m) = tween(&h, 0) else { panic!("motion") };
        assert_eq!((m.frames(Kind::Position), m.frames(Kind::Rotation)), (vec![0, 10], vec![0, 10]));
        assert_eq!(h.state().element(r).cloned(), before);
        assert!(left(&h, 10).abs() < 0.01 && (left(&h, 0) - 99.5).abs() < 0.01, "{} {}", left(&h, 10), left(&h, 0));
        // Remove Transform there keys the turn back to none.
        h.state_mut().run(Action::RemoveTransform, &ctx);
        let Tween::Motion(m) = tween(&h, 0) else { panic!("motion") };
        assert!(m.props[motion::ROTATION].iter().all(|k| k.1.abs() < 1e-3), "{:?}", m.props[motion::ROTATION]);
        assert_eq!(h.state().element(r).cloned(), before);
    }

    #[test]
    fn typing_in_tweened_text_where_it_shows() {
        let mut h = blank();
        let ctx = h.ctx.clone();
        h.state_mut().tool = Tool::Text;
        mouse_click(&mut h, (100.0, 80.0));
        h.event(egui::Event::Text("abc".into()));
        h.step();
        h.key_press(egui::Key::Escape);
        h.step();
        // Motion tweened, and moved 200 px right at frame 11.
        h.state_mut().tool = Tool::Selection;
        h.state_mut().run(Action::CreateMotionTween, &ctx);
        h.run();
        let r = ElementRef { layer: 0, keyframe: 0, element: 0 };
        let before = h.state().element(r).cloned();
        h.state_mut().set_frame(10);
        h.state_mut().selection = vec![r];
        h.state_mut().nudge(200.0, 0.0, "nudge");
        assert!(matches!(tween(&h, 0), Tween::Motion(m) if m.frames(Kind::Position) == [0, 10]));
        // The Text tool types in it where it shows: a click at its start, a Shift+click at its
        // end selects it all, and typing replaces it. It keeps its own place.
        let Some(Element::Text(b)) = before else { panic!("text") };
        let width = minnamate_core::text::layout(&b).width;
        h.state_mut().tool = Tool::Text;
        mouse_click(&mut h, (300.5, 88.0));
        let end = on_stage(&h, (299.0 + width, 88.0));
        screen_click(&mut h, end, Modifiers::SHIFT);
        h.event(egui::Event::Text("d".into()));
        h.step();
        let Some(Element::Text(t)) = h.state().element(r).cloned() else { panic!("text") };
        assert_eq!((minnamate_core::text::string(&t).as_str(), t.matrix), ("d", b.matrix));
    }

    #[test]
    fn dragging_motion_path_points() {
        let mut h = rectangle();
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::CreateMotionTween, &ctx);
        h.run();
        h.key_press(egui::Key::Enter);
        h.run();
        h.state_mut().set_frame(20);
        h.run();
        mouse_drag(&mut h, (125.0, 125.0), (325.0, 125.0));
        // From frame 6, with the square selected: its path runs from (125, 125) to (325, 125).
        h.state_mut().set_frame(5);
        h.state_mut().selection = vec![ElementRef { layer: 0, keyframe: 0, element: 0 }];
        h.run();
        let points = |h: &Harness<'_, Editor>| motion::path_points(&h.state().timeline().layers[0].frames[0]);
        assert!((points(&h)[10].0 - 225.0).abs() < 0.01);
        let (stage, at) = (h.state().stage_rect, |h: &Harness<'_, Editor>, p| on_stage(h, p));
        assert_eq!(cursor(h.state(), stage, at(&h, (225.0, 125.0))), Some(Cursor::Arc), "frame 11's dot bends");
        assert_eq!(cursor(h.state(), stage, at(&h, (325.0, 125.0))), Some(Cursor::Corner), "frame 21's keyframe moves");
        assert_eq!(cursor(h.state(), stage, at(&h, (175.0, 125.0))), None, "the square itself, where it is now");

        // Frame 11's dot dragged up: the path bends through there.
        mouse_drag(&mut h, (225.0, 125.0), (225.0, 75.0));
        let p = points(&h)[10];
        assert!((p.0 - 225.0).abs() < 0.5 && (p.1 - 75.0).abs() < 0.5, "{p:?}");
        assert_eq!(h.state().selection.len(), 1, "the square stays selected");
        // The keyframe at frame 21 dragged down moves the path's end; one undo step each.
        mouse_drag(&mut h, (325.0, 125.0), (325.0, 175.0));
        assert!((points(&h)[20].1 - 175.0).abs() < 0.01);
        h.state_mut().run(Action::Undo, &ctx);
        assert!((points(&h)[20].1 - 125.0).abs() < 0.01 && (points(&h)[10].1 - 75.0).abs() < 0.5);
    }

    /// A shape tween from a red square at (100, 100) to a blue one at (300, 150), frames 1 to 11.
    fn squares(gpu: bool) -> Harness<'static, Editor> {
        use minnamate_core::doc::Paint;
        let builder = Harness::builder().with_size(egui::vec2(1600.0, 900.0)).with_step_dt(1.0 / 60.0);
        let mut h = if gpu { builder.wgpu() } else { builder }.build_eframe(|cc| Editor::new(&cc.egui_ctx, None));
        h.state_mut().new_document(550.0, 400.0, 24.0);
        let square = |x: f32, y: f32, c| Element::Shape(minnamate_core::draw::rectangle([(x, y), (x + 100.0, y + 100.0)], 0.0, None, Some(Paint::Solid(c))));
        let shape = Tween::Shape { ease: 0.0, angular: false, hints: Vec::new() };
        h.state_mut().timeline_mut().layers[0].frames = vec![
            Frame { index: 0, duration: 10, tween: shape, elements: vec![square(100.0, 100.0, [1.0, 0.0, 0.0, 1.0])], extra: Default::default() },
            Frame { index: 10, duration: 1, tween: Tween::None, elements: vec![square(300.0, 150.0, [0.0, 0.0, 1.0, 1.0])], extra: Default::default() },
        ];
        h.state_mut().tool = Tool::Selection;
        h.run();
        h
    }

    fn hints_of(h: &Harness<'_, Editor>) -> Vec<(Point, Point)> {
        match &h.state().timeline().layers[0].frames[0].tween {
            Tween::Shape { hints, .. } => hints.clone(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn shape_hints_pin_the_tween() {
        let mut h = squares(false);
        let ctx = h.ctx.clone();
        // Ctrl+Shift+H on the first keyframe: hint a, in the middle of both squares.
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, egui::Key::H);
        h.run();
        assert_eq!(hints_of(&h), [((150.0, 150.0), (350.0, 200.0))]);
        // Dragged onto the first square's top left corner, then on the last keyframe onto the second's bottom right.
        mouse_drag(&mut h, (150.0, 150.0), (100.0, 100.0));
        h.state_mut().set_frame(10);
        h.run();
        assert!(!hint_available(h.state(), Action::AddShapeHint) && hint_available(h.state(), Action::RemoveAllHints));
        mouse_drag(&mut h, (350.0, 200.0), (400.0, 250.0));
        assert_eq!(hints_of(&h), [((100.0, 100.0), (400.0, 250.0))]);
        assert_eq!(h.state().timeline().layers[0].frames[1].elements.len(), 1, "the square stays put");
        // Halfway, the pinned corners meet halfway.
        let layer = &h.state().timeline().layers[0];
        let mid = shape_tween::at(&layer.frames[0], layer.frames.get(1), 5).unwrap();
        assert!(mid.edges[0].paths[0].segs.iter().any(|s| (s.end().0 - 250.0).hypot(s.end().1 - 175.0) < 0.5));
        // Dragged off the stage, a hint goes; Remove All Hints takes them all; undo brings them back.
        h.state_mut().set_frame(0);
        h.run();
        h.state_mut().run(Action::AddShapeHint, &ctx);
        mouse_drag(&mut h, (100.0, 100.0), (-60.0, -60.0));
        assert_eq!(hints_of(&h), [((150.0, 150.0), (350.0, 200.0))], "b took a's place");
        h.state_mut().run(Action::RemoveAllHints, &ctx);
        assert!(hints_of(&h).is_empty());
        h.state_mut().run(Action::Undo, &ctx);
        assert_eq!(hints_of(&h).len(), 1);
    }

    #[test]
    fn show_shape_hints_hides_and_shows_them() {
        let mut h = squares(false);
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::AddShapeHint, &ctx);
        h.run();
        let toggle = |h: &mut Harness<'_, Editor>| {
            h.get_by_label("View").click();
            h.run();
            h.get_by_label("Show Shape Hints Ctrl+Alt+I").click();
            h.run();
        };
        // View > Show Shape Hints off: there's no hint to drag (the drag moves the square instead).
        toggle(&mut h);
        mouse_drag(&mut h, (150.0, 150.0), (100.0, 100.0));
        assert_eq!(hints_of(&h)[0].0, (150.0, 150.0));
        // Back on, it drags.
        toggle(&mut h);
        mouse_drag(&mut h, (150.0, 150.0), (130.0, 120.0));
        assert_eq!(hints_of(&h)[0].0, (130.0, 120.0));
    }

    #[test]
    fn hints_dropped_by_an_outline_go_onto_it() {
        let mut h = squares(false);
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::AddShapeHint, &ctx);
        h.run();
        // Dropped 4 px inside the first square's top side, it sits on that side; further in, where dropped.
        mouse_drag(&mut h, (150.0, 150.0), (130.0, 104.0));
        assert_eq!(hints_of(&h)[0].0, (130.0, 100.0));
        mouse_drag(&mut h, (130.0, 100.0), (130.0, 120.0));
        assert_eq!(hints_of(&h)[0].0, (130.0, 120.0));
    }

    /// Writes target/screenshots/shape_hints.png: a shape tween's first keyframe with hint a
    /// on its outline (yellow) and hint b off it (red).
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn shape_hints_screenshot() {
        let mut h = squares(true);
        if let Tween::Shape { hints, .. } = &mut h.state_mut().timeline_mut().layers[0].frames[0].tween {
            hints.extend([((100.0, 100.0), (400.0, 250.0)), ((150.0, 150.0), (350.0, 200.0))]);
        }
        h.state_mut().zoom = 2.0;
        h.run();
        h.remove_cursor();
        h.run();
        screenshot(&mut h, "shape_hints");
    }

    #[test]
    fn split_motion_from_the_frame_menu() {
        let mut h = rectangle();
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::CreateMotionTween, &ctx);
        h.run();
        h.key_press(egui::Key::Enter);
        h.run();
        h.state_mut().set_frame(20);
        h.run();
        mouse_drag(&mut h, (125.0, 125.0), (325.0, 125.0));
        let left = |h: &Harness<'_, Editor>, f: u32| {
            let ed = h.state();
            let key = ed.timeline().layers[0].keyframe_at(f).unwrap();
            hit::bounds(&ed.doc, ed.timeline(), f, &[ElementRef { layer: 0, keyframe: key, element: 0 }]).unwrap().min.0
        };
        let before: Vec<f32> = (0..24).map(|f| left(&h, f)).collect();
        // Right-click frame 9: the motion tween's menu, as Animate's, splits the span there.
        let at = cell(&h, 0, 8);
        h.hover_at(at);
        h.step();
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: Modifiers::NONE });
            h.step();
        }
        h.run();
        assert!(h.query_by_label("Paste Motion Special...").is_none() && h.query_by_label("Join Motions").is_some());
        h.get_by_label("Split Motion").click();
        h.run();
        let layer = &h.state().timeline().layers[0];
        assert_eq!(layer.frames.iter().map(|k| (k.index, matches!(k.tween, Tween::Motion(_)))).collect::<Vec<_>>(), [(0, true), (8, true)]);
        let after: Vec<f32> = (0..24).map(|f| left(&h, f)).collect();
        assert!(before.iter().zip(&after).all(|(a, b)| (a - b).abs() < 0.01), "{before:?}\n{after:?}");
        h.state_mut().run(Action::Undo, &ctx);
        assert_eq!(h.state().timeline().layers[0].frames.len(), 1);
    }

    #[test]
    fn property_keyframes_from_the_frame_menu() {
        let mut h = rectangle();
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::CreateMotionTween, &ctx);
        h.run();
        h.key_press(egui::Key::Enter);
        h.run();
        // Right-click frame 6: inside the tween the menu keys properties.
        let at = cell(&h, 0, 5);
        h.hover_at(at);
        h.step();
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: Modifiers::NONE });
            h.step();
        }
        h.run();
        assert!(h.query_by_label("Convert to Keyframes F6").is_none(), "not inside a motion tween");
        h.get_by_label("Insert Keyframe ⏵").hover();
        h.run_steps(10);
        h.get_by_label("Scale").click();
        h.run();
        let Tween::Motion(m) = tween(&h, 0) else { panic!("motion") };
        assert_eq!((m.frames(Kind::Scale), m.frames(Kind::Position)), (vec![0, 5], vec![]));
        // F6 there keys the rest; Clear Keyframe takes them away again.
        h.key_press(egui::Key::F6);
        h.run();
        let Tween::Motion(m) = tween(&h, 0) else { panic!("motion") };
        assert_eq!(Kind::ALL.map(|k| m.frames(k).contains(&5)), [true; 5]);
        assert_eq!(h.state().timeline().layers[0].frames.len(), 1, "F6 keeps the span whole");
        h.state_mut().run(Action::ClearPropertyKeyframe(None), &ctx);
        let Tween::Motion(m) = tween(&h, 0) else { panic!("motion") };
        assert!(Kind::ALL.iter().all(|&k| !m.frames(k).contains(&5)));
    }

    #[test]
    fn motion_commands_on_odd_timelines() {
        let all = [
            Action::CreateMotionTween,
            Action::CreateShapeTween,
            Action::InsertPropertyKeyframe(None),
            Action::ClearPropertyKeyframe(Some(Kind::Color)),
            Action::RemoveTween,
        ];
        let setups: [fn(&mut Editor); 3] = [|ed| ed.frame = 40, |ed| ed.doc.scenes[0].layers.clear(), |ed| {
            ed.doc.scenes[0].layers.clear();
            ed.doc.scenes[0].insert_folder(0, "Folder_1");
        }];
        for setup in setups {
            let mut h = harness(false);
            h.run();
            let ctx = h.ctx.clone();
            setup(h.state_mut());
            h.run();
            for action in all {
                h.state_mut().run(action, &ctx);
                h.run();
            }
        }
    }

    /// The tween fixture with the ball's layer a motion tween bent into a curve, a shape
    /// tween on a new layer (one keyframe a shape, the next a circle), and a broken one.
    fn showcase() -> Harness<'static, Editor> {
        use minnamate_core::doc::{Paint, Stroke};
        let mut h = harness(true);
        h.run();
        let ed = h.state_mut();
        let fps = ed.doc.frame_rate;
        let tl = ed.timeline_mut();
        tl.layers[1].frames.truncate(1);
        tl.layers[1].frames[0].duration = 30;
        assert!(tl.layers[1].create_motion_tween(0, fps));
        // Across the stage on a curve, growing by frame 13 and turning by frame 21, about its transformation point.
        let ball = ElementRef { layer: 1, keyframe: 0, element: 0 };
        motion::edit_shown(tl, ball, 29, |el| hit::translate(el, 330.0, -60.0));
        if let Tween::Motion(m) = &mut tl.layers[1].frames[0].tween {
            let from = (**m).clone();
            m.bend(&from, 15, (0.0, -120.0));
        }
        let pivot = |tl: &minnamate_core::doc::Timeline, f: u32| match motion::target_at(&tl.layers[1].frames[0], f) {
            Some(Element::Instance(i)) => i.matrix.apply(i.pivot),
            _ => panic!("the ball"),
        };
        let grow = Matrix::scale(1.5, 1.5).about(pivot(tl, 12));
        motion::edit_shown(tl, ball, 12, |el| minnamate_core::transform::transform(el, &grow));
        let turn = Matrix::rotate(0.8).about(pivot(tl, 20));
        motion::edit_shown(tl, ball, 20, |el| minnamate_core::transform::transform(el, &turn));
        let black = Stroke { width: 2.0, paint: Paint::Solid([0.0, 0.0, 0.0, 1.0]), cap: minnamate_core::doc::Cap::Round, join: minnamate_core::doc::Join::Round };
        let square = minnamate_core::draw::rectangle([(60.0, 300.0), (140.0, 380.0)], 0.0, Some(black.clone()), Some(Paint::Solid([1.0, 0.6, 0.0, 1.0])));
        let circle = minnamate_core::draw::oval([(420.0, 290.0), (520.0, 390.0)], Some(black), Some(Paint::Solid([0.2, 0.4, 1.0, 1.0])));
        let at = tl.insert_layer(0, "Morph");
        let morph = &mut tl.layers[at];
        morph.frames[0].elements = vec![Element::Shape(square)];
        morph.frames[0].duration = 12;
        morph.create_shape_tween(0);
        morph.insert_blank_keyframe(12);
        morph.frames[1].elements = vec![Element::Shape(circle)];
        morph.frames[1].duration = 8;
        morph.insert_frames(19, 3);
        morph.create_shape_tween(20);
        morph.frames.last_mut().unwrap().duration = 6;
        ed.layer = 2;
        h
    }

    /// Writes target/screenshots/tween_timeline.png: a motion tween (gold, diamonds at its
    /// property keyframes), shape tweens (orange, an arrow; the last broken, dashed) and the
    /// classic-tween fixture's other layers; and tween_timeline_menu.png, the frame menu open
    /// inside the motion tween.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn tween_timeline_screenshot() {
        let mut h = showcase();
        h.state_mut().set_frame(15);
        h.run();
        h.remove_cursor();
        h.run();
        screenshot(&mut h, "tween_timeline");
        let at = cell(&h, 2, 15);
        h.hover_at(at);
        h.step();
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: Modifiers::NONE });
            h.step();
        }
        h.run();
        h.get_by_label("Insert Keyframe ⏵").hover();
        h.run_steps(10);
        screenshot(&mut h, "tween_timeline_menu");
    }

    /// Writes target/screenshots/motion_path.png: the ball selected halfway along its
    /// curved motion path, Properties showing the motion tween's settings.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn motion_path_screenshot() {
        let mut h = showcase();
        let ed = h.state_mut();
        ed.set_frame(15);
        ed.selection = vec![ElementRef { layer: 2, keyframe: 0, element: 0 }];
        ed.props_tab = crate::app::PropsTab::Frame;
        h.run();
        h.remove_cursor();
        h.run();
        screenshot(&mut h, "motion_path");
    }
}
