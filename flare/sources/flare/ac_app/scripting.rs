//! Scripting UI: Actions panel (frame scripts), Script Console (authoring
//! scripts in several languages), Output panel, Command Reference, and the
//! frame-script hook used during playback.

use crate::App;
use ac_script::{FramePlayer, Language, PlayAction, ScriptOptions, ScriptReport};
use eframe::egui::{self, Color32, Key, Modifiers, RichText};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OutKind {
    Info,
    Print,
    Error,
}

pub enum Pending {
    Code(Language, String),
    File(PathBuf),
}

pub struct ScriptingState {
    pub actions_open: bool,
    pub console_open: bool,
    pub output_open: bool,
    pub reference_open: bool,
    pub frame_scripts: bool,
    pub player: Option<FramePlayer>,
    pub output: Vec<(OutKind, String)>,
    pub lang: Language,
    pub code: Vec<(Language, String)>,
    pub run_path: String,
    pub timeout_secs: u64,
    pub trust_external: bool,
    pub confirm: Option<Pending>,
    /// Actions panel buffer: (target key, text, dirty)
    actions_key: Option<(Option<u32>, usize, u32)>,
    actions_buf: String,
    actions_dirty: bool,
    /// Instance properties set by frame scripts during this playback (by instance name).
    pub overrides: std::collections::HashMap<String, ac_script::Instance>,
}

impl Default for ScriptingState {
    fn default() -> Self {
        ScriptingState {
            overrides: Default::default(),
            actions_open: false,
            console_open: false,
            output_open: false,
            reference_open: false,
            frame_scripts: true,
            player: None,
            output: vec![],
            lang: Language::Js,
            code: Language::ALL.iter().map(|l| (*l, l.example().to_string())).collect(),
            run_path: String::new(),
            timeout_secs: 30,
            trust_external: false,
            confirm: None,
            actions_key: None,
            actions_buf: String::new(),
            actions_dirty: false,
        }
    }
}

impl ScriptingState {
    fn code_mut(&mut self, l: Language) -> &mut String {
        let i = self.code.iter().position(|(x, _)| *x == l).unwrap();
        &mut self.code[i].1
    }
    pub fn push(&mut self, kind: OutKind, s: impl Into<String>) {
        self.output.push((kind, s.into()));
        if self.output.len() > 2000 {
            self.output.drain(..500);
        }
    }
}

// ------------------------------------------------------------------ running authoring scripts

pub fn request_run(app: &mut App, p: Pending) {
    let external = match &p {
        // JSFL can read and write files (FLfile), so it asks first like external languages
        Pending::Code(l, _) => *l != Language::Js,
        Pending::File(f) => f.extension().map(|e| e != "js").unwrap_or(true),
    };
    if external && !app.scripting.trust_external {
        app.scripting.confirm = Some(p);
    } else {
        run_now(app, p);
    }
}

fn run_now(app: &mut App, p: Pending) {
    let s = &mut app.scripting;
    s.output_open = true;
    let mut opts = ScriptOptions { timeout: Duration::from_secs(s.timeout_secs.max(1)), ..Default::default() };
    let (label, rep): (String, ScriptReport) = match p {
        Pending::Code(lang, code) => {
            s.push(OutKind::Info, format!("▶ Running {} from the console…", lang.name()));
            (lang.name().into(), ac_script::run_code(&mut app.ed, lang, &code, &opts))
        }
        Pending::File(path) => {
            s.push(OutKind::Info, format!("▶ Running {}…", path.display()));
            if let Some(d) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
                opts.base_dir = d.to_path_buf();
            }
            (path.display().to_string(), ac_script::run_file(&mut app.ed, &path, &opts))
        }
    };
    let s = &mut app.scripting;
    for l in &rep.log {
        s.push(OutKind::Print, l.clone());
    }
    match &rep.error {
        Some(e) => {
            for line in e.lines() {
                s.push(OutKind::Error, line.to_string());
            }
            app.status = format!("Script failed: {}", e.lines().next().unwrap_or(""));
        }
        None => app.status = format!("Script finished: {} command(s) in {:.0?}. Ctrl+Z undoes the whole run.", rep.commands, rep.elapsed),
    }
    app.scripting.push(OutKind::Info, format!("■ {label}: {} command(s), {:.0?}{}", rep.commands, rep.elapsed, if rep.ok() { "" } else { ", failed" }));
    app.fit_pending |= false;
}

