//! Text tool (T): click to type static text, drag to give it a fixed width,
//! click text to type in it. A hidden TextEdit takes the keyboard, so tool keys
//! type instead of switching tools; the text shows as the document renders it,
//! with the caret, selection and box drawn here from its layout.

use std::ops::Range;

use eframe::egui::text::{CCursor, CCursorRange};
use eframe::egui::{self, Area, Color32, EventFilter, Id, Key, Modifiers, Painter, Rect, Response, Stroke, StrokeKind, Ui, vec2};
use minnamate_core::doc::{Element, Extra, Matrix, Point, Rgba, Text, TextAlign, TextKind, TextRun};
use minnamate_core::fonts;
use minnamate_core::hit::ElementRef;
use minnamate_core::text::{self, Layout};

use crate::app::{Editor, rgba};
use crate::icons::Icon;
use crate::stage::{hit_at, local_point, screen_point};
use crate::theme;
use crate::tools::Tool;

/// The hidden TextEdit that takes the keyboard while typing.
const FIELD: &str = "text-tool-field";
/// The box around the text being typed.
const BOX: Color32 = Color32::from_gray(0x40);

/// The style new text gets (Properties > Tool), and the text being typed.
pub struct State {
    /// As XFL names faces: a PostScript name.
    face: String,
    /// In pixels, which Animate's panels call points.
    size: f32,
    letter_spacing: f32,
    align: TextAlign,
    editing: Option<Session>,
}

impl Default for State {
    fn default() -> Self {
        // Times New Roman, 12 pt: Animate's starting font.
        State { face: "TimesNewRomanPSMT".into(), size: 12.0, letter_spacing: 0.0, align: TextAlign::Left, editing: None }
    }
}

/// A text field being typed in.
struct Session {
    /// Where the document holds it: None for new text until its first character.
    target: Option<ElementRef>,
    /// The text as typed, which the document holds at `target`.
    text: Text,
    /// Its characters, as the TextEdit edits them.
    buffer: String,
    /// The selection as (anchor, caret) character indices.
    cursor: (usize, usize),
    /// The cursor moved here, for the TextEdit to take up.
    moved: bool,
    /// Not shown to the TextEdit yet, whose undo history is still another text's.
    fresh: bool,
    /// The fill color the text last took: picking another recolors it.
    color: Color32,
    /// Where typing started: leaving the frame or symbol ends it, and new text goes on the layer.
    frame: u32,
    depth: usize,
    layer: usize,
    drag: Option<Drag>,
}

/// A press on the stage, while the button is down.
enum Drag {
    /// Selecting characters from this one.
    Select(usize),
    /// Sizing a new text from this point: a drag sets a fixed width.
    Size(Point),
}

impl Session {
    fn new(target: Option<ElementRef>, text: Text, ed: &Editor) -> Self {
        Session {
            target,
            buffer: text::string(&text),
            text,
            cursor: (0, 0),
            moved: true,
            fresh: true,
            color: ed.fill_color,
            frame: ed.frame,
            depth: ed.editing.len(),
            layer: ed.layer,
            drag: None,
        }
    }

    /// The caret position nearest `p` (in the timeline's coordinates), and whether `p` is in the box.
    fn index_at(&self, p: Point) -> (usize, bool) {
        let layout = text::layout(&self.text);
        let q = self.text.matrix.inverse().map_or(p, |m| m.apply(p));
        let inside = (0.0..=layout.width).contains(&q.0) && (0.0..=layout.height).contains(&q.1);
        (layout.index_at(q), inside)
    }

    fn select(&mut self, anchor: usize, caret: usize) {
        self.cursor = (anchor, caret);
        self.moved = true;
    }
}

pub fn kind_name(kind: TextKind) -> &'static str {
    match kind {
        TextKind::Static => "Static Text",
        TextKind::Dynamic => "Dynamic Text",
        TextKind::Input => "Input Text",
    }
}

/// Pointer and keyboard input on the stage while this tool is active.
pub fn input(ui: &Ui, ed: &mut Editor, resp: &Response, stage: Rect) {
    check(ed);
    let (pressed, down, pos, shift) =
        ui.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.interact_pos(), i.modifiers.shift));
    if let Some(pos) = pos {
        let p = local_point(ed, stage, pos);
        if pressed && resp.hovered() {
            press(ed, p, shift);
        } else if down {
            drag(ed, p);
        }
    }
    if let Some(s) = &mut ed.text.editing {
        if !down {
            s.drag = None;
        }
        if resp.double_clicked() {
            select_word(s);
        }
    }
    // A color picked for the fill, here or in the toolbar, is the text's.
    if let Some(s) = &ed.text.editing
        && s.color != ed.fill_color
    {
        let (color, picked) = (rgba(ed.fill_color), ed.fill_color);
        restyle(ed, |run| run.color = color);
        if let Some(s) = &mut ed.text.editing {
            s.color = picked;
        }
    }
    keyboard(ui, ed, stage);
}

