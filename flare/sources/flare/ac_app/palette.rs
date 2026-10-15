//! Command search (Ctrl+Shift+P; after Flare Studio's command search, our own
//! code): type part of a command or tool name, Enter runs the first match,
//! arrows move, Escape closes. Studio uses Ctrl+K, which is Align here.

use crate::files::{FileAction, Pending};
use crate::toolbar::{all_tools, Tool};
use crate::App;
use eframe::egui::{self, Color32, Key, RichText};

type Run = fn(&mut App);

/// (menu path, name, shortcut, action)
const COMMANDS: &[(&str, &str, &str, Run)] = &[
    ("File", "New…", "Ctrl+N", |a| a.guarded(Pending::New)),
    ("File", "Open…", "Ctrl+O", |a| a.guarded(Pending::Open)),
    ("File", "Open Demo Scene", "", |a| a.guarded(Pending::OpenDemo)),
    ("File", "Save", "Ctrl+S", |a| a.save()),
    ("File", "Save As…", "Ctrl+Shift+S", |a| a.file(FileAction::Save)),
    ("File › Import", "Import to Stage…", "Ctrl+R", |a| a.file(FileAction::ImportImage)),
    ("File › Import", "Import Image Sequence…", "", |a| a.file(FileAction::ImportSequence)),
    ("File › Import", "Import Photoshop (PSD)…", "", |a| a.file(FileAction::ImportPsd)),
    ("File › Import", "Import SVG…", "", |a| a.file(FileAction::ImportSvg)),
    ("File › Import", "Import Sound…", "", |a| a.file(FileAction::ImportSound)),
    ("File › Import", "Import XDTS Timing Sheet…", "", |a| a.file(FileAction::ImportXdts)),
    ("Swatches", "Add Swatches from File…", "", |a| {
        a.swatch_ui.replace = false;
        a.file(FileAction::ImportSwatches)
    }),
    ("Swatches", "Save Swatches…", "", |a| a.file(FileAction::ExportSwatches)),
    ("File › Export", "Export Image…", "", |a| a.publish.image_open = true),
    ("File › Export", "Export SVG…", "", |a| a.file(FileAction::ExportSvg)),
    ("File › Export", "Export Animated GIF…", "", |a| a.file(FileAction::ExportGif)),
    ("File › Export", "Export PNG Sequence…", "", |a| a.file(FileAction::ExportPngSequence)),
    ("File › Export", "Export Video…", "", |a| a.file(FileAction::ExportVideo)),
    ("File › Export", "Publish HTML5 Canvas…", "", |a| a.file(FileAction::ExportHtml5)),
    ("File › Export", "Generate Sprite Sheet…", "", |a| a.file(FileAction::ExportSpriteSheet)),
    ("File › Export", "Export Animated SVG…", "", |a| a.file(FileAction::ExportAnimatedSvg)),
    ("File › Export", "Export Lottie JSON…", "", |a| a.file(FileAction::ExportLottie)),
    ("File › Export", "Export .fla / XFL…", "", |a| a.file(FileAction::ExportXfl)),
    ("File › Export", "Export Timing Sheet (XDTS)…", "", |a| a.file(FileAction::ExportXdts)),
    ("File", "Publish Settings…", "Ctrl+Shift+F12", |a| a.publish.open = true),
    ("Edit", "Undo", "Ctrl+Z", |a| {
        a.ed.undo();
    }),
    ("Edit", "Redo", "Ctrl+Shift+Z", |a| {
        a.ed.redo();
    }),
    ("Edit", "Cut", "Ctrl+X", |a| a.cut()),
    ("Edit", "Copy", "Ctrl+C", |a| a.copy()),
    ("Edit", "Paste in Center", "Ctrl+V", |a| a.paste(false)),
    ("Edit", "Paste in Place", "Ctrl+Shift+V", |a| a.paste(true)),
    ("Edit", "Duplicate", "Ctrl+D", |a| a.ed.duplicate()),
    ("Edit", "Delete", "Delete", |a| a.delete()),
    ("Edit", "Select All", "Ctrl+A", |a| a.ed.select_all()),
    ("Edit", "Preferences…", "Ctrl+U", |a| a.prefs_open = true),
    ("Edit", "Deselect All", "Esc", |a| a.ed.deselect()),
    ("Modify", "Convert to Symbol…", "F8", |a| a.open_symbol_dialog(true)),
    ("Insert", "New Symbol…", "Ctrl+F8", |a| a.open_symbol_dialog(false)),
    ("Modify", "Document…", "Ctrl+J", |a| a.doc_dlg = Some(crate::windows::DocDlg::of(a))),
    ("Modify", "Break Apart", "Ctrl+B", |a| a.ed.break_apart()),
    ("Modify", "Group", "Ctrl+G", |a| {
        a.ed.group_selected();
    }),
    ("Modify", "Ungroup", "Ctrl+Shift+G", |a| {
        a.ed.ungroup_selected();
    }),
    ("Modify", "Scale and Rotate…", "Ctrl+Alt+S", |a| a.open_scale_rotate()),
    ("Modify", "Add Shape Hint", "Ctrl+Shift+H", |a| a.add_shape_hint()),
    ("Modify › Shape", "Convert Lines to Fills", "", |a| a.ed.shape_fill_op("lines_to_fills", 0.0, 0, false)),
    ("Modify › Shape", "Expand Fill…", "", |a| a.shape_dlg = Some(crate::windows::ShapeDlg { soften: false, dist: 4.0, steps: 4, inset: false })),
    ("Modify › Shape", "Soften Fill Edges…", "", |a| a.shape_dlg = Some(crate::windows::ShapeDlg { soften: true, dist: 4.0, steps: 4, inset: false })),
    ("Timeline", "Insert Frame", "F5", |a| a.ed.insert_frame()),
    ("Timeline", "Remove Frame", "Shift+F5", |a| a.ed.remove_frame()),
    ("Timeline", "Insert Keyframe", "F6", |a| a.ed.insert_keyframe()),
    ("Timeline", "Clear Keyframe", "Shift+F6", |a| a.ed.clear_keyframe()),
    ("Timeline", "Insert Blank Keyframe", "F7", |a| a.ed.insert_blank_keyframe()),
    ("Timeline", "Reverse Frames", "", |a| a.ed.reverse_frames()),
    ("Timeline", "Synchronize Symbols", "", |a| sync_symbols(a)),
    ("Timeline", "Show Time in Seconds", "", |a| {
        a.tl.seconds = !a.tl.seconds;
        a.settings.tl_seconds = a.tl.seconds;
    }),
    ("Modify", "Duplicate Symbol and Swap", "", |a| {
        if let Some(id) = a.ed.duplicate_and_swap("") {
            a.status = format!("Swapped to {}", a.ed.doc.symbol(id).map(|s| s.name.as_str()).unwrap_or(""));
        }
    }),
    ("Insert", "New Layer", "", |a| a.ed.add_layer()),
    ("Insert", "Layer Folder", "", |a| {
        a.ed.wrap_layer(ac_core::LayerKind::Folder, "Folder");
    }),
    ("Insert", "Mask Layer", "", |a| {
        a.ed.wrap_layer(ac_core::LayerKind::Mask, "Mask");
    }),
    ("Insert", "Classic Motion Guide", "", |a| {
        a.ed.wrap_layer(ac_core::LayerKind::Guide, "Motion Guide");
    }),
    ("Insert", "Scene", "", |a| {
        a.ed.add_scene();
        a.fit_pending = true;
    }),
    ("Control", "Play / Stop", "Enter", |a| a.toggle_play()),
    ("Control", "Test Movie", "Ctrl+Enter", |a| crate::publish::test_movie(a)),
    ("Control", "Rewind", "", |a| a.step(-100_000)),
    ("Control", "Go to End", "", |a| a.step(100_000)),
    ("Control", "Step Forward One Frame", ".", |a| a.step(1)),
    ("Control", "Step Backward One Frame", ",", |a| a.step(-1)),
    ("Control", "Next Keyframe", "Shift+.", |a| a.step_keyframe(true)),
    ("Control", "Previous Keyframe", "Shift+,", |a| a.step_keyframe(false)),
    ("Control", "Loop Playback", "", |a| a.loop_play = !a.loop_play),
    ("Control", "Loop Range from Selection", "Ctrl+Alt+L", |a| a.set_loop_range()),
    ("Control", "Clear Loop Range", "", |a| a.tl.loop_range = None),
    ("Control", "Onion Skin", "", |a| a.onion = !a.onion),
    ("View", "Zoom In", "Ctrl+=", |a| a.zoom *= 1.25),
    ("View", "Zoom Out", "Ctrl+-", |a| a.zoom /= 1.25),
    ("View", "100%", "Ctrl+1", |a| a.zoom = 1.0),
    ("View", "Fit in Window", "Ctrl+2", |a| a.fit_pending = true),
    ("View", "Rulers", "Ctrl+Alt+Shift+R", |a| a.view.rulers = !a.view.rulers),
    ("View", "Show Grid", "Ctrl+'", |a| a.view.grid = !a.view.grid),
    ("View", "Snap to Grid", "Ctrl+Shift+'", |a| a.view.snap_grid = !a.view.snap_grid),
    ("View", "Show Guides", "Ctrl+;", |a| a.view.guides = !a.view.guides),
    ("View", "Clip Content Outside Stage", "Ctrl+Shift+W", |a| a.view.clip = !a.view.clip),
    ("Window", "Library", "Ctrl+L", |a| a.lib.open = !a.lib.open),
    ("Window", "Swatches", "Ctrl+F9", |a| a.toggle_swatches()),
    ("Window", "Color", "Ctrl+Shift+F9", |a| a.colors.open = !a.colors.open),
    ("Window", "Navigator", "", |a| a.nav.open = !a.nav.open),
    ("Window", "History", "Ctrl+F10", |a| a.history_open = !a.history_open),
    ("Window", "Tool Options Bar", "", |a| a.toggle_options_bar()),
    ("Window", "Transform", "Ctrl+T", |a| a.transform_open = !a.transform_open),
    ("Window", "Align", "Ctrl+K", |a| a.align_open = !a.align_open),
    ("Window", "Info", "Ctrl+I", |a| a.info_open = !a.info_open),
    ("Window", "Motion Editor", "", |a| a.motion.open = !a.motion.open),
    ("Window", "Scene", "Shift+F2", |a| a.scene_open = !a.scene_open),
    ("Window", "Actions", "F9", |a| a.scripting.actions_open = true),
    ("Window", "Output", "F2", |a| a.scripting.output_open = true),
    ("Window", "Script Console", "", |a| a.scripting.console_open = true),
    ("Window", "Edit Toolbar…", "", |a| a.topts.edit_toolbar_open = true),
    ("Window", "Reset Layout", "", |a| a.reset_layout()),
];