// ------------------------------------------------------------------ frame scripts during playback

/// Called when playback starts.
pub fn start_playback(app: &mut App) {
    app.scripting.player = None;
    app.scripting.overrides.clear();
    enter_frame(app);
}

/// Run frame scripts on the keyframes starting at the current frame, and
/// apply their stop/play/goto requests (following jumps a few times).
pub fn enter_frame(app: &mut App) {
    if !app.scripting.frame_scripts {
        return;
    }
    for _ in 0..8 {
        let tl = app.ed.timeline();
        let f = app.ed.frame;
        let scripts = ac_script::scripts_at(tl, f);
        if scripts.is_empty() {
            return;
        }
        let labels = ac_script::labels(tl);
        let total = tl.frame_count();
        let mut player = match app.scripting.player.take() {
            Some(p) => p,
            None => match FramePlayer::new() {
                Ok(p) => p,
                Err(e) => {
                    app.scripting.push(OutKind::Error, format!("frame scripts unavailable: {e}"));
                    return;
                }
            },
        };
        let mut jumped = false;
        let mut instances = named_instances(app);
        for code in scripts {
            match player.run_with(&code, f, total, &labels, &instances) {
                Ok((actions, changed)) => {
                    for c in changed {
                        if let Some(i) = instances.iter_mut().find(|i| i.name == c.name) {
                            *i = c.clone();
                        }
                        app.scripting.overrides.insert(c.name.clone(), c);
                    }
                    for a in actions {
                        match a {
                            // stop() holds the main timeline; movie clips keep playing
                            PlayAction::Stop => app.halted = true,
                            PlayAction::Play => app.halted = false,
                            PlayAction::Goto { frame, play } => {
                                app.ed.frame = frame;
                                app.halted = !play;
                                jumped = frame != f;
                            }
                        }
                    }
                }
                Err(e) => {
                    app.playing = false;
                    app.scripting.output_open = true;
                    app.scripting.push(OutKind::Error, format!("Frame {} script error: {e}", f + 1));
                    app.status = format!("Frame script error on frame {} (see Output)", f + 1);
                    break;
                }
            }
        }
        for l in player.take_log() {
            app.scripting.output_open = true;
            app.scripting.push(OutKind::Print, l);
        }
        app.scripting.player = Some(player);
        if !jumped {
            return;
        }
    }
}

// ------------------------------------------------------------------ windows

pub fn shortcuts(app: &mut App, ctx: &egui::Context) {
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F9)) {
        app.scripting.actions_open = !app.scripting.actions_open;
    }
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F2)) {
        app.scripting.output_open = !app.scripting.output_open;
    }
}

pub fn windows(app: &mut App, ctx: &egui::Context) {
    actions_panel(app, ctx);
    console(app, ctx);
    output(app, ctx);
    reference(app, ctx);
    confirm(app, ctx);
}

const SNIPPETS: &[(&str, &str)] = &[
    ("Stop", "this.stop();"),
    ("Go to frame and play", "this.gotoAndPlay(0); // 0 = first frame"),
    ("Go to label and play", "this.gotoAndPlay(\"start\");"),
    ("Go to frame and stop", "this.gotoAndStop(0);"),
    ("Trace (print)", "trace(\"frame\", this.currentFrame, \"of\", this.totalFrames);"),
    ("Loop 3 times then stop", "this.loops = (this.loops || 0) + 1;\nif (this.loops >= 3) this.stop();\nelse this.gotoAndPlay(0);"),
];

