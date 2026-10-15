//! Window > Actions (F9): the selected keyframe's frame script in a code editor (line
//! numbers; keywords, strings and comments coloured), each run of typing one undo step;
//! it floats, as Animate's does. Also Control > Enable Simple Frame Actions, under which
//! playback on the stage honours stop, play and goto.

use eframe::egui::{self, Align, Align2, Color32, FontId, Id, Layout, Rect, ScrollArea, Sense, Stroke, TextFormat, Ui, pos2, text::LayoutJob, vec2};
use minnamate_core::doc::LayerKind;
use minnamate_core::script::{self, Command, Dialect, Lex, Step, Stmt, Track};

use crate::app::Editor;
use crate::icons::{self, Icon};
use crate::{theme, widgets};

#[derive(Default)]
pub struct State {
    /// The panel is open.
    pub open: bool,
    /// Pin Script: the keyframe shown whatever is selected: where it was pinned (scene and
    /// symbols being edited), its layer and frame.
    pinned: Option<(usize, Vec<String>, usize, u32)>,
    /// Control > Enable Simple Frame Actions.
    pub simple: bool,
    /// The playback `simple_tick` counts from, (start, from frame), and the frames it has stepped.
    played: Option<((f64, u32), u32)>,
    /// A run of typing in the code editor is going on, its undo step taken: the document's
    /// revision after its last edit. Anything else that changes the document ends the run.
    typing: Option<u32>,
}

// The code editor's colours, ours.
const KEYWORD: Color32 = Color32::from_rgb(0x6C, 0xA3, 0xE8);
const BUILTIN: Color32 = Color32::from_rgb(0x6F, 0xC3, 0xC0);
const STRING: Color32 = Color32::from_rgb(0x9C, 0xC7, 0x7E);
const NUMBER: Color32 = Color32::from_rgb(0xD9, 0xB6, 0x76);
const COMMENT: Color32 = Color32::from_rgb(0x84, 0x8C, 0x84);
const LINE_NUMBER: Color32 = Color32::from_rgb(0x74, 0x74, 0x74);
/// The line-number column: its width and fill.
const GUTTER: f32 = 38.0;
const GUTTER_BG: Color32 = Color32::from_rgb(0x2B, 0x2B, 0x2B);

const KEYWORDS: &[&str] = &[
    "as", "break", "case", "catch", "class", "const", "continue", "default", "delete", "do", "dynamic", "each", "else",
    "extends", "false", "final", "finally", "for", "function", "get", "if", "implements", "import", "in", "instanceof",
    "interface", "internal", "is", "let", "new", "null", "override", "package", "private", "protected", "public",
    "return", "set", "static", "super", "switch", "this", "throw", "true", "try", "typeof", "undefined", "var", "void",
    "while", "with",
];
/// What the subset knows by name.
const BUILTINS: &[&str] = &[
    "stop", "play", "gotoAndPlay", "gotoAndStop", "nextFrame", "prevFrame", "nextScene", "prevScene", "trace",
    "navigateToURL", "getURL", "URLRequest", "MovieClip", "parent", "root", "exportRoot", "currentFrame", "totalFrames",
    "console", "log", "window", "open", "getChildByName",
];

/// Window > Actions: opens the panel, or closes it.
pub fn toggle(ed: &mut Editor) {
    ed.actions.open = !ed.actions.open;
}

/// The keyframe the panel shows, as (layer, keyframe): the pinned one, or the one whose span
/// holds the selected frame. None where there's no keyframe to hold a script.
fn target(ed: &Editor) -> Option<(usize, usize)> {
    let here = (ed.scene, ed.editing.iter().map(|t| t.symbol.clone()).collect::<Vec<_>>());
    let (l, f) = match &ed.actions.pinned {
        Some((scene, symbols, l, f)) if (*scene, symbols) == (here.0, &here.1) => (*l, *f),
        _ => (ed.layer, ed.frame),
    };
    let layer = ed.timeline().layers.get(l).filter(|l| l.kind != LayerKind::Folder)?;
    Some((l, layer.keyframe_at(f)?))
}