/// Pressing in the text being typed moves the caret (Shift: the selection's end);
/// elsewhere it ends that, then types in the text pressed on or starts new text.
fn press(ed: &mut Editor, p: Point, shift: bool) {
    if let Some(s) = &mut ed.text.editing {
        let (i, inside) = s.index_at(p);
        if inside {
            let anchor = if shift { s.cursor.0 } else { i };
            s.select(anchor, i);
            s.drag = Some(Drag::Select(anchor));
            return;
        }
    }
    commit(ed);
    match hit_at(ed, p) {
        Some(r) if matches!(element(ed, r), Some(Element::Text(_))) => {
            edit(ed, r, p);
            if let Some(s) = &mut ed.text.editing {
                s.drag = Some(Drag::Select(s.cursor.0));
            }
        }
        _ => start(ed, p),
    }
}

fn drag(ed: &mut Editor, p: Point) {
    let zoom = ed.zoom;
    let Some(s) = &mut ed.text.editing else { return };
    match s.drag {
        Some(Drag::Select(anchor)) => {
            let (i, _) = s.index_at(p);
            if s.cursor != (anchor, i) {
                s.select(anchor, i);
            }
        }
        Some(Drag::Size(start)) => {
            // Dragged wider than a few screen pixels, the box takes that width, as in Animate.
            let w = (p.0 - start.0).abs();
            s.text.width = (w * zoom > 4.0).then_some(w);
            s.text.matrix.tx = if s.text.width.is_some() { start.0.min(p.0) } else { start.0 };
        }
        None => {}
    }
}

/// Double-click: the word around the caret.
fn select_word(s: &mut Session) {
    let chars: Vec<char> = s.buffer.chars().collect();
    let word = |c: &&char| c.is_alphanumeric() || **c == '_';
    let i = s.cursor.1.min(chars.len());
    let start = i - chars[..i].iter().rev().take_while(word).count();
    let end = i + chars[i..].iter().take_while(word).count();
    s.select(start, end);
    s.drag = None;
}

/// Double-click on text with the Selection tool: the Text tool, typing there.
pub fn edit_at(ed: &mut Editor, r: ElementRef, at: Point) {
    commit(ed);
    ed.tool = Tool::Text;
    edit(ed, r, at);
}

/// Starts typing in text `r` of the edited timeline, the caret nearest `at`.
fn edit(ed: &mut Editor, r: ElementRef, at: Point) {
    let Some(Element::Text(t)) = element(ed, r).cloned() else { return };
    // A motion tween's text is typed in where it shows (`write` keeps its own place).
    let t = Text { matrix: crate::tweens::shown_matrix(ed, r).unwrap_or(t.matrix), ..t };
    // Properties shows the text's style, and the fill its color, as in Animate.
    if let Some(run) = t.runs.first() {
        ed.text.face = run.face.clone();
        ed.text.size = run.size;
        ed.text.letter_spacing = run.letter_spacing;
        ed.fill_color = color32(run.color);
    }
    ed.text.align = t.align;
    (ed.layer, ed.selection) = (r.layer, Vec::new());
    let mut s = Session::new(Some(r), t, ed);
    let (i, _) = s.index_at(at);
    s.select(i, i);
    ed.text.editing = Some(s);
}

/// Starts new text with its box's top left corner at `p`, as wide as its text.
fn start(ed: &mut Editor, p: Point) {
    if !ed.drawable() {
        ed.log("Text needs a normal layer, shown and unlocked: select one in the timeline.");
        return;
    }
    let text = Text {
        kind: TextKind::Static,
        matrix: Matrix { tx: p.0, ty: p.1, ..Matrix::IDENTITY },
        width: None,
        align: ed.text.align,
        // Animate's default line spacing.
        line_spacing: 2.0,
        runs: vec![new_run(ed)],
        // Static text can't be selected in the movie unless asked, as Animate makes it.
        extra: Extra { attrs: vec![("isSelectable".into(), "false".into())], ..Extra::default() },
    };
    ed.selection.clear();
    let mut s = Session::new(None, text, ed);
    s.drag = Some(Drag::Size(p));
    ed.text.editing = Some(s);
}