#[derive(Default)]
pub(crate) struct Palette {
    pub open: bool,
    query: String,
    cursor: usize,
    focus: bool,
}

enum Entry {
    Command(usize),
    Tool(Tool),
}

/// How well `name` matches `q`: words of the query must each appear (prefix
/// of a word scores best). None = no match.
pub(crate) fn score(name: &str, q: &str) -> Option<u32> {
    let n = name.to_lowercase();
    let mut total = 0;
    for w in q.to_lowercase().split_whitespace() {
        let pos = n.find(w)?;
        let word_start = pos == 0 || !n.as_bytes()[pos - 1].is_ascii_alphanumeric();
        total += if pos == 0 { 0 } else if word_start { 1 } else { 3 };
    }
    Some(total)
}

fn matches(q: &str) -> Vec<Entry> {
    let mut v: Vec<(u32, usize, Entry)> = Vec::new();
    for (i, (path, name, _, _)) in COMMANDS.iter().enumerate() {
        if let Some(s) = score(name, q).or_else(|| score(&format!("{path} {name}"), q).map(|s| s + 4)) {
            v.push((s, i, Entry::Command(i)));
        }
    }
    for (i, t) in all_tools().filter(|t| t.usable()).enumerate() {
        let name = format!("{} tool", t.info().name);
        if let Some(s) = score(&name, q) {
            v.push((s, 1000 + i, Entry::Tool(t)));
        }
    }
    v.sort_by_key(|(s, i, _)| (*s, *i));
    v.into_iter().map(|(_, _, e)| e).collect()
}