/// "Layer_1 : 5", as the panel names a keyframe's script.
fn name(ed: &Editor, (l, k): (usize, usize)) -> String {
    let layer = &ed.timeline().layers[l];
    format!("{} : {}", layer.name, layer.frames[k].index + 1)
}

pub fn show(ctx: &egui::Context, ed: &mut Editor) {
    // Window > Hide Panels (F4) hides it with the docks, and brings it back with them.
    let docks = ed.show_tools || ed.show_right || ed.show_bottom;
    if !ed.actions.open || ed.home.shown || !docks {
        return;
    }
    let frame = egui::Frame::NONE.fill(theme::PANEL_BG).stroke(Stroke::new(1.0, theme::POPUP_BORDER)).shadow(ctx.global_style().visuals.window_shadow);
    let mut open = true;
    egui::Window::new("Actions")
        .id(Id::new("actions-panel"))
        .title_bar(false)
        .frame(frame)
        .default_rect(Rect::from_center_size(ctx.content_rect().center(), vec2(620.0, 360.0)))
        .min_size(vec2(360.0, 180.0))
        .resizable(true)
        .show(ctx, |ui| panel(ui, ed, &mut open));
    ed.actions.open &= open;
}

fn panel(ui: &mut Ui, ed: &mut Editor, open: &mut bool) {
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
    // The floating group's bar: it moves the panel; its close box closes it.
    let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), 14.0), Sense::hover());
    ui.painter().rect_filled(bar, 0.0, theme::BAR);
    let close = ui.interact(Rect::from_center_size(pos2(bar.right() - 9.0, bar.center().y), vec2(14.0, 14.0)), ui.id().with("close"), Sense::click());
    icons::paint(ui.painter(), close.rect.shrink(3.5), Icon::WinClose, if close.hovered() { Color32::WHITE } else { theme::TEXT_DIM });
    *open &= !close.on_hover_text("Close").clicked();

    let target = target(ed);
    let pinned = ed.actions.pinned.is_some();
    theme::tab_strip_menu(ui, &["Actions"], 0, Some(|ui: &mut Ui| {
        if theme::menu_toggle(ui, target.is_some() || pinned, pinned, "Pin Script").clicked() {
            pin(ed, target);
        }
        ui.separator();
        // ponytail: the code editor's own commands aren't built; egui's TextEdit undoes and redoes.
        for (label, keys) in [("Find and Replace", "Ctrl+F"), ("Go to Line...", ""), ("Auto Format", "Ctrl+Shift+F"), ("Comment Selection", "Ctrl+M"), ("Uncomment Selection", "Ctrl+Shift+M")] {
            theme::menu_item(ui, false, label, keys);
        }
        ui.separator();
        theme::menu_toggle(ui, false, true, "Line Numbers");
        theme::menu_toggle(ui, false, true, "Word Wrap");
        ui.separator();
        theme::menu_item(ui, false, "Help", "");
        ui.separator();
        for label in ["Close", "Close Group"] {
            *open &= !theme::menu_item(ui, true, label, "").clicked();
        }
    }));
    toolbar(ui, ed, target);
    let status_h = 22.0;
    let height = (ui.available_height() - status_h).max(40.0);
    let (code, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    ui.painter().rect_filled(code, 0.0, theme::INSET);
    let mut cursor = None;
    match target {
        Some(at) => cursor = editor(&mut ui.new_child(egui::UiBuilder::new().max_rect(code)), ed, at),
        None => {
            let message = egui::RichText::new("Current selection cannot have actions applied to it.").color(theme::TEXT_DIM);
            ui.put(code, egui::Label::new(message));
        }
    }
    // The status bar: the script's place, and where the cursor is in it.
    let (status, _) = ui.allocate_exact_size(vec2(ui.available_width(), status_h), Sense::hover());
    let p = ui.painter();
    p.hline(status.x_range(), status.top() + 0.5, Stroke::new(1.0, theme::BORDER));
    if let Some(at) = target {
        let mark = Rect::from_center_size(pos2(status.left() + 14.0, status.center().y), vec2(12.0, 12.0));
        p.circle_filled(mark.center(), 2.75, theme::TEXT_DIM);
        p.text(pos2(mark.right() + 4.0, status.center().y), Align2::LEFT_CENTER, name(ed, at), FontId::proportional(11.5), theme::TEXT);
    }
    if let Some((line, lines, col)) = cursor {
        let text = format!("Line {line} of {lines}, Col {col}");
        p.text(pos2(status.right() - 20.0, status.center().y), Align2::RIGHT_CENTER, text, FontId::proportional(11.5), theme::TEXT_DIM);
    }
}