/// An empty run in the style Properties shows.
fn new_run(ed: &Editor) -> TextRun {
    TextRun {
        text: String::new(),
        face: ed.text.face.clone(),
        size: ed.text.size,
        color: rgba(ed.fill_color),
        bold: false,
        italic: false,
        letter_spacing: ed.text.letter_spacing,
        extra: Extra::default(),
    }
}

/// Ends typing when the document, frame or timeline changed under it. After an
/// undo the text isn't what was typed: that stops it, leaving the document be.
fn check(ed: &mut Editor) {
    let Some(s) = &ed.text.editing else { return };
    let intact = s.target.is_none_or(|r| matches!(element(ed, r), Some(Element::Text(t)) if *t == held(ed, r, s)));
    if !intact {
        ed.text.editing = None;
    } else if s.frame != ed.frame || s.depth != ed.editing.len() {
        commit(ed);
    }
}

/// Ends typing; static text left empty goes, as in Animate. Dynamic and input
/// text may be empty, for the movie or the viewer to fill in.
pub fn commit(ed: &mut Editor) {
    let Some(s) = ed.text.editing.take() else { return };
    if let Some(r) = s.target
        && s.text.kind == TextKind::Static
        && text::string(&s.text).is_empty()
        && matches!(element(ed, r), Some(Element::Text(t)) if *t == held(ed, r, &s))
    {
        ed.checkpoint_gesture("text");
        if let Some(key) = ed.timeline_mut().layers.get_mut(r.layer).and_then(|l| l.frames.get_mut(r.keyframe)) {
            key.elements.remove(r.element);
        }
    }
    ed.end_gesture();
}

/// The hidden TextEdit: the keys it gets edit the text, which then goes into the document.
fn keyboard(ui: &Ui, ed: &mut Editor, stage: Rect) {
    let ctx = ui.ctx().clone();
    let id = Id::new(FIELD);
    let focused = ctx.memory(|m| m.has_focus(id));
    if focused && ctx.input(|i| i.key_pressed(Key::Escape)) {
        commit(ed);
        return;
    }
    let style = new_run(ed);
    let Some(s) = &ed.text.editing else { return };
    let layout = text::layout(&s.text);
    // The field sits at the caret, where an input method shows its window.
    let l = layout.line_of(s.cursor.1);
    let top = layout.lines.get(l).map_or(0.0, |line| line.baseline - line.ascent);
    let at = screen_point(ed, stage, s.text.matrix.apply((layout.x_of(l, s.cursor.1), top)));

    let Some(s) = &mut ed.text.editing else { return };
    if focused {
        line_keys(&ctx, s, &layout);
    }
    if std::mem::take(&mut s.moved) {
        let mut state =
            if std::mem::take(&mut s.fresh) { Default::default() } else { egui::TextEdit::load_state(&ctx, id).unwrap_or_default() };
        state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(s.cursor.0), CCursor::new(s.cursor.1))));
        egui::TextEdit::store_state(&ctx, id, state);
    }
    let field = Area::new(id.with("area")).fixed_pos(at).interactable(false).show(&ctx, |ui| {
        // Invisible: the document draws the text, and `overlay` the caret and selection.
        let v = ui.visuals_mut();
        (v.text_cursor.stroke, v.text_cursor.blink) = (Stroke::NONE, false);
        (v.selection.bg_fill, v.selection.stroke) = (Color32::TRANSPARENT, Stroke::NONE);
        egui::TextEdit::multiline(&mut s.buffer)
            .id(id)
            .frame(egui::Frame::NONE)
            .text_color(Color32::TRANSPARENT)
            .desired_width(f32::INFINITY)
            .desired_rows(1)
            // Escape ends typing here, and Tab doesn't move on to another field.
            .event_filter(EventFilter { horizontal_arrows: true, vertical_arrows: true, tab: true, escape: true })
            .show(ui)
    });
    let out = field.inner;
    // Clicks surrender egui's focus; typing goes on unless another field took the keyboard.
    if ctx.memory(|m| m.focused().is_none()) {
        out.response.request_focus();
    }
    if let Some(r) = out.cursor_range {
        s.cursor = (r.secondary.index.0, r.primary.index.0);
    }
    if out.response.changed() {
        // Pasted text may bring Windows line ends.
        if s.buffer.contains('\r') {
            s.buffer = s.buffer.replace("\r\n", "\n").replace('\r', "\n");
        }
        let (range, with) = diff(&text::string(&s.text), &s.buffer);
        text::replace(&mut s.text, range, &with, &style);
        write(ed);
    }
}

