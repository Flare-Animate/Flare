//! Button symbols in the editor. A button's timeline names its Up, Over, Down and Hit frames over
//! them, and Control > Enable Simple Buttons (Ctrl+Alt+B) makes the stage's buttons answer the
//! pointer as in Test Movie: Over under it, Down while pressed, as far as their Hit frame reaches.

use std::collections::BTreeSet;

use eframe::egui::{self, Align2, CursorIcon, FontId, Painter, Rect, pos2};
use minnamate_core::button::{self, Live};
use minnamate_core::display::display_list_shown;
use minnamate_core::doc::{Point, SymbolKind};
use minnamate_core::render::render_timeline_live;

use crate::app::{Editor, ViewPort, hash};
use crate::theme;

/// A button's frames are this wide at least, so their state names fit over them.
const CELL_W: f32 = 36.0;

/// Whether the timeline being edited is a button symbol's.
pub fn editing_button(ed: &Editor) -> bool {
    ed.editing.last().and_then(|t| ed.doc.library.get(&t.symbol)).is_some_and(|s| s.kind == SymbolKind::Button)
}

/// The timeline's frame width: as zoomed, but wide enough for the state names in a button.
pub fn cell_w(ed: &Editor) -> f32 {
    if editing_button(ed) { ed.cell_w.max(CELL_W) } else { ed.cell_w }
}

/// Up, Over, Down and Hit over a button's first four frames, in the ruler's tier of numbers at `y`.
pub fn ruler_labels(p: &Painter, ed: &Editor, y: f32, x_of: &dyn Fn(u32) -> f32) {
    if editing_button(ed) {
        for (f, name) in button::STATES.iter().enumerate() {
            p.text(pos2(x_of(f as u32) + 3.0, y), Align2::LEFT_TOP, *name, FontId::proportional(11.0), theme::RULER_TEXT);
        }
    }
}

/// The pointer on a stage button while Enable Simple Buttons is on and the scene is edited: where,
/// in the scene's coordinates, and whether it's pressed. `view` is the stage view's area.
fn live(ed: &Editor, ctx: &egui::Context, stage: Rect, view: Rect) -> Option<(Point, bool)> {
    if !ed.simple_buttons || !ed.editing.is_empty() {
        return None;
    }
    let pos = ctx.input(|i| i.pointer.hover_pos()).filter(|p| view.contains(*p))?;
    let p = crate::stage::local_point(ed, stage, pos);
    let list = display_list_shown(&ed.doc, ed.timeline(), ed.frame, &mut BTreeSet::new());
    button::under(&ed.doc, &list, p).then(|| (p, ctx.input(|i| i.pointer.primary_down())))
}

/// Whether the stage's pointer is on a live button, which then takes clicks from the tools.
pub fn takes_pointer(ed: &Editor, ctx: &egui::Context, stage: Rect, view: Rect) -> bool {
    live(ed, ctx, stage, view).is_some()
}

/// The scene with the button under the pointer hovered or pressed, while Enable Simple Buttons is
/// on; None draws the stage as usual.
pub fn stage_texture(ed: &mut Editor, ctx: &egui::Context, vp: &ViewPort, stage: Rect, view: Rect) -> Option<egui::TextureId> {
    let (p, down) = live(ed, ctx, stage, view)?;
    ctx.set_cursor_icon(CursorIcon::PointingHand);
    let pointer = vp.matrix.apply(p);
    let (frame, scene) = (ed.frame, ed.scene);
    let key = hash((ed.rev, scene, frame, vp.bits(), pointer.0.to_bits(), pointer.1.to_bits(), down));
    let vp = *vp;
    ed.canvas_texture(ctx, "live-buttons", key, |ed| {
        let live = Some(Live { pointer, down });
        render_timeline_live(&ed.doc, &ed.doc.scenes[scene], frame, &vp.matrix, vp.size, 1.0, live).ok().map(|(pixmap, _)| pixmap)
    })
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Key, Modifiers, vec2};
    use minnamate_core::doc::{Element, SymbolKind};
    use minnamate_core::hit::ElementRef;

    use super::*;
    use crate::app::testkit::*;

    /// The tween fixture with its Ball made a button, whose Hit frame is a 120 px square.
    fn with_a_button() -> Harness<'static, Editor> {
        let mut h = harness(false);
        h.run();
        let ed = h.state_mut();
        let ball = ed.doc.library.get_mut("Ball").unwrap();
        ball.kind = SymbolKind::Button;
        let layer = &mut ball.timeline.layers[0];
        layer.insert_blank_keyframe(3);
        let square = minnamate_core::draw::rectangle([(-60.0, -60.0), (60.0, 60.0)], 0.0, None, Some(minnamate_core::doc::Paint::Solid([0.0; 4])));
        minnamate_core::draw::add(layer.frames.last_mut().unwrap(), square);
        if let Some(Element::Instance(i)) = ed.element_mut(ElementRef { layer: 1, keyframe: 0, element: 0 }) {
            i.kind = SymbolKind::Button;
        }
        h.run();
        h
    }

    #[test]
    fn a_buttons_timeline_names_its_states() {
        let mut h = with_a_button();
        h.state_mut().edit_library_symbol("Ball");
        h.run();
        let ed = h.state();
        assert!(editing_button(ed) && cell_w(ed) == CELL_W);
        let shown: Vec<String> = painted_texts(&h).into_iter().map(|(t, _)| t).collect();
        for state in button::STATES {
            assert!(shown.iter().any(|t| t == state), "{state} in {shown:?}");
        }
    }

    /// Writes target/screenshots/button_timeline.png: a button symbol's timeline, its frames named.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn button_timeline_screenshot() {
        let mut h = harness(true);
        h.run();
        let ball = h.state_mut().doc.library.get_mut("Ball").unwrap();
        ball.kind = SymbolKind::Button;
        for f in 1..4 {
            ball.timeline.layers[0].insert_keyframe(f);
        }
        h.state_mut().edit_library_symbol("Ball");
        h.run();
        screenshot(&mut h, "button_timeline");
    }

    #[test]
    fn simple_buttons_take_the_click_from_the_selection_tool() {
        let mut h = with_a_button();
        // The ball sits at (100, 200); its Hit square reaches 60 px each way, past its 20 px face.
        h.state_mut().set_frame(0);
        h.run();
        let ed = h.state();
        let (stage, view) = (ed.stage_rect, ed.stage_rect.expand(200.0));
        let at = on_stage(&h, (150.0, 250.0));
        h.hover_at(at);
        h.step();
        assert!(!takes_pointer(h.state(), &h.ctx, stage, view), "off until Ctrl+Alt+B");
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, Key::B);
        h.step();
        assert!(h.state().simple_buttons);
        h.hover_at(at);
        h.step();
        assert!(takes_pointer(h.state(), &h.ctx, stage, view));
        mouse_click(&mut h, (150.0, 250.0));
        assert!(h.state().selection.is_empty(), "a live button isn't selected by a click");
        let away = on_stage(&h, (400.0, 100.0)) + vec2(0.0, 0.0);
        h.hover_at(away);
        h.step();
        assert!(!takes_pointer(h.state(), &h.ctx, stage, view));
    }
}