/// Pin Script: keeps showing this keyframe's script, or lets the panel follow the selection again.
fn pin(ed: &mut Editor, target: Option<(usize, usize)>) {
    ed.actions.pinned = match (&ed.actions.pinned, target) {
        (None, Some((l, k))) => {
            let symbols = ed.editing.iter().map(|t| t.symbol.clone()).collect();
            Some((ed.scene, symbols, l, ed.timeline().layers[l].frames[k].index))
        }
        _ => None,
    };
}

/// The toolbar: the script picker (this timeline's scripts, as the script navigator lists
/// them), then Pin Script and the editor's buttons.
fn toolbar(ui: &mut Ui, ed: &mut Editor, target: Option<(usize, usize)>) {
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::hover());
    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(row.shrink2(vec2(8.0, 0.0))).layout(Layout::left_to_right(Align::Center)));
    ui.spacing_mut().item_spacing.x = 2.0;
    let current = target.map_or_else(|| "No script".to_string(), |at| name(ed, at));
    let scripts: Vec<(usize, u32, String)> = ed
        .timeline()
        .layers
        .iter()
        .enumerate()
        .flat_map(|(l, layer)| layer.frames.iter().enumerate().filter(|(_, k)| k.script().is_some()).map(move |(k, key)| (l, k, key.index)))
        .map(|(l, k, f)| (l, f, name(ed, (l, k))))
        .collect();
    let picked = widgets::dropdown(&mut ui, "Script", &current, 170.0, |ui| {
        if scripts.is_empty() {
            widgets::choice(ui, false, false, "No frame scripts here");
        }
        scripts.iter().find(|(_, _, label)| widgets::choice(ui, *label == current, true, label).clicked()).map(|&(l, f, _)| (l, f))
    });
    if let Some((l, f)) = picked.flatten() {
        // The script's keyframe becomes the selected frame, as the navigator selects it.
        ed.actions.pinned = None;
        ed.layer = l;
        ed.set_frame(f);
        ed.tl.sel = [(l, f)].into();
        crate::timeline::keep_selection(ed);
    }
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        theme::icon_button(ui, 24.0, false, false, "Help", |p, r, c| {
            p.text(r.center(), Align2::CENTER_CENTER, "?", FontId::proportional(13.0), c);
        });
        // ponytail: Code Snippets, Format Code, Find and Insert Target Path aren't built.
        for (icon, tip) in [(Icon::Code, "Code Snippets"), (Icon::AlignLeft, "Format code"), (Icon::Zoom, "Find"), (Icon::CenterStage, "Insert instance path and name")] {
            theme::icon_button(ui, 24.0, false, false, tip, |p, r, c| icons::paint(p, r.shrink(5.0), icon, c));
        }
        let pinned = ed.actions.pinned.is_some();
        if theme::icon_button(ui, 24.0, pinned, target.is_some() || pinned, "Pin script", |p, r, c| icons::paint(p, r.shrink(5.0), Icon::Pin, c)).clicked() {
            pin(ed, target);
        }
    });
}