fn actions_panel(app: &mut App, ctx: &egui::Context) {
    if !app.scripting.actions_open {
        return;
    }
    let layer = app.ed.layer;
    let frame = app.ed.frame;
    let tl = app.ed.timeline();
    let lname = tl.layers.get(layer).map(|l| l.name.clone()).unwrap_or_default();
    let key = (app.ed.editing, layer, frame);
    let existing = tl.layers.get(layer).and_then(|l| l.keyframes.get(&frame)).map(|k| k.script.clone().unwrap_or_default());
    let is_key = existing.is_some();
    let st = &mut app.scripting;
    if st.actions_key != Some(key) && !st.actions_dirty {
        st.actions_key = Some(key);
        st.actions_buf = existing.clone().unwrap_or_default();
    }
    let mut open = true;
    let mut apply = false;
    let mut revert = false;
    egui::Window::new("Actions").open(&mut open).default_size([460.0, 320.0]).resizable(true).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{lname} : frame {}", frame + 1)).strong());
            if !is_key {
                ui.label(RichText::new("(not a keyframe: applying creates one)").small().color(Color32::from_rgb(0xFF, 0xB0, 0x40)));
            }
            if st.actions_dirty {
                ui.label(RichText::new("● unsaved").small().color(Color32::from_rgb(0xFF, 0xB0, 0x40)));
            }
        });
        ui.horizontal(|ui| {
            ui.menu_button("Insert snippet", |ui| {
                for (name, code) in SNIPPETS {
                    if ui.button(*name).clicked() {
                        if !st.actions_buf.is_empty() && !st.actions_buf.ends_with('\n') {
                            st.actions_buf.push('\n');
                        }
                        st.actions_buf.push_str(code);
                        st.actions_buf.push('\n');
                        st.actions_dirty = true;
                        ui.close_menu();
                    }
                }
            });
            if ui.button("Apply").on_hover_text("Save this script on the keyframe (Ctrl+Enter)").clicked() {
                apply = true;
            }
            if ui.add_enabled(st.actions_dirty, egui::Button::new("Revert")).clicked() {
                revert = true;
            }
            ui.checkbox(&mut st.frame_scripts, "Run during playback");
        });
        let r = ui.add(egui::TextEdit::multiline(&mut st.actions_buf).code_editor().desired_rows(10).desired_width(f32::INFINITY).hint_text("// e.g.  this.stop();"));
        if r.changed() {
            st.actions_dirty = true;
        }
        if r.has_focus() && ui.input(|i| i.modifiers.command && i.key_pressed(Key::Enter)) {
            apply = true;
        }
        ui.label(
            RichText::new(
                "JavaScript, Animate HTML5 Canvas style: `this` is the timeline. this.stop(), this.play(), this.gotoAndPlay(frameOrLabel), \
                 this.gotoAndStop(…), this.currentFrame, this.totalFrames, trace(…). Frames are 0-based here, as in Animate's HTML5 Canvas. \
                 Frame scripts are sandboxed: no files, network or editor access; 100 ms and 32 MB per frame.",
            )
            .small()
            .color(Color32::from_gray(150)),
        );
    });
    if apply {
        let code = app.scripting.actions_buf.clone();
        app.ed.set_frame_script(&code);
        app.scripting.actions_dirty = false;
        app.scripting.actions_key = None;
        app.status = if code.trim().is_empty() { "Frame script removed".into() } else { format!("Frame script saved on frame {}", frame + 1) };
    }
    if revert {
        app.scripting.actions_dirty = false;
        app.scripting.actions_key = None;
    }
    app.scripting.actions_open = open;
}

fn console(app: &mut App, ctx: &egui::Context) {
    if !app.scripting.console_open {
        return;
    }
    let mut open = true;
    let mut run: Option<Pending> = None;
    egui::Window::new("Script Console").open(&mut open).default_size([520.0, 380.0]).resizable(true).show(ctx, |ui| {
        let st = &mut app.scripting;
        ui.horizontal(|ui| {
            ui.label("Language");
            egui::ComboBox::from_id_salt("script-lang").selected_text(st.lang.name()).show_ui(ui, |ui| {
                for l in Language::ALL {
                    ui.selectable_value(&mut st.lang, l, l.name());
                }
            });
            let lang = st.lang;
            if ui.button("Example").on_hover_text("Replace with the example script").clicked() {
                *st.code_mut(lang) = lang.example().to_string();
            }
            if ui.button("Clear").clicked() {
                st.code_mut(lang).clear();
            }
        });
        let lang = st.lang;
        if lang == Language::Jsfl {
            ui.label(RichText::new("Built in. Runs scripts written for the authoring tool's JSFL API (fl, document, timeline, library); unsupported calls say so.").small().color(Color32::from_gray(150)));
        } else if lang != Language::Js {
            ui.label(RichText::new(format!("Runs with the {} installed on your computer (must be on PATH).", match lang { Language::Python => "Python", Language::Node => "Node.js", Language::Ruby => "Ruby", Language::Js | Language::Jsfl => "" })).small().color(Color32::from_gray(150)));
        }
        let r = ui.add(egui::TextEdit::multiline(st.code_mut(lang)).code_editor().desired_rows(12).desired_width(f32::INFINITY));
        let ctrl_enter = r.has_focus() && ui.input(|i| i.modifiers.command && i.key_pressed(Key::Enter));
        ui.horizontal(|ui| {
            if ui.button(RichText::new("▶ Run").strong()).on_hover_text("Ctrl+Enter").clicked() || ctrl_enter {
                let code = st.code_mut(lang).clone();
                run = Some(Pending::Code(lang, code));
            }
            ui.label("Timeout");
            ui.add(egui::DragValue::new(&mut st.timeout_secs).range(1..=600).suffix(" s"));
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Script file");
            ui.add(egui::TextEdit::singleline(&mut st.run_path).hint_text("path/to/script.js | .jsfl | .py | .mjs | .rb").desired_width(260.0));
            if ui.add_enabled(!st.run_path.trim().is_empty(), egui::Button::new("Run file")).clicked() {
                run = Some(Pending::File(PathBuf::from(st.run_path.trim())));
            }
        });
        ui.label(RichText::new("Scripts call the same commands as the menus (see Commands › Command Reference). A whole run is one undo step.").small().color(Color32::from_gray(150)));
    });
    app.scripting.console_open = open;
    if let Some(p) = run {
        request_run(app, p);
    }
}