/// Up, Down, Home and End go by the lines as laid out, which the TextEdit doesn't know.
fn line_keys(ctx: &egui::Context, s: &mut Session, layout: &Layout) {
    // Static text has no tab stops to lay a tab character out at.
    ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab));
    // Shift first: egui matches a plain key with Shift held too.
    for (shift, mods) in [(true, Modifiers::SHIFT), (false, Modifiers::NONE)] {
        for key in [Key::ArrowUp, Key::ArrowDown, Key::Home, Key::End] {
            if !ctx.input_mut(|i| i.consume_key(mods, key)) {
                continue;
            }
            let caret = s.cursor.1;
            let l = layout.line_of(caret);
            let to = match key {
                Key::Home => layout.lines.get(l).map_or(0, |line| line.start),
                Key::End => layout.line_end(l),
                Key::ArrowUp if l == 0 => 0,
                Key::ArrowDown if l + 1 >= layout.lines.len() => s.buffer.chars().count(),
                _ => {
                    let target = if key == Key::ArrowUp { l - 1 } else { l + 1 };
                    layout.index_at((layout.x_of(l, caret), layout.lines[target].baseline))
                }
            };
            s.select(if shift { s.cursor.0 } else { to }, to);
        }
    }
}

/// The characters of `old` that `new` replaced, and what replaced them.
fn diff(old: &str, new: &str) -> (Range<usize>, String) {
    let (a, b): (Vec<char>, Vec<char>) = (old.chars().collect(), new.chars().collect());
    let start = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let end = a[start..].iter().rev().zip(b[start..].iter().rev()).take_while(|(x, y)| x == y).count();
    (start..a.len() - end, b[start..b.len() - end].iter().collect())
}

/// Puts the text as typed into the document: where it is, or, once it has
/// characters, onto the layer it was started on. A whole edit is one undo step.
fn write(ed: &mut Editor) {
    let Some(s) = &ed.text.editing else { return };
    let (target, text, layer) = (s.target, s.text.clone(), s.layer);
    match target {
        Some(r) => {
            let text = held(ed, r, s);
            ed.checkpoint_gesture("text");
            if let Some(el) = ed.element_mut(r) {
                *el = Element::Text(text);
            }
        }
        None if !text::string(&text).is_empty() => {
            let r = add(ed, layer, text);
            if let Some(s) = &mut ed.text.editing {
                s.target = r;
            }
        }
        None => {}
    }
}

/// Adds `text` to layer `l` at the playhead: to the keyframe showing there, or
/// a new blank keyframe past the layer's end.
fn add(ed: &mut Editor, l: usize, text: Text) -> Option<ElementRef> {
    let f = ed.frame;
    ed.timeline().layers.get(l)?;
    ed.checkpoint_gesture("text");
    let layer = &mut ed.timeline_mut().layers[l];
    if layer.keyframe_at(f).is_none() {
        layer.insert_blank_keyframe(f);
    }
    let k = layer.keyframe_at(f)?;
    let elements = &mut layer.frames[k].elements;
    elements.push(Element::Text(text));
    Some(ElementRef { layer: l, keyframe: k, element: elements.len() - 1 })
}

fn element(ed: &Editor, r: ElementRef) -> Option<&Element> {
    ed.timeline().layers.get(r.layer)?.frames.get(r.keyframe)?.elements.get(r.element)
}

/// The text being typed as text `r` holds it: a motion tween's, typed where it shows, keeps its own place.
fn held(ed: &Editor, r: ElementRef, s: &Session) -> Text {
    match element(ed, r) {
        Some(Element::Text(own)) if minnamate_core::motion::tweening(ed.timeline(), r, s.frame) => Text { matrix: own.matrix, ..s.text.clone() },
        _ => s.text.clone(),
    }
}

/// Applies a Properties change to the text being typed, every run of it.
// ponytail: to the whole text, where Animate styles the selected characters. Split the runs
// at the selection to style part of a text.
fn restyle(ed: &mut Editor, change: impl Fn(&mut TextRun)) {
    let Some(s) = &mut ed.text.editing else { return };
    s.text.runs.iter_mut().for_each(change);
    write(ed);
}