/// The code editor on keyframe `(l, k)`'s script. Returns the cursor's line, the line count
/// and the cursor's column, for the status bar.
fn editor(ui: &mut Ui, ed: &mut Editor, (l, k): (usize, usize)) -> Option<(usize, usize, usize)> {
    let mut text = ed.timeline().layers[l].frames[k].script().unwrap_or_default();
    let font = FontId::monospace(12.0);
    let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, wrap: f32| {
        let mut job = colored(buf.as_str(), &font);
        job.wrap.max_width = wrap;
        ui.fonts_mut(|f| f.layout_job(job))
    };
    let area = ui.max_rect();
    ui.painter().rect_filled(Rect::from_min_size(area.min, vec2(GUTTER, area.height())), 0.0, GUTTER_BG);
    let out = ScrollArea::vertical().id_salt("actions-code").auto_shrink(false).show(ui, |ui| {
        ui.horizontal_top(|ui| {
            ui.add_space(GUTTER + 8.0);
            let edit = egui::TextEdit::multiline(&mut text)
                .id(Id::new(("actions-code", l, k)))
                .code_editor()
                .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(0, 4)))
                .desired_width(f32::INFINITY)
                .min_size(vec2(0.0, area.height()))
                .layouter(&mut layouter)
                .show(ui);
            // A number by the first row of each line.
            let p = ui.painter();
            let mut line = 0;
            for (i, row) in edit.galley.rows.iter().enumerate() {
                if i == 0 || edit.galley.rows[i - 1].ends_with_newline {
                    line += 1;
                    let at = pos2(area.left() + GUTTER - 8.0, edit.galley_pos.y + row.pos.y);
                    p.text(at, Align2::RIGHT_TOP, line.to_string(), font.clone(), LINE_NUMBER);
                }
            }
            edit
        })
        .inner
    });
    let edit = out.inner;
    if edit.response.changed() {
        // A run of typing is one undo step, until the editor lets go of the keyboard or the
        // document changes otherwise (another one opened, say). Not a gesture: those Escape
        // cancels, where leaving the code editor keeps what was typed.
        if ed.actions.typing == Some(ed.rev) {
            (ed.rev, ed.dirty) = (ed.rev + 1, true);
        } else {
            ed.checkpoint();
        }
        // The timeline's "a" follows the script.
        ed.timeline_mut().layers[l].frames[k].set_script(&text);
        ed.actions.typing = Some(ed.rev);
    }
    if !edit.response.has_focus() {
        ed.actions.typing = None;
    }
    let lines = text.split('\n').count();
    let at = edit.cursor_range.map_or(0, |r| r.primary.index.0);
    let before: String = text.chars().take(at).collect();
    let line = before.matches('\n').count() + 1;
    let col = before.rsplit('\n').next().map_or(0, |s| s.chars().count()) + 1;
    Some((line, lines, col))
}

/// `text` in the editor's font, coloured by what its tokens are.
fn colored(text: &str, font: &FontId) -> LayoutJob {
    let mut job = LayoutJob::default();
    let put = |job: &mut LayoutJob, part: &str, color: Color32| job.append(part, 0.0, TextFormat::simple(font.clone(), color));
    let mut at = 0;
    for (kind, range) in script::lex(text) {
        put(&mut job, &text[at..range.start], theme::TEXT);
        let word = &text[range.clone()];
        let color = match kind {
            Lex::Comment => COMMENT,
            Lex::Text => STRING,
            Lex::Number => NUMBER,
            Lex::Word if KEYWORDS.contains(&word) => KEYWORD,
            Lex::Word if BUILTINS.contains(&word) => BUILTIN,
            _ => theme::TEXT,
        };
        put(&mut job, word, color);
        at = range.end;
    }
    put(&mut job, &text[at..], theme::TEXT);
    job
}

/// Control > Play with Enable Simple Frame Actions on: as `Editor::tick` does, frame by
/// frame, each frame entered running its scripts' stop, play and goto on this timeline (and,
/// on a scene's, gotos to other scenes). The rest waits for Test Movie.
pub fn simple_tick(ed: &mut Editor, ctx: &egui::Context, start: f64, from: u32) {
    let now = ctx.input(|i| i.time);
    let fps = ed.doc.frame_rate.max(0.01) as f64;
    let due = ((now - start).max(0.0) * fps) as u32;
    let mut done = match ed.actions.played {
        Some((key, done)) if key == (start, from) => done,
        _ => 0,
    };
    while done < due && ed.playing == Some((start, from)) {
        done += 1;
        let (first, last) = crate::timeline::loop_range(ed);
        if ed.loop_playback && ed.frame >= last {
            ed.frame = first;
        } else if ed.frame + 1 < ed.frame_count() {
            ed.frame += 1;
        } else {
            ed.playing = None;
            break;
        }
        run_frame(ed, now, 0);
    }
    ed.actions.played = ed.playing.map(|key| (key, if key == (start, from) { done } else { 0 }));
    if ed.playing.is_some() {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(1.0 / fps));
    }
}