fn output(app: &mut App, ctx: &egui::Context) {
    if !app.scripting.output_open {
        return;
    }
    let mut open = true;
    egui::Window::new("Output").open(&mut open).default_size([480.0, 220.0]).resizable(true).show(ctx, |ui| {
        ui.horizontal(|ui| {
            if ui.button("Clear").clicked() {
                app.scripting.output.clear();
            }
            if ui.button("Copy all").clicked() {
                let all: String = app.scripting.output.iter().map(|(_, s)| s.as_str()).collect::<Vec<_>>().join("\n");
                ui.ctx().copy_text(all);
            }
        });
        egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
            for (k, line) in &app.scripting.output {
                let c = match k {
                    OutKind::Info => Color32::from_gray(140),
                    OutKind::Print => Color32::from_gray(225),
                    OutKind::Error => Color32::from_rgb(0xFF, 0x70, 0x70),
                };
                ui.label(RichText::new(line).monospace().color(c));
            }
        });
    });
    app.scripting.output_open = open;
}

fn reference(app: &mut App, ctx: &egui::Context) {
    if !app.scripting.reference_open {
        return;
    }
    let mut open = true;
    egui::Window::new("Command Reference").open(&mut open).default_size([640.0, 440.0]).resizable(true).show(ctx, |ui| {
        ui.label("Every command works the same from every language:");
        ui.label(RichText::new("JavaScript:  ac.draw.rect({x: 10, y: 10, w: 50, h: 50})     Python:  ac.draw.rect(x=10, y=10, w=50, h=50)").monospace().small());
        ui.label(RichText::new("Node:  ac.draw.rect({...})     Ruby:  Flare.call(\"draw.rect\", x: 10, ...)     any language: see protocol below").monospace().small());
        ui.separator();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("cmdref").striped(true).num_columns(3).show(ui, |ui| {
                ui.label(RichText::new("Command").strong());
                ui.label(RichText::new("Arguments").strong());
                ui.label(RichText::new("What it does").strong());
                ui.end_row();
                for c in ac_core::commands::COMMANDS {
                    ui.label(RichText::new(c.id).monospace().color(Color32::from_rgb(0x8C, 0xC8, 0xFF)));
                    ui.label(RichText::new(c.args).monospace().small());
                    ui.label(c.summary);
                    ui.end_row();
                }
                for (id, args, summary) in ac_script::EXTRA_COMMANDS {
                    ui.label(RichText::new(*id).monospace().color(Color32::from_rgb(0x8C, 0xC8, 0xFF)));
                    ui.label(RichText::new(*args).monospace().small());
                    ui.label(*summary);
                    ui.end_row();
                }
            });
            ui.separator();
            ui.label(RichText::new(ac_script::PROTOCOL).monospace().small());
            if let Ok(dir) = ac_script::helper_dir() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("Helper libraries: {}", dir.display())).small());
                    if ui.small_button("Copy path").clicked() {
                        ui.ctx().copy_text(dir.display().to_string());
                    }
                });
            }
        });
    });
    app.scripting.reference_open = open;
}