fn color32(c: Rgba) -> Color32 {
    let [r, g, b, a] = c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// The box around the text being typed (a round handle while it grows with its
/// text, square once its width is fixed), the selection and the caret.
pub fn overlay(p: &Painter, ed: &Editor, stage: Rect) {
    let Some(s) = &ed.text.editing else { return };
    let layout = text::layout(&s.text);
    let to_screen = |q: Point| screen_point(ed, stage, s.text.matrix.apply(q));
    // A box in the text's coordinates, as it shows on screen.
    let quad = |x0: f32, y0: f32, x1: f32, y1: f32| vec![to_screen((x0, y0)), to_screen((x1, y0)), to_screen((x1, y1)), to_screen((x0, y1))];

    let (a, b) = (s.cursor.0.min(s.cursor.1), s.cursor.0.max(s.cursor.1));
    for (l, line) in layout.lines.iter().enumerate() {
        let (top, bottom) = (line.baseline - line.ascent, line.baseline + line.descent);
        let end = line.start + line.glyphs.len();
        if a == b {
            if l == layout.line_of(b) {
                let x = layout.x_of(l, b);
                p.line_segment([to_screen((x, top)), to_screen((x, bottom))], Stroke::new(1.0, Color32::BLACK));
            }
        } else if a <= end && b > line.start {
            let x0 = layout.x_of(l, a.max(line.start));
            let x1 = layout.x_of(l, b.min(end));
            // Selected on past the line's end, its newline shows as a sliver.
            let x1 = if b > end { x1.max(x0) + line.ascent / 3.0 } else { x1 };
            p.add(egui::Shape::convex_polygon(quad(x0, top, x1, bottom), theme::SELECTION_BOX.gamma_multiply(0.35), Stroke::NONE));
        }
    }

    let (w, h) = (layout.width, layout.height);
    p.add(egui::Shape::closed_line(quad(0.0, 0.0, w, h), Stroke::new(1.0, BOX)));
    // Static text's handle is at the top right, dynamic and input text's at the bottom right.
    let corner = to_screen(if s.text.kind == TextKind::Static { (w, 0.0) } else { (w, h) });
    if s.text.width.is_none() {
        p.circle(corner, 3.5, Color32::WHITE, Stroke::new(1.0, BOX));
    } else {
        p.rect(Rect::from_center_size(corner, vec2(7.0, 7.0)), 0.0, Color32::WHITE, Stroke::new(1.0, BOX), StrokeKind::Middle);
    }
}

/// Properties > Tool: Animate's Character and Paragraph settings. While typing
/// they restyle the text; otherwise they're for the next one.
pub fn options(ui: &mut Ui, ed: &mut Editor) {
    use crate::widgets::{self, Number, WORD_W};
    // ponytail: shows the kind, but new text is always static. Dynamic and input text need
    // their own options (instance name, line type, embedding) to be worth making.
    let kind = ed.text.editing.as_ref().map_or(TextKind::Static, |s| s.text.kind);
    widgets::row(ui, |ui| {
        ui.add_space(16.0);
        let kinds = [TextKind::Static, TextKind::Dynamic, TextKind::Input].map(|k| (k, kind_name(k), k == kind));
        widgets::pick(ui, "Text type", kind, &kinds, ui.available_width() - 16.0);
    });
    theme::section(ui, "Character", |ui| {
        // A face that isn't installed shows by name; it draws in a fallback font.
        let face = fonts::find(&ed.text.face);
        let family = face.map_or(ed.text.face.clone(), |f| f.family.clone());
        let mut picked = None;
        widgets::row(ui, |ui| {
            widgets::label(ui, "Family", WORD_W);
            widgets::dropdown(ui, "Font family", &family, ui.available_width() - 4.0, |ui| {
                ui.set_max_height(320.0);
                for name in fonts::families() {
                    if widgets::choice(ui, name == family, true, name).clicked() {
                        // The family's face nearest the one in use.
                        let (bold, italic) = face.map_or((false, false), |f| (f.weight >= 600, f.italic));
                        picked = fonts::resolve(name, bold, italic);
                    }
                }
            });
        });
        widgets::row(ui, |ui| {
            widgets::label(ui, "Style", WORD_W);
            let style = face.map_or("Regular", |f| f.style.as_str());
            widgets::dropdown(ui, "Font style", style, ui.available_width() - 4.0, |ui| {
                for f in fonts::system().iter().filter(|f| f.family == family) {
                    if widgets::choice(ui, f.style == style, true, &f.style).clicked() {
                        picked = Some(f);
                    }
                }
            });
        });
        if let Some(f) = picked {
            ed.text.face = f.postscript.clone();
            // The face carries the weight and slant now.
            restyle(ed, |run| (run.face, run.bold, run.italic) = (f.postscript.clone(), false, false));
        }
        widgets::row(ui, |ui| {
            widgets::label(ui, "Size", WORD_W);
            let mut size = ed.text.size;
            if ui.add(Number::new("Size", &mut size, 1.0..=2500.0).speed(0.5).suffix(" pt")).changed() {
                ed.text.size = size;
                restyle(ed, |run| run.size = size);
            }
        });
        widgets::row(ui, |ui| {
            widgets::label(ui, "Letter spacing", 84.0);
            let mut spacing = ed.text.letter_spacing;
            if ui.add(Number::new("Letter spacing", &mut spacing, -60.0..=60.0).speed(0.1)).changed() {
                ed.text.letter_spacing = spacing;
                restyle(ed, |run| run.letter_spacing = spacing);
            }
        });
        // Text has no "no colour", so its popup leaves that button out.
        widgets::row(ui, |ui| {
            let chip = widgets::chip(ui, ed.fill_color, "Text color");
            if let Some(c) = crate::color::swatch_popup(ui, ed, &chip, ed.fill_color, false) {
                ed.fill_color = c;
            }
            widgets::label(ui, "Fill", 50.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let [r, g, b, a] = ed.fill_color.to_srgba_unmultiplied();
                let mut alpha = (a as f32 / 2.55).round();
                if ui.add(Number::new("Text alpha", &mut alpha, 0.0..=100.0).suffix(" %").width(44.0)).changed() {
                    ed.fill_color = Color32::from_rgba_unmultiplied(r, g, b, (alpha * 2.55).round() as u8);
                }
                widgets::checker(ui);
            });
        });
    });
    theme::section(ui, "Paragraph", |ui| {
        widgets::row(ui, |ui| {
            widgets::label(ui, "Format", WORD_W);
            let aligns = [TextAlign::Left, TextAlign::Center, TextAlign::Right, TextAlign::Justify];
            let items = [(Icon::AlignLeft, "Align left", true), (Icon::AlignCenter, "Align center", true), (Icon::AlignRight, "Align right", true), (Icon::AlignJustify, "Justify", true)];
            let current = aligns.iter().position(|&a| a == ed.text.align).unwrap_or(0);
            if let Some(i) = widgets::segmented(ui, &items, current, 28.0, false) {
                ed.text.align = aligns[i];
                if let Some(s) = &mut ed.text.editing {
                    s.text.align = aligns[i];
                    write(ed);
                }
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Event, Key, Modifiers};
    use minnamate_core::doc::{Document, Symbol, SymbolKind};

    use super::*;
    use crate::app::testkit::*;
    use crate::app::PropsTab;

    /// The text on stage at the current frame.
    fn texts(ed: &Editor) -> Vec<Text> {
        let element = |r: ElementRef| ed.timeline().layers[r.layer].frames[r.keyframe].elements[r.element].clone();
        ed.all_elements().into_iter().filter_map(|r| if let Element::Text(t) = element(r) { Some(t) } else { None }).collect()
    }

    fn strings(ed: &Editor) -> Vec<String> {
        texts(ed).iter().map(text::string).collect()
    }

    fn type_text(h: &mut Harness<'_, Editor>, s: &str) {
        h.event(Event::Text(s.into()));
        h.step();
    }

    fn press(h: &mut Harness<'_, Editor>, mods: Modifiers, key: Key) {
        h.key_press_modifiers(mods, key);
        h.step();
    }

    #[test]
    fn typing_makes_static_text() {
        let mut h = blank();
        h.state_mut().tool = Tool::Text;
        mouse_click(&mut h, (100.0, 80.0));
        assert!(h.state().text.editing.is_some());
        assert!(texts(h.state()).is_empty(), "nothing in the document until something's typed");

        type_text(&mut h, "Hello");
        press(&mut h, Modifiers::NONE, Key::Enter);
        // Tool letters type, rather than switching tools.
        press(&mut h, Modifiers::NONE, Key::V);
        type_text(&mut h, "vow");
        h.key_down(Key::Space);
        type_text(&mut h, " ");
        h.key_up(Key::Space);
        h.step();
        let ed = h.state();
        assert_eq!(ed.tool, Tool::Text);
        assert_eq!(strings(ed), ["Hello\nvow "]);
        let t = &texts(ed)[0];
        assert_eq!((t.kind, t.width, t.runs[0].face.as_str(), t.runs[0].size), (TextKind::Static, None, "TimesNewRomanPSMT", 12.0));
        assert!((t.matrix.tx - 100.0).abs() < 1.0 && (t.matrix.ty - 80.0).abs() < 1.0, "{:?}", t.matrix);
        assert_eq!(t.runs[0].color, rgba(ed.fill_color), "in the fill color");

        // Ctrl+A selects the text, not the stage; typing replaces it.
        press(&mut h, Modifiers::COMMAND, Key::A);
        type_text(&mut h, "Hi");
        assert_eq!(strings(h.state()), ["Hi"]);
        assert!(h.state().selection.is_empty());

        // Escape ends typing, and one undo takes the whole edit away.
        press(&mut h, Modifiers::NONE, Key::Escape);
        assert!(h.state().text.editing.is_none());
        press(&mut h, Modifiers::COMMAND, Key::Z);
        assert!(strings(h.state()).is_empty(), "{:?}", strings(h.state()));
        press(&mut h, Modifiers::COMMAND, Key::Y);
        assert_eq!(strings(h.state()), ["Hi"]);
    }

    #[test]
    fn clicking_text_types_in_it_and_emptied_text_goes() {
        let mut h = blank();
        h.state_mut().tool = Tool::Text;
        mouse_click(&mut h, (100.0, 80.0));
        type_text(&mut h, "abc");
        // A click outside ends it and starts another, which goes when left empty.
        mouse_click(&mut h, (300.0, 300.0));
        assert_eq!(strings(h.state()), ["abc"]);
        mouse_click(&mut h, (400.0, 300.0));
        assert_eq!(strings(h.state()), ["abc"]);

        // Clicking the text types in it, at the caret.
        mouse_click(&mut h, (100.0, 88.0));
        let s = h.state().text.editing.as_ref().expect("typing");
        assert!(s.target.is_some() && s.cursor == (0, 0), "caret before the first character: {:?}", s.cursor);
        press(&mut h, Modifiers::NONE, Key::End);
        type_text(&mut h, "d");
        assert_eq!(strings(h.state()), ["abcd"]);

        // Emptied, it goes when typing ends; undo brings it back.
        press(&mut h, Modifiers::COMMAND, Key::A);
        press(&mut h, Modifiers::NONE, Key::Backspace);
        assert_eq!(strings(h.state()), [""]);
        h.state_mut().tool = Tool::Selection;
        h.step();
        assert!(strings(h.state()).is_empty() && h.state().text.editing.is_none());
        press(&mut h, Modifiers::COMMAND, Key::Z);
        assert_eq!(strings(h.state()), ["abc"]);
    }

    #[test]
    fn empty_input_text_stays() {
        // Unlike static text, a field the viewer types in (or the movie fills) may be empty.
        let mut h = blank();
        let run = TextRun { text: String::new(), ..new_run(h.state()) };
        let matrix = Matrix { tx: 100.0, ty: 80.0, ..Matrix::IDENTITY };
        let field = Text { kind: TextKind::Input, matrix, width: Some(80.0), align: TextAlign::Left, line_spacing: 2.0, runs: vec![run], extra: Extra::default() };
        h.state_mut().doc.scenes[0].layers[0].frames[0].elements.push(Element::Text(field));
        h.state_mut().tool = Tool::Text;
        mouse_click(&mut h, (140.0, 88.0));
        assert!(h.state().text.editing.as_ref().is_some_and(|s| s.target.is_some()), "typing in the field");
        press(&mut h, Modifiers::NONE, Key::Escape);
        assert_eq!(strings(h.state()), [""]);
    }

    #[test]
    fn each_edit_undoes_its_own_typing() {
        let mut h = blank();
        h.state_mut().tool = Tool::Text;
        mouse_click(&mut h, (100.0, 80.0));
        type_text(&mut h, "Hello");
        // Still for over a second, the TextEdit keeps "Hello" as an undo point.
        for _ in 0..70 {
            h.step();
        }
        press(&mut h, Modifiers::NONE, Key::Escape);
        mouse_click(&mut h, (100.0, 200.0));
        type_text(&mut h, "x");
        // While typing, Ctrl+Z undoes typing, and only this text's.
        press(&mut h, Modifiers::COMMAND, Key::Z);
        assert_eq!(strings(h.state()), ["Hello", ""]);
    }

    #[test]
    fn leaving_the_symbol_ends_typing_in_it() {
        let mut h = blank();
        let ed = h.state_mut();
        let timeline = Document::new(1.0, 1.0, 1.0).scenes.remove(0);
        ed.doc.library.insert("Sym".into(), Symbol { kind: SymbolKind::Graphic, timeline, extra: Extra::default() });
        ed.edit_library_symbol("Sym");
        ed.tool = Tool::Text;
        mouse_click(&mut h, (100.0, 80.0));
        type_text(&mut h, "a");
        press(&mut h, Modifiers::NONE, Key::Backspace);
        // The edit bar's Back button: the text emptied in the symbol goes, as on any other exit.
        h.state_mut().exit_symbol();
        h.step();
        let elements = &h.state().doc.library["Sym"].timeline.layers[0].frames[0].elements;
        assert!(elements.is_empty() && h.state().text.editing.is_none(), "{elements:?}");
    }

    #[test]
    fn a_press_in_the_timeline_ends_typing() {
        let mut h = blank();
        h.state_mut().tool = Tool::Text;
        mouse_click(&mut h, (100.0, 80.0));
        type_text(&mut h, "abc");
        // The layer's cell at the playhead: its frame and the text get selected, and F5 is the timeline's again.
        click_cell(&mut h, (0, 0), Modifiers::NONE);
        press(&mut h, Modifiers::NONE, Key::F5);
        assert_eq!((h.state().timeline().layers[0].frames[0].duration, strings(h.state())), (2, vec!["abc".to_string()]));
        // Its name too.
        mouse_click(&mut h, (100.0, 200.0));
        type_text(&mut h, "de");
        let name = h.state().layer_rows.left_top() + vec2(80.0, h.state().tl.row_h / 2.0);
        screen_click(&mut h, name, Modifiers::NONE);
        press(&mut h, Modifiers::NONE, Key::F5);
        assert!(h.state().text.editing.is_none() && h.state().timeline().layers[0].frames[0].duration > 2);
    }

    #[test]
    fn a_new_document_ends_typing() {
        let mut h = blank();
        h.state_mut().tool = Tool::Text;
        mouse_click(&mut h, (100.0, 80.0));
        h.state_mut().new_document(550.0, 400.0, 24.0);
        h.step();
        assert!(h.state().text.editing.is_none(), "the old document's text box followed");
    }

    #[test]
    fn dragging_makes_a_fixed_width_that_wraps() {
        let mut h = blank();
        h.state_mut().tool = Tool::Text;
        mouse_drag(&mut h, (100.0, 80.0), (160.0, 90.0));
        type_text(&mut h, "one two three four");
        let t = &texts(h.state())[0];
        let width = t.width.expect("a fixed width");
        assert!((width - 60.0).abs() < 1.0, "{width}");
        let layout = text::layout(t);
        assert!(layout.lines.len() > 1 && (layout.width - width).abs() < 1e-3, "{layout:?}");
    }

    #[test]
    fn double_clicking_text_with_the_selection_tool_types_in_it() {
        let mut h = blank();
        h.state_mut().tool = Tool::Text;
        mouse_click(&mut h, (100.0, 80.0));
        type_text(&mut h, "abc");
        h.state_mut().tool = Tool::Selection;
        h.step();
        assert!(h.state().text.editing.is_none());

        // Long enough after the click that made the text not to count as a triple click.
        let p = on_stage(&h, (110.0, 88.0));
        h.hover_at(p);
        for _ in 0..40 {
            h.step();
        }
        for _ in 0..2 {
            h.drag_at(p);
            h.drop_at(p);
            h.step();
        }
        assert_eq!(h.state().tool, Tool::Text);
        assert!(h.state().text.editing.as_ref().is_some_and(|s| s.target.is_some()));
        type_text(&mut h, "x");
        assert_eq!(strings(h.state()).len(), 1);
        assert!(strings(h.state())[0].contains('x'));
    }

    #[test]
    fn properties_restyle_the_text_being_typed() {
        let mut h = blank();
        h.state_mut().tool = Tool::Text;
        mouse_click(&mut h, (100.0, 80.0));
        type_text(&mut h, "abc");
        let ed = h.state_mut();
        ed.fill_color = Color32::RED;
        ed.text.size = 30.0;
        restyle(ed, |run| run.size = 30.0);
        h.step();
        let t = &texts(h.state())[0];
        assert_eq!((t.runs[0].size, t.runs[0].color), (30.0, [1.0, 0.0, 0.0, 1.0]));
        type_text(&mut h, "d");
        assert_eq!(texts(h.state())[0].runs.len(), 1, "typed on in the same style");
    }

    #[test]
    fn diff_finds_the_change() {
        assert_eq!(diff("abc", "abXc"), (2..2, "X".into()));
        assert_eq!(diff("abc", "ac"), (1..2, String::new()));
        assert_eq!(diff("abc", "xyz"), (0..3, "xyz".into()));
        assert_eq!(diff("", "é"), (0..0, "é".into()));
    }

    /// Writes target/screenshots/text.png: typing on the tween fixture, with Properties > Tool.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn text_screenshot() {
        let mut h = harness(true);
        h.run();
        h.state_mut().tool = Tool::Text;
        h.state_mut().props_tab = PropsTab::Tool;
        h.state_mut().text.size = 28.0;
        mouse_click(&mut h, (190.0, 60.0));
        type_text(&mut h, "Minnamate");
        press(&mut h, Modifiers::NONE, Key::Enter);
        type_text(&mut h, "static text");
        press(&mut h, Modifiers::SHIFT, Key::Home);
        for _ in 0..3 {
            h.step();
        }
        screenshot(&mut h, "text");
    }
}