/// Runs the simple frame actions of the frame the playhead entered. A goto that lands on
/// a frame runs that frame's too, up to a few deep.
fn run_frame(ed: &mut Editor, now: f64, depth: u32) {
    let scene = ed.editing.is_empty();
    let dialect = Dialect::of(&ed.doc);
    let mut commands = Vec::new();
    {
        let track = if scene { Track::movie(&ed.doc) } else { Track::symbol(ed.timeline()) };
        let here = if scene { track.scenes[ed.scene].start + ed.frame } else { ed.frame };
        for (_, script) in track.scripts(here, dialect) {
            for stmt in script.stmts {
                // This timeline's own commands; a scene's are also the root's.
                if let Stmt::Run { path, command } = stmt
                    && (path.is_empty() || (scene && path == [Step::Root]))
                {
                    commands.push(command);
                }
            }
        }
    }
    let mut moved = false;
    for command in commands {
        match command {
            Command::Stop => ed.playing = None,
            Command::Play if ed.playing.is_none() => ed.playing = Some((now, ed.frame)),
            Command::Play | Command::Trace(_) | Command::OpenUrl { .. } => {}
            command => {
                let went = {
                    let track = if scene { Track::movie(&ed.doc) } else { Track::symbol(ed.timeline()) };
                    let here = if scene { track.scenes[ed.scene].start + ed.frame } else { ed.frame };
                    track.go(here, &command).map(|(frame, play)| (track.at(frame), play))
                };
                let Some(((s, frame), play)) = went else { continue };
                moved |= (scene && s != ed.scene) || frame != ed.frame;
                if scene && s != ed.scene {
                    ed.go_to_scene(s);
                }
                ed.frame = frame;
                ed.playing = play.then_some((now, frame));
            }
        }
    }
    if moved && depth < 8 {
        run_frame(ed, now, depth + 1);
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Key, Modifiers};
    use egui_kittest::kittest::Queryable;
    use minnamate_core::doc::Document;

    use super::*;
    use crate::app::Action;
    use crate::app::testkit::*;

    /// A 10-frame scene with keyframes at frames 5 and 8 (1-based) holding `scripts`.
    fn scripted(h: &mut Harness<'_, Editor>, scripts: [&str; 2]) {
        let ed = h.state_mut();
        let layer = &mut ed.doc.scenes[0].layers[0];
        layer.insert_frames(0, 9);
        for (f, script) in [4, 7].into_iter().zip(scripts) {
            layer.insert_keyframe(f);
            let k = layer.keyframe_at(f).unwrap();
            layer.frames[k].set_script(script);
        }
        h.run();
    }

    #[test]
    fn f9_opens_the_panel_on_the_selected_keyframe_and_typing_is_one_undo_step() {
        let mut h = blank();
        scripted(&mut h, ["stop();", ""]);
        h.state_mut().set_frame(5);
        h.key_press(Key::F9);
        h.run();
        assert!(h.state().actions.open);
        h.get_by_label("Script").hover();
        let ed = h.state();
        assert_eq!(target(ed).map(|at| name(ed, at)).as_deref(), Some("Layer_1 : 5"), "frame 6 is in frame 5's span");

        // Typing into the code editor sets the script as it goes.
        let code = h.state().timeline().layers[0].frames[1].script();
        assert_eq!(code.as_deref(), Some("stop();"));
        h.get_by_role(egui::accesskit::Role::MultilineTextInput).click();
        h.run();
        h.key_press_modifiers(Modifiers::COMMAND, Key::A);
        h.event(egui::Event::Text("gotoAndPlay(1);".into()));
        h.run();
        h.event(egui::Event::Text(" // loop".into()));
        h.key_press(Key::Escape);
        h.run();
        let script = |h: &Harness<'_, Editor>| h.state().timeline().layers[0].frames[1].script();
        assert_eq!(script(&h).as_deref(), Some("gotoAndPlay(1); // loop"), "Escape leaves what was typed");
        // The whole run goes in one step.
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.run();
        assert_eq!(script(&h).as_deref(), Some("stop();"));

        // On a frame past the layer's end there's no keyframe to hold a script.
        h.state_mut().set_frame(20);
        h.run();
        assert!(target(h.state()).is_none());
        h.get_by_label_contains("cannot have actions applied");
    }

    #[test]
    fn a_frames_actions_opens_the_panel_on_it() {
        let mut h = blank();
        scripted(&mut h, ["stop();", "gotoAndPlay(2);"]);
        h.key_press(Key::F9);
        h.run();
        // With the panel open, the frame menu's Actions shows that frame's script; F9 alone toggles.
        let at = cell(&h, 0, 8);
        h.hover_at(at);
        h.step();
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: Modifiers::NONE });
            h.step();
        }
        h.run();
        h.get_by_label("Actions F9").click();
        h.run();
        let ed = h.state();
        assert!(ed.actions.open);
        assert_eq!(target(ed).map(|at| name(ed, at)).as_deref(), Some("Layer_1 : 8"));
        // Hide Panels (F4) hides it with the rest, and F4 again brings it back.
        h.key_press(Key::F4);
        h.run();
        assert!(h.query_by_label("Script").is_none());
        h.key_press(Key::F4);
        h.run();
        h.get_by_label("Script");
    }

    #[test]
    fn typing_on_in_another_document_is_its_own_undo_step() {
        let mut h = blank();
        let script = |h: &Harness<'_, Editor>| h.state().timeline().layers[0].frames[0].script();
        h.state_mut().doc.scenes[0].layers[0].frames[0].set_script("stop();");
        h.key_press(Key::F9);
        h.run();
        h.get_by_role(egui::accesskit::Role::MultilineTextInput).click();
        h.run();
        h.event(egui::Event::Text("a".into()));
        h.run();
        // A document opened while the code editor keeps the keyboard (Ctrl+O runs while typing),
        // with a script on its first frame too: the same code editor goes on.
        h.state_mut().new_document(550.0, 400.0, 24.0);
        h.state_mut().doc.scenes[0].layers[0].frames[0].set_script("stop();");
        h.run();
        h.event(egui::Event::Text("b".into()));
        h.run();
        assert!(script(&h).is_some_and(|s| s.contains('b') && !s.contains('a')), "{:?}", script(&h));
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::Undo, &ctx);
        h.run();
        assert_eq!(script(&h).as_deref(), Some("stop();"), "what was typed in this document undoes");
    }

    #[test]
    fn simple_frame_actions_stop_and_goto_on_the_stage() {
        let mut h = blank();
        scripted(&mut h, ["stop();", "gotoAndPlay(2);"]);
        let ctx = h.ctx.clone();
        // Off, Control > Play passes over scripts, as Animate's does.
        let now = ctx.input(|i| i.time);
        h.state_mut().toggle_play(now);
        h.state_mut().playing = Some((now - 6.5 / 24.0, 0));
        h.step();
        assert_eq!(h.state().frame, 6);
        // On (Ctrl+Alt+F), playback stops on frame 5.
        h.state_mut().set_frame(0);
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, Key::F);
        h.step();
        assert!(h.state().actions.simple);
        let now = ctx.input(|i| i.time);
        h.state_mut().playing = Some((now - 6.5 / 24.0, 0));
        h.step();
        assert_eq!((h.state().frame, h.state().playing), (4, None));
        // Played on from there, frame 8 sends it back to frame 2, and it plays on.
        let now = ctx.input(|i| i.time);
        h.state_mut().playing = Some((now - 3.5 / 24.0, 4));
        h.step();
        assert_eq!(h.state().frame, 1);
        assert!(h.state().playing.is_some());
        h.state_mut().playing = None;
    }

    #[test]
    fn test_movie_stops_where_the_script_says() {
        let mut h = blank();
        scripted(&mut h, ["stop(); trace(\"stopped on \" + currentFrame);", ""]);
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::TestMovie, &ctx);
        // At 24 fps, 90 steps of 1/60 s are 36 frames: well past frame 5.
        for _ in 0..90 {
            h.step();
        }
        let ed = h.state();
        let player = &ed.test.as_ref().expect("Test Movie is up").player;
        assert_eq!((player.root.frame, player.root.playing), (4, false));
        assert_eq!(ed.output.last().map(String::as_str), Some("stopped on 5"));
        assert_eq!(ed.bottom_tab, 1, "the Output panel comes forward with what's traced");
    }

    #[test]
    fn new_document_makes_html5_canvas_documents() {
        let mut h = Harness::builder().with_size(egui::vec2(1600.0, 900.0)).build_eframe(|cc| Editor::new(&cc.egui_ctx, None));
        h.run();
        // Character Animation makes ActionScript 3.0; the Advanced tab's HTML5 Canvas tile makes Canvas.
        for (tab, canvas) in [(None, false), (Some("Ads"), true), (Some("Advanced"), true)] {
            h.get_by_label("Create New").click();
            h.run();
            if let Some(tab) = tab {
                h.get_by_label(tab).click();
                h.run();
            }
            if tab == Some("Advanced") {
                h.get_by_label("HTML5 Canvas").click();
                h.run();
            }
            h.get_by_label("Create").click();
            h.run();
            assert_eq!(h.state().doc.is_canvas(), canvas, "{tab:?}");
            crate::home::toggle(h.state_mut());
            h.run();
        }
    }

    #[test]
    fn a_canvas_document_called_off_leaves_the_next_one_be() {
        let mut h = Harness::builder().with_size(egui::vec2(1600.0, 900.0)).build_eframe(|cc| Editor::new(&cc.egui_ctx, None));
        h.run();
        h.get_by_label("Full HD").click();
        h.run();
        h.state_mut().dirty = true;
        // The Ads tab makes an HTML5 Canvas document, but Cancel at "Save changes?" calls it off.
        h.key_press_modifiers(Modifiers::COMMAND, Key::N);
        h.run();
        h.get_by_label("Ads").click();
        h.run();
        h.get_by_label("Create").click();
        h.run();
        h.get_by_label_contains("Cancel").click();
        h.run();
        // Home's Full HD then makes an ActionScript 3.0 document, as ever.
        crate::home::toggle(h.state_mut());
        h.run();
        h.get_by_label("Full HD").click();
        h.run();
        h.get_by_label_contains("Don't Save").click();
        h.run();
        assert!(!h.state().dirty, "a new document");
        assert!(!h.state().doc.is_canvas());
    }

    #[test]
    fn canvas_documents_read_their_scripts_as_javascript() {
        let mut doc = Document::new(550.0, 400.0, 24.0);
        doc.set_canvas(true);
        assert_eq!(Dialect::of(&doc), Dialect::JavaScript);
        let job = colored("this.stop(); // x", &FontId::monospace(12.0));
        let colors: Vec<_> = job.sections.iter().map(|s| (&job.text[s.byte_range.start.0..s.byte_range.end.0], s.format.color)).filter(|(t, _)| !t.trim().is_empty()).collect();
        assert_eq!(colors[0], ("this", KEYWORD));
        assert_eq!(colors[2], ("stop", BUILTIN));
        assert_eq!(colors.last(), Some(&("// x", COMMENT)));
    }

    /// Writes target/screenshots/actions_panel.png: the Actions panel over the tween fixture,
    /// on a keyframe with a script.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn actions_panel_screenshot() {
        let mut h = harness(true);
        h.run();
        let ed = h.state_mut();
        let layer = &mut ed.doc.scenes[0].layers[0];
        layer.insert_keyframe(4);
        let script = "// Hold here until the ball comes back.\nstop();\n\nif (ball.x > 300) {\n    gotoAndPlay(\"intro\", \"Scene 1\");\n}\ntrace(\"Frame \" + currentFrame, 0x1F);\n";
        layer.frames[1].set_script(script);
        // Inside the keyframe's span, so its "a" shows by the playhead.
        ed.set_frame(7);
        ed.actions.open = true;
        h.run();
        screenshot(&mut h, "actions_panel");
    }
}