fn confirm(app: &mut App, ctx: &egui::Context) {
    let Some(p) = &app.scripting.confirm else { return };
    let what = match p {
        Pending::Code(l, _) => format!("this {} script", l.name()),
        Pending::File(f) => format!("“{}”", f.display()),
    };
    let mut choice = 0; // 1 run, 2 run+trust, 3 cancel
    egui::Window::new("Run script?").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
        ui.label(format!("About to run {what}."));
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "Python, Node.js, Ruby and other external scripts run as normal programs with your permissions: \
                 they can read, change or delete your files and use the network. Only run scripts you wrote or trust.",
            )
            .color(Color32::from_rgb(0xFF, 0xC0, 0x60)),
        );
        ui.label(RichText::new("Built-in JavaScript can only edit the document and save/export files you name.").small().color(Color32::from_gray(150)));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("Run").clicked() { choice = 1; }
            if ui.button("Run, and don't ask again this session").clicked() { choice = 2; }
            if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) { choice = 3; }
        });
    });
    match choice {
        1 | 2 => {
            if choice == 2 {
                app.scripting.trust_external = true;
            }
            let p = app.scripting.confirm.take().unwrap();
            run_now(app, p);
        }
        3 => {
            app.scripting.confirm = None;
            app.status = "Script cancelled".into();
        }
        _ => {}
    }
}

/// Named top-level instances at the playhead, with script overrides applied.
fn named_instances(app: &App) -> Vec<ac_script::Instance> {
    let mut out = Vec::new();
    for l in &app.ed.timeline().layers {
        let Some((els, _)) = ac_core::layer_elements_at(l, app.ed.frame) else { continue };
        for e in els {
            let nameable = matches!(e.kind, ac_core::ElementKind::Instance(_) | ac_core::ElementKind::Text(_) | ac_core::ElementKind::Bitmap(_) | ac_core::ElementKind::Group(_));
            if !nameable || e.name.is_empty() || out.iter().any(|i: &ac_script::Instance| i.name == e.name) {
                continue;
            }
            let d = e.matrix.decompose();
            let text = match &e.kind {
                ac_core::ElementKind::Text(t) => Some(t.text.clone()),
                _ => None,
            };
            let base = ac_script::Instance { name: e.name.clone(), x: e.matrix.tx, y: e.matrix.ty, rotation: d.skew_y.to_degrees(), scale_x: d.scale_x, scale_y: d.scale_y, alpha: e.alpha, visible: true, text };
            out.push(app.scripting.overrides.get(&e.name).cloned().unwrap_or(base));
        }
    }
    out
}

/// Apply script-set instance properties to rendered items (playback only).
pub fn apply_overrides(app: &App, items: &mut Vec<ac_core::DrawItem>) {
    if !app.playing || app.scripting.overrides.is_empty() {
        return;
    }
    let tl = app.ed.timeline();
    type Entry = Option<(ac_core::Matrix, f32, ac_script::Instance, Option<ac_core::Path>)>;
    let mut cache: std::collections::HashMap<(usize, usize), Entry> = Default::default();
    items.retain_mut(|it| {
        let Some((l, e)) = it.src else { return true };
        let entry = cache.entry((l, e)).or_insert_with(|| {
            let el = tl.layers.get(l).and_then(|layer| ac_core::layer_elements_at(layer, app.ed.frame)).and_then(|(els, _)| els.get(e).cloned())?;
            let ov = app.scripting.overrides.get(&el.name)?.clone();
            // a script changed a text field's text: lay the new text out
            let path = match (&el.kind, &ov.text) {
                (ac_core::ElementKind::Text(t), Some(s)) if *s != t.text => Some(ac_core::TextEl { text: s.clone(), ..t.clone() }.outline()),
                _ => None,
            };
            Some((el.matrix, el.alpha, ov, path))
        });
        let Some((m, alpha, ov, path)) = entry else { return true };
        if let (Some(p), None) = (path, it.stroke) {
            it.path = p.clone();
        }
        if !ov.visible {
            return false;
        }
        let mut d = m.decompose();
        let rot = ov.rotation.to_radians() - d.skew_y;
        d.skew_x += rot;
        d.skew_y += rot;
        d.scale_x = ov.scale_x;
        d.scale_y = ov.scale_y;
        d.tx = ov.x;
        d.ty = ov.y;
        let want = ac_core::Matrix::compose(&d);
        if let Some(inv) = m.invert() {
            it.matrix = want.concat(&inv).concat(&it.matrix);
        }
        if *alpha > 1e-4 {
            it.alpha *= ov.alpha / *alpha;
        }
        true
    });
}