pub(crate) fn shortcut(app: &mut App, ctx: &egui::Context) {
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::P)) {
        app.palette.open = !app.palette.open;
        app.palette.query.clear();
        app.palette.cursor = 0;
        app.palette.focus = true;
    }
}

pub(crate) fn window(app: &mut App, ctx: &egui::Context) {
    if !app.palette.open {
        return;
    }
    let found = matches(&app.palette.query);
    let (up, down, enter, esc) = ctx.input_mut(|i| {
        (i.consume_key(egui::Modifiers::NONE, Key::ArrowUp), i.consume_key(egui::Modifiers::NONE, Key::ArrowDown), i.consume_key(egui::Modifiers::NONE, Key::Enter), i.consume_key(egui::Modifiers::NONE, Key::Escape))
    });
    let p = &mut app.palette;
    if down {
        p.cursor = (p.cursor + 1).min(found.len().saturating_sub(1));
    }
    if up {
        p.cursor = p.cursor.saturating_sub(1);
    }
    p.cursor = p.cursor.min(found.len().saturating_sub(1));
    let mut run: Option<usize> = enter.then_some(p.cursor);
    let screen = ctx.screen_rect();
    egui::Area::new(egui::Id::new("command-search")).order(egui::Order::Foreground).fixed_pos(egui::pos2(screen.center().x - 260.0, screen.top() + 60.0)).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(8.0).show(ui, |ui| {
            ui.set_width(520.0);
            let r = ui.add(egui::TextEdit::singleline(&mut p.query).hint_text("Search commands and tools…").desired_width(f32::INFINITY));
            if p.focus {
                r.request_focus();
                p.focus = false;
            }
            if r.changed() {
                p.cursor = 0;
            }
            ui.add_space(4.0);
            if found.is_empty() {
                ui.label(RichText::new("No matching command").color(Color32::from_gray(150)));
            }
            egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                for (k, e) in found.iter().enumerate().take(60) {
                    let (path, name, sc) = match e {
                        Entry::Command(i) => (COMMANDS[*i].0.to_string(), COMMANDS[*i].1.to_string(), COMMANDS[*i].2.to_string()),
                        Entry::Tool(t) => ("Tools".to_string(), format!("{} tool", t.info().name), t.shortcut_label()),
                    };
                    let sel = k == p.cursor;
                    let resp = ui
                        .horizontal(|ui| {
                            ui.set_width(ui.available_width());
                            let r = ui.selectable_label(sel, name);
                            ui.label(RichText::new(path).small().color(Color32::from_gray(130)));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(sc).small().color(Color32::from_gray(150)));
                            });
                            r
                        })
                        .inner;
                    if sel && (up || down) {
                        resp.scroll_to_me(None);
                    }
                    if resp.clicked() {
                        run = Some(k);
                    }
                }
            });
            ui.label(RichText::new("Enter runs · ↑↓ move · Esc closes").small().color(Color32::from_gray(130)));
        });
    });
    if esc {
        app.palette.open = false;
        return;
    }
    if let Some(k) = run {
        app.palette.open = false;
        match found.get(k) {
            Some(Entry::Command(i)) => {
                let (_, name, _, f) = COMMANDS[*i];
                let before = app.status.clone();
                f(app);
                if app.status == before {
                    app.status = name.to_string();
                }
            }
            Some(Entry::Tool(t)) => app.set_tool(*t),
            None => {}
        }
    }
}

/// Timeline › Synchronize Symbols, with a status line.
pub(crate) fn sync_symbols(a: &mut App) {
    let n = a.ed.synchronize_symbols();
    a.status = if n == 0 { "Symbols already in sync (graphic instances of the same symbol on consecutive keyframes)".into() } else { format!("Synchronized {n} keyframe(s)/instance(s)") };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_ranks_prefixes_and_needs_every_word() {
        assert_eq!(score("Insert Keyframe", "key"), Some(1));
        assert_eq!(score("Insert Keyframe", "ins"), Some(0));
        assert_eq!(score("Insert Keyframe", "ins key"), Some(1));
        assert_eq!(score("Insert Keyframe", "blank"), None);
        let found = matches("blank key");
        assert!(matches!(found.first(), Some(Entry::Command(i)) if COMMANDS[*i].1 == "Insert Blank Keyframe"));
        let found = matches("rect");
        assert!(found.iter().any(|e| matches!(e, Entry::Tool(Tool::Rect))));
        // every command name is unique within its menu
        let mut seen = std::collections::HashSet::new();
        for (p, n, _, _) in COMMANDS {
            assert!(seen.insert((p, n)), "{p} {n}");
        }
        assert!(matches("").len() > COMMANDS.len());
    }
}

