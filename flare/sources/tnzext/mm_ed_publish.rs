//! File > Publish Settings (Ctrl+Shift+F12) and Publish (Shift+Alt+F12), the Doc tab's Publish
//! Settings section, and File > Export: Export Image, Export Animated GIF, Export Movie (SWF or a
//! PNG sequence) and Export Video/Media through ffmpeg (docs/animate-ui.md §7.4, §7.2.2).

use std::path::{Path, PathBuf};

use eframe::egui::{
    self, Align, Align2, Color32, ColorImage, FontId, Frame, Key, Layout, Margin, Rect, RichText, Sense, Stroke, StrokeKind,
    TextureHandle, TextureOptions, Ui, WidgetInfo, WidgetType, pos2, vec2,
};
use minnamate_core::export::{self, Format};
use minnamate_core::publish::{self, Image, Settings, Target};
use minnamate_core::swf_export;

use crate::app::{Action, Editor};
use crate::icons::{self, Icon};
use crate::widgets::{self, Number};
use crate::{home, theme};

/// Publishing in the editor: the document's settings, and the dialogs while they're open.
#[derive(Default)]
pub struct State {
    /// The document's publish profiles: its PublishSettings.xml, written back when it's saved.
    pub settings: Settings,
    /// The document's PublishSettings.xml couldn't be read, so `settings` are a new document's.
    unread: bool,
    /// Publish Settings: the settings as edited so far.
    dialog: Option<Dialog>,
    /// Export Image or Export Animated GIF.
    export: Option<Export>,
}

impl State {
    /// The settings of the document at `path`, or a new document's (also when its file can't be read).
    pub fn read(path: Option<&Path>) -> State {
        let read = path.map(Settings::read).transpose();
        State { unread: read.is_err(), settings: read.ok().flatten().unwrap_or_default(), ..Default::default() }
    }

    /// What saving to `path` writes besides the document: PublishSettings.xml. A file that couldn't
    /// be read is left as it was (saving copies the source's) until the settings are changed.
    pub fn files(&self, path: &Path) -> Vec<(String, Vec<u8>)> {
        if self.unread && self.settings == Settings::default() {
            return Vec::new();
        }
        vec![(publish::FILE.to_string(), self.settings.to_xml(&publish::stem(path)).into_bytes())]
    }
}

/// The formats Publish Settings lists: those it publishes, then the ones it shows greyed.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Shown {
    Swf,
    Html,
    /// An HTML5 Canvas document's page (the profile's HTML file).
    Canvas,
    Gif,
    Jpeg,
    Png,
}

const FORMATS: [(Option<Shown>, &str); 12] = [
    (Some(Shown::Swf), "SWF Movie (.swf)"),
    (None, "SWC"),
    (Some(Shown::Html), "HTML Wrapper"),
    (Some(Shown::Canvas), "HTML5 Canvas"),
    (Some(Shown::Gif), "GIF Image"),
    (Some(Shown::Jpeg), "JPEG Image"),
    (Some(Shown::Png), "PNG Image"),
    (None, "OAM Package"),
    (None, "SVG Image"),
    (None, "SWF Archive"),
    (None, "Mac Projector"),
    (None, "Win Projector"),
];

struct Dialog {
    settings: Settings,
    shown: Shown,
    /// An HTML5 Canvas document: it publishes its page, where an ActionScript one has a SWF and its wrapper.
    canvas: bool,
    /// The current profile's name, being typed over.
    renaming: Option<String>,
}

const WIDTH: f32 = 760.0;
/// The format list and options panes: as tall as the SWF's options, so the dialog keeps its size.
const PANE_H: f32 = 470.0;

/// The commands of this module's menu items.
pub fn command(ed: &mut Editor, action: Action) {
    match action {
        Action::PublishSettings => {
            let canvas = ed.doc.is_canvas();
            let shown = if canvas { Shown::Canvas } else { Shown::Swf };
            ed.publish.dialog = Some(Dialog { settings: ed.publish.settings.clone(), shown, canvas, renaming: None });
        }
        Action::Publish => publish(ed),
        Action::ExportImage | Action::ExportGif => ed.publish.export = Some(Export::new(action == Action::ExportGif)),
        Action::ExportMovie => export_movie(ed),
        Action::ExportVideo => export_video(ed),
        _ => {}
    }
}

/// The dialogs, while open.
pub fn dialogs(ctx: &egui::Context, ed: &mut Editor) {
    settings_dialog(ctx, ed);
    export_dialog(ctx, ed);
}

/// The document's name, as published and exported files take it.
fn stem(ed: &Editor) -> String {
    ed.path.as_deref().map_or_else(|| ed.title.trim_end_matches(".fla").to_string(), publish::stem)
}

/// File > Publish: the current profile's files, beside the saved document.
fn publish(ed: &mut Editor) {
    let Some(path) = ed.path.clone() else {
        ed.error = Some("Save the document first: published files go in its folder.".into());
        return;
    };
    match publish::publish(&ed.doc, ed.publish.settings.current(), &path) {
        Ok(files) => {
            for (file, skipped) in files {
                ed.log(format!("Published {}", file.display()));
                for s in skipped {
                    ed.log(format!("Warning: publishing skipped {s}"));
                }
            }
        }
        Err(e) => ed.error = Some(format!("Couldn't publish.\n\n{e}")),
    }
}

/// The Doc tab's Publish Settings: the profile, the player targeted, and the dialog.
pub fn doc_section(ui: &mut Ui, ed: &mut Editor) {
    let settings = &ed.publish.settings;
    let names: Vec<String> = settings.profiles.iter().map(|p| p.name.clone()).collect();
    let profiles: Vec<(usize, &str, bool)> = names.iter().enumerate().map(|(i, n)| (i, n.as_str(), true)).collect();
    let current = settings.current.min(names.len() - 1);
    let players = publish::players();
    let targets: Vec<(u8, String)> = players.iter().map(|(player, swf)| (*swf, target_name(player, *swf))).collect();
    let targets: Vec<(u8, &str, bool)> = targets.iter().map(|(v, n)| (*v, n.as_str(), true)).collect();
    let version = settings.current().version;
    let (mut profile, mut target) = (None, None);
    widgets::row(ui, |ui| {
        widgets::label(ui, "Profile", widgets::WORD_W);
        profile = widgets::pick(ui, "Profile", current, &profiles, ui.available_width() - 4.0);
    });
    widgets::row(ui, |ui| {
        widgets::label(ui, "Target", widgets::WORD_W);
        target = widgets::pick(ui, "Target", version, &targets, ui.available_width() - 4.0);
    });
    widgets::row(ui, |ui| {
        if widgets::button(ui, "More settings", ui.available_width()).clicked() {
            command(ed, Action::PublishSettings);
        }
    });
    if let Some(i) = profile {
        ed.publish.settings.current = i;
        ed.dirty = true;
    }
    if let Some(v) = target {
        ed.publish.settings.current_mut().version = v;
        ed.dirty = true;
    }
}

/// How Target lists a player: by the SWF version it reads, as Ruffle goes by it.
fn target_name(player: &str, swf: u8) -> String {
    format!("SWF {swf} (player {player})")
}

fn settings_dialog(ctx: &egui::Context, ed: &mut Editor) {
    let stem = stem(ed);
    let dir = ed.path.as_deref().map(publish::folder);
    let stage = (ed.doc.width, ed.doc.height);
    let Some(d) = &mut ed.publish.dialog else { return };
    // An open dropdown, a message over the dialog, or a profile's name being typed takes Enter and Escape.
    let popup_open = egui::Popup::is_any_open(ctx) || ed.error.is_some() || d.renaming.is_some();
    let (mut ok, mut cancel, mut publish_now) = (false, false, false);
    home::dialog("publish-settings", ctx, theme::PANEL_BG).show(ctx, |ui| {
        cancel = home::title_bar(ui, "Publish Settings", WIDTH);
        Frame::NONE.inner_margin(Margin { left: 16, right: 16, top: 12, bottom: 14 }).show(ui, |ui| {
            ui.set_width(WIDTH - 32.0);
            profile_row(ui, d);
            ui.add_space(10.0);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                formats(ui, d);
                let pane = Frame::NONE.fill(theme::INSET).stroke(Stroke::new(1.0, theme::BORDER)).inner_margin(Margin::symmetric(14, 10));
                pane.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.set_min_height(PANE_H);
                    ui.vertical(|ui| options(ui, d, &stem, dir.as_deref(), stage));
                });
            });
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.add_enabled(false, egui::Button::new("Help").min_size(vec2(72.0, 28.0)));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    cancel |= ui.add(egui::Button::new("Cancel").min_size(vec2(72.0, 28.0))).clicked();
                    let (_, rect) = ui.allocate_space(vec2(72.0, 28.0));
                    ok = home::push(ui, rect, "OK", true, 4.0).clicked();
                    publish_now = ui.add(egui::Button::new("Publish").min_size(vec2(72.0, 28.0))).clicked();
                });
            });
        });
    });
    if !popup_open {
        ok |= ctx.input(|i| i.key_pressed(Key::Enter));
        cancel |= ctx.input(|i| i.key_pressed(Key::Escape));
    }
    if cancel {
        ed.publish.dialog = None;
    } else if ok || publish_now {
        // Publish settings are the document's, but not its drawing: they don't go on the undo list.
        if d.settings != ed.publish.settings {
            ed.publish.settings = d.settings.clone();
            ed.dirty = true;
        }
        if ok {
            ed.publish.dialog = None;
        } else {
            publish(ed);
        }
    }
}

/// Profile, with its options menu; Target; Script.
fn profile_row(ui: &mut Ui, d: &mut Dialog) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(RichText::new("Profile:").color(theme::TEXT_DIM));
        match &mut d.renaming {
            Some(name) => {
                let field = ui.add(egui::TextEdit::singleline(name).desired_width(160.0));
                field.request_focus();
                if field.lost_focus() || ui.input(|i| i.key_pressed(Key::Enter)) {
                    let name = name.trim().to_string();
                    let taken = d.settings.profiles.iter().enumerate().any(|(i, p)| i != d.settings.current && p.name == name);
                    if !name.is_empty() && !taken && !ui.input(|i| i.key_pressed(Key::Escape)) {
                        d.settings.current_mut().name = name;
                    }
                    d.renaming = None;
                }
            }
            None => {
                let names: Vec<String> = d.settings.profiles.iter().map(|p| p.name.clone()).collect();
                let items: Vec<(usize, &str, bool)> = names.iter().enumerate().map(|(i, n)| (i, n.as_str(), true)).collect();
                if let Some(i) = widgets::pick(ui, "Publish profile", d.settings.current, &items, 160.0) {
                    d.settings.current = i;
                }
            }
        }
        let gear = theme::icon_button(ui, 24.0, false, true, "Profile options", |p, r, c| icons::paint(p, r.shrink(4.0), Icon::Gear, c));
        egui::Popup::menu(&gear).style(theme::menu_style).show(|ui| {
            ui.set_min_width(180.0);
            let name = |profiles: &Vec<publish::Profile>, base: &str| {
                (1..).map(|n| if n == 1 { base.to_string() } else { format!("{base} {n}") }).find(|n| !profiles.iter().any(|p| &p.name == n)).unwrap_or_default()
            };
            if theme::menu_item(ui, true, "Create Profile", "").clicked() {
                let new = publish::Profile::new(&name(&d.settings.profiles, "Profile"));
                d.settings.profiles.push(new);
                d.settings.current = d.settings.profiles.len() - 1;
            } else if theme::menu_item(ui, true, "Duplicate Profile", "").clicked() {
                let mut copy = d.settings.current().clone();
                copy.name = name(&d.settings.profiles, &format!("{} copy", copy.name));
                d.settings.profiles.push(copy);
                d.settings.current = d.settings.profiles.len() - 1;
            } else if theme::menu_item(ui, true, "Rename Profile", "").clicked() {
                d.renaming = Some(d.settings.current().name.clone());
            } else if theme::menu_item(ui, d.settings.profiles.len() > 1, "Delete Profile", "").clicked() {
                d.settings.profiles.remove(d.settings.current.min(d.settings.profiles.len() - 1));
                d.settings.current = 0;
            }
            ui.separator();
            // ponytail: profiles travel inside the document only.
            theme::menu_item(ui, false, "Import Profile...", "");
            theme::menu_item(ui, false, "Export Profile...", "");
        });
        ui.add_space(24.0);
        ui.label(RichText::new("Target:").color(theme::TEXT_DIM));
        if d.canvas {
            ui.add_enabled_ui(false, |ui| widgets::pick(ui, "Publish target", 0, &[(0, "HTML5 Canvas", true)], 170.0));
        } else {
            let players = publish::players();
            let names: Vec<(u8, String)> = players.iter().map(|(player, swf)| (*swf, target_name(player, *swf))).collect();
            let items: Vec<(u8, &str, bool)> = names.iter().map(|(v, n)| (*v, n.as_str(), true)).collect();
            if let Some(v) = widgets::pick(ui, "Publish target", d.settings.current().version, &items, 170.0) {
                d.settings.current_mut().version = v;
            }
        }
        ui.add_space(24.0);
        ui.label(RichText::new("Script:").color(theme::TEXT_DIM));
        let script = if d.canvas { "JavaScript" } else { "ActionScript 3.0" };
        ui.add_enabled_ui(false, |ui| widgets::pick(ui, "Script", 0, &[(0, script, true)], 130.0));
    });
}

/// The formats, each with its checkbox: a click on the box turns it on or off, on the name shows
/// its options.
fn formats(ui: &mut Ui, d: &mut Dialog) {
    Frame::NONE.fill(theme::INSET).stroke(Stroke::new(1.0, theme::BORDER)).inner_margin(Margin::symmetric(0, 8)).show(ui, |ui| {
        ui.set_width(200.0);
        ui.set_min_height(PANE_H);
        ui.vertical(|ui| formats_list(ui, d));
    });
}

fn formats_list(ui: &mut Ui, d: &mut Dialog) {
    ui.spacing_mut().item_spacing.y = 0.0;
    for (i, &(shown, label)) in FORMATS.iter().enumerate() {
        if i == 0 || i == 2 {
            let heading = if i == 0 { "PUBLISH" } else { "OTHER FORMATS" };
            ui.add_space(if i == 0 { 0.0 } else { 10.0 });
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(RichText::new(heading).font(theme::semibold(11.0)).color(theme::TEXT_DIM));
            });
            ui.add_space(4.0);
        }
        // A Canvas document has no SWF nor its wrapper; an ActionScript one no Canvas page.
        let shown = shown.filter(|s| match s {
            Shown::Swf | Shown::Html => !d.canvas,
            Shown::Canvas => d.canvas,
            _ => true,
        });
        let p = &mut d.settings.current_mut();
        let on = match shown {
            Some(Shown::Swf) => Some(&mut p.swf.on),
            Some(Shown::Html | Shown::Canvas) => Some(&mut p.html.on),
            Some(Shown::Gif) => Some(&mut p.gif.target.on),
            Some(Shown::Jpeg) => Some(&mut p.jpeg.target.on),
            Some(Shown::Png) => Some(&mut p.png.target.on),
            None => None,
        };
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), if on.is_some() { Sense::click() } else { Sense::hover() });
        let selected = shown.is_some() && shown == Some(d.shown);
        let checked = on.as_deref().copied().unwrap_or(false);
        let enabled = on.is_some();
        let painter = ui.painter();
        if selected {
            painter.rect_filled(rect, 0.0, theme::SELECTION);
        } else if enabled && resp.hovered() {
            painter.rect_filled(rect, 0.0, theme::BUTTON_HOVER);
        }
        let check = Rect::from_min_size(pos2(rect.left() + 12.0, rect.center().y - 7.0), vec2(14.0, 14.0));
        let fade = |c: Color32| if enabled { c } else { c.gamma_multiply(0.4) };
        if checked {
            painter.rect_filled(check, 2.0, fade(theme::CHECKBOX_ON));
            let tick = vec![check.min + vec2(3.0, 7.0), check.min + vec2(6.0, 10.0), check.min + vec2(11.0, 4.0)];
            painter.add(egui::Shape::line(tick, Stroke::new(1.6, theme::INSET)));
        } else {
            painter.rect_stroke(check, 2.0, Stroke::new(1.5, fade(theme::CHECKBOX_BORDER)), StrokeKind::Inside);
        }
        let color = if !enabled { theme::TEXT_DISABLED } else if selected { Color32::WHITE } else { theme::TEXT };
        painter.text(pos2(check.right() + 8.0, rect.center().y), Align2::LEFT_CENTER, label, FontId::proportional(12.0), color);
        resp.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, checked, label));
        if let (Some(on), Some(shown)) = (on, shown)
            && resp.clicked()
        {
            if resp.interact_pointer_pos().is_some_and(|p| p.x < check.right() + 4.0) {
                *on = !*on;
            }
            d.shown = shown;
        }
    }
}

/// The shown format's options.
fn options(ui: &mut Ui, d: &mut Dialog, stem: &str, dir: Option<&Path>, stage: (f32, f32)) {
    ui.spacing_mut().item_spacing.y = 2.0;
    let p = d.settings.current_mut();
    match d.shown {
        Shown::Swf => {
            output(ui, &mut p.swf, stem, "swf", "SWF Movie", dir);
            widgets::row(ui, |ui| {
                widgets::label(ui, "JPEG Quality:", 92.0);
                let mut q = p.jpeg_quality as f32;
                if ui.add(Number::new("JPEG Quality", &mut q, 0.0..=100.0).width(40.0)).changed() {
                    p.jpeg_quality = q.round() as u8;
                }
                ui.add_space(12.0);
                ui.add_enabled_ui(false, |ui| widgets::checkbox(ui, &mut false, "Enable JPEG deblocking"));
            });
            // ponytail: sounds publish as sound_swf.rs codes them (MP3 as imported, the rest ADPCM),
            // whatever is asked here, so these show greyed.
            ui.add_enabled_ui(false, |ui| {
                for (label, value) in [("Audio stream:", "MP3, 16 kbps, Mono"), ("Audio event:", "MP3, 16 kbps, Mono")] {
                    widgets::row(ui, |ui| {
                        widgets::label(ui, label, 92.0);
                        widgets::label(ui, value, 160.0);
                    });
                }
                widgets::row(ui, |ui| widgets::checkbox(ui, &mut false, "Override sound settings"));
            });
            ui.add_space(6.0);
            ui.label(RichText::new("Advanced").font(theme::semibold(12.0)).color(theme::SECTION_TITLE));
            widgets::row(ui, |ui| {
                widgets::checkbox(ui, &mut p.compress, "Compress movie");
                ui.add_enabled_ui(p.compress, |ui| {
                    // ponytail: LZMA needs an encoder minnamate doesn't have; Deflate it is.
                    widgets::pick(ui, "Compression", 0, &[(0, "Deflate", true), (1, "LZMA", false)], 90.0);
                });
            });
            widgets::row(ui, |ui| widgets::checkbox(ui, &mut p.include_hidden, "Include hidden layers"));
            ui.add_enabled_ui(false, |ui| {
                for label in ["Include XMP metadata", "Generate size report", "Omit trace statements", "Permit debugging", "Protect from import"] {
                    widgets::row(ui, |ui| widgets::checkbox(ui, &mut false, label));
                }
                widgets::row(ui, |ui| {
                    widgets::label(ui, "Script time limit:", 110.0);
                    widgets::label(ui, "15 seconds", 80.0);
                });
                widgets::row(ui, |ui| {
                    widgets::label(ui, "Hardware acceleration:", 140.0);
                    widgets::pick(ui, "Hardware acceleration", 0, &[(0, "None", true)], 120.0);
                });
            });
        }
        Shown::Html => {
            output(ui, &mut p.html, stem, "html", "Web page", dir);
            ui.add_space(4.0);
            ui.label(RichText::new("A page the size of the stage that plays the SWF through Ruffle, the open-source player.").color(theme::TEXT_DIM));
            ui.add_enabled_ui(false, |ui| {
                widgets::row(ui, |ui| {
                    widgets::label(ui, "Template:", 92.0);
                    widgets::pick(ui, "Template", 0, &[(0, "Ruffle", true)], 160.0);
                });
                widgets::row(ui, |ui| {
                    widgets::label(ui, "Size:", 92.0);
                    widgets::pick(ui, "HTML size", 0, &[(0, "Match Movie", true)], 160.0);
                });
            });
        }
        Shown::Canvas => {
            output(ui, &mut p.html, stem, "html", "Web page", dir);
            ui.add_space(4.0);
            let about = "The page, and its movie as a script of the same name beside it, which runtime.js (minnamate's player) plays on a canvas. Bitmaps go in images/.";
            ui.label(RichText::new(about).color(theme::TEXT_DIM));
        }
        Shown::Gif => {
            output(ui, &mut p.gif.target, stem, "gif", "GIF Image", dir);
            size(ui, &mut p.gif, stage);
            widgets::row(ui, |ui| {
                widgets::label(ui, "Playback:", 92.0);
                ui.radio_value(&mut p.gif.animated, false, "Static");
                ui.radio_value(&mut p.gif.animated, true, "Animated");
            });
            ui.add_enabled_ui(p.gif.animated, |ui| {
                widgets::row(ui, |ui| {
                    widgets::label(ui, "", 92.0);
                    if ui.radio(p.gif.repeat.is_none(), "Loop continuously").clicked() {
                        p.gif.repeat = None;
                    }
                    if ui.radio(p.gif.repeat.is_some(), "Repeat").clicked() && p.gif.repeat.is_none() {
                        p.gif.repeat = Some(1);
                    }
                    let mut n = p.gif.repeat.unwrap_or(1) as f32;
                    if ui.add_enabled(p.gif.repeat.is_some(), Number::new("Repeat times", &mut n, 0.0..=65535.0).width(40.0)).changed() {
                        p.gif.repeat = Some(n.round() as u16);
                    }
                    widgets::label(ui, "times", 40.0);
                });
            });
        }
        Shown::Jpeg => {
            output(ui, &mut p.jpeg.target, stem, "jpg", "JPEG Image", dir);
            size(ui, &mut p.jpeg, stage);
            widgets::row(ui, |ui| {
                widgets::label(ui, "Quality:", 92.0);
                let mut q = p.jpeg.quality as f32;
                if ui.add(Number::new("Quality", &mut q, 1.0..=100.0).width(40.0)).changed() {
                    p.jpeg.quality = q.round() as u8;
                }
            });
            ui.add_enabled_ui(false, |ui| widgets::row(ui, |ui| widgets::checkbox(ui, &mut false, "Progressive")));
        }
        Shown::Png => {
            output(ui, &mut p.png.target, stem, "png", "PNG Image", dir);
            size(ui, &mut p.png, stage);
            widgets::row(ui, |ui| {
                widgets::label(ui, "Bit depth:", 92.0);
                let depths = [(false, "24-bit", true), (true, "24-bit with Alpha", true)];
                if let Some(t) = widgets::pick(ui, "Bit depth", p.png.transparent, &depths, 160.0) {
                    p.png.transparent = t;
                }
            });
        }
    }
}

/// Output name: the file, relative to the document's folder, and a button to pick it.
fn output(ui: &mut Ui, target: &mut Target, stem: &str, ext: &str, kind: &str, dir: Option<&Path>) {
    widgets::row(ui, |ui| {
        widgets::label(ui, "Output name:", 92.0);
        let default = format!("{stem}.{ext}");
        let mut text = publish::output(target, stem, ext);
        if widgets::text_input(ui, &mut text, "Output name", ui.available_width() - 40.0).changed() {
            target.file = if text.trim() == default { String::new() } else { text.trim().to_string() };
        }
        let browse = theme::icon_button(ui, 24.0, false, true, "Select publish destination", |p, r, c| icons::paint(p, r.shrink(4.0), Icon::Folder, c));
        if browse.clicked() {
            let mut picker = rfd::FileDialog::new().add_filter(kind, &[ext]).set_file_name(publish::output(target, stem, ext));
            if let Some(dir) = dir {
                picker = picker.set_directory(dir);
            }
            if let Some(path) = picker.save_file() {
                let relative = dir.and_then(|d| path.strip_prefix(d).ok()).map(|p| p.to_string_lossy().replace('\\', "/"));
                target.file = relative.unwrap_or_else(|| path.to_string_lossy().into_owned());
            }
        }
    });
}

/// An image's size: its own width and height (in proportion), or the stage's.
fn size(ui: &mut Ui, image: &mut Image, (stage_w, stage_h): (f32, f32)) {
    let mut matched = image.size.is_none();
    let (mut w, mut h) = image.size.map_or((stage_w, stage_h), |(w, h)| (w as f32, h as f32));
    let (w0, h0) = (w, h);
    widgets::row(ui, |ui| {
        widgets::label(ui, "Size:", 92.0);
        ui.add_enabled_ui(!matched, |ui| {
            widgets::label(ui, "W", 14.0);
            ui.add(Number::new("Image width", &mut w, 1.0..=8192.0).suffix(" px").width(64.0));
            widgets::label(ui, "H", 14.0);
            ui.add(Number::new("Image height", &mut h, 1.0..=8192.0).suffix(" px").width(64.0));
        });
        ui.add_space(8.0);
        widgets::checkbox(ui, &mut matched, "Match movie");
    });
    // Drawn at the width's scale, so the height keeps the stage's proportions.
    if w != w0 {
        h = w * stage_h / stage_w;
    } else if h != h0 {
        w = h * stage_w / stage_h;
    }
    image.size = (!matched).then(|| (w.round().max(1.0) as u32, h.round().max(1.0) as u32));
}

/// Export Image and Export Animated GIF: the format's options over a preview, then Save.
struct Export {
    gif: bool,
    format: Format,
    transparent: bool,
    /// Size, in percent of the stage's.
    percent: f32,
    /// A GIF plays this many more times; None loops for ever.
    repeat: Option<u16>,
    preview: Option<(TextureHandle, u64)>,
}

impl Export {
    fn new(gif: bool) -> Export {
        Export { gif, format: Format::Png, transparent: false, percent: 100.0, repeat: None, preview: None }
    }
}

/// The frame on stage, numbered across the scenes as exports number them; the scene's own frame
/// while a symbol is edited.
fn shown_frame(ed: &Editor) -> u32 {
    let before: u32 = ed.doc.scenes[..ed.scene].iter().map(|s| s.frame_count()).sum();
    let frame = ed.editing.first().map_or(ed.frame, |t| t.frame);
    before + frame.min(ed.doc.scenes[ed.scene].frame_count().saturating_sub(1))
}

fn export_dialog(ctx: &egui::Context, ed: &mut Editor) {
    let frame = shown_frame(ed);
    let Some(x) = &mut ed.publish.export else { return };
    let stage = (ed.doc.width, ed.doc.height);
    // The preview: the frame, fitted into its box, redrawn as the transparency changes (a JPEG has none).
    let clear = x.transparent && (x.gif || x.format == Format::Png);
    let key = crate::app::hash((ed.rev, frame, clear));
    if x.preview.as_ref().map(|(_, k)| *k) != Some(key) {
        let fit = (400.0 / stage.0).min(300.0 / stage.1).min(4.0);
        let mut doc = std::borrow::Cow::Borrowed(&ed.doc);
        if clear {
            doc.to_mut().background[3] = 0.0;
        }
        if let Ok((pixmap, _)) = minnamate_core::render::render(&doc, frame, fit) {
            let image = ColorImage::from_rgba_premultiplied([pixmap.width() as usize, pixmap.height() as usize], pixmap.data());
            x.preview = Some((ctx.load_texture("export-preview", image, TextureOptions::LINEAR), key));
        }
    }
    let title = if x.gif { "Export Animated GIF" } else { "Export Image" };
    let popup_open = egui::Popup::is_any_open(ctx) || ed.error.is_some();
    let (mut save, mut cancel) = (false, false);
    home::dialog("export-image", ctx, theme::PANEL_BG).show(ctx, |ui| {
        cancel = home::title_bar(ui, title, 700.0);
        Frame::NONE.inner_margin(Margin { left: 16, right: 16, top: 12, bottom: 14 }).show(ui, |ui| {
            ui.set_width(668.0);
            ui.horizontal_top(|ui| {
                // The preview on a checkerboard, as transparency shows there.
                let (area, _) = ui.allocate_exact_size(vec2(420.0, 320.0), Sense::hover());
                let p = ui.painter_at(area);
                p.rect_filled(area, 0.0, theme::INSET);
                for row in 0..(area.height() / 10.0) as i32 {
                    for col in 0..(area.width() / 10.0) as i32 {
                        if (row + col) % 2 == 0 {
                            p.rect_filled(Rect::from_min_size(area.min + vec2(col as f32 * 10.0, row as f32 * 10.0), vec2(10.0, 10.0)), 0.0, Color32::from_gray(0x3A));
                        }
                    }
                }
                if let Some((tex, _)) = &x.preview {
                    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                    p.image(tex.id(), Rect::from_center_size(area.center(), tex.size_vec2()), uv, Color32::WHITE);
                }
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.set_width(236.0);
                    ui.label(RichText::new("Preset").font(theme::semibold(12.0)).color(theme::SECTION_TITLE));
                    widgets::row(ui, |ui| {
                        widgets::label(ui, "Format", 70.0);
                        if x.gif {
                            ui.add_enabled_ui(false, |ui| widgets::pick(ui, "Format", 0, &[(0, "GIF", true)], 120.0));
                        } else {
                            let jpeg = matches!(x.format, Format::Jpeg(_));
                            if let Some(j) = widgets::pick(ui, "Format", jpeg, &[(false, "PNG", true), (true, "JPEG", true)], 120.0) {
                                x.format = if j { Format::Jpeg(80) } else { Format::Png };
                            }
                        }
                    });
                    if let Format::Jpeg(q) = &mut x.format {
                        widgets::row(ui, |ui| {
                            widgets::label(ui, "Quality", 70.0);
                            let mut v = *q as f32;
                            if ui.add(Number::new("Quality", &mut v, 1.0..=100.0).width(40.0)).changed() {
                                *q = v.round() as u8;
                            }
                        });
                    } else {
                        widgets::row(ui, |ui| widgets::checkbox(ui, &mut x.transparent, "Transparency"));
                    }
                    ui.add_space(8.0);
                    ui.label(RichText::new("Image Size").font(theme::semibold(12.0)).color(theme::SECTION_TITLE));
                    let (mut w, mut h) = (stage.0 * x.percent / 100.0, stage.1 * x.percent / 100.0);
                    let (w0, h0) = (w, h);
                    widgets::row(ui, |ui| {
                        widgets::label(ui, "W", 14.0);
                        ui.add(Number::new("Width", &mut w, 1.0..=8192.0).suffix(" px").width(64.0));
                        widgets::label(ui, "H", 14.0);
                        ui.add(Number::new("Height", &mut h, 1.0..=8192.0).suffix(" px").width(64.0));
                    });
                    widgets::row(ui, |ui| {
                        widgets::label(ui, "Percent", 70.0);
                        ui.add(Number::new("Percent", &mut x.percent, 1.0..=1000.0).suffix(" %").width(64.0));
                    });
                    if w != w0 {
                        x.percent = w / stage.0 * 100.0;
                    } else if h != h0 {
                        x.percent = h / stage.1 * 100.0;
                    }
                    if x.gif {
                        ui.add_space(8.0);
                        ui.label(RichText::new("Animation").font(theme::semibold(12.0)).color(theme::SECTION_TITLE));
                        widgets::row(ui, |ui| {
                            widgets::label(ui, "Looping", 70.0);
                            let looping = match x.repeat {
                                None => 0,
                                Some(0) => 1,
                                Some(_) => 2,
                            };
                            let picks = [(0, "Forever", true), (1, "Once", true), (2, "Other...", true)];
                            match widgets::pick(ui, "Looping Options", looping, &picks, 100.0) {
                                Some(0) => x.repeat = None,
                                Some(1) => x.repeat = Some(0),
                                Some(_) => x.repeat = Some(2),
                                None => {}
                            }
                            if let Some(n) = x.repeat.filter(|&n| n > 0) {
                                let mut v = n as f32;
                                if ui.add(Number::new("Loops", &mut v, 1.0..=65535.0).width(40.0)).changed() {
                                    x.repeat = Some(v.round() as u16);
                                }
                            }
                        });
                    }
                });
            });
            ui.add_space(14.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                cancel |= ui.add(egui::Button::new("Cancel").min_size(vec2(72.0, 28.0))).clicked();
                let (_, rect) = ui.allocate_space(vec2(72.0, 28.0));
                save = home::push(ui, rect, "Save", true, 4.0).clicked();
            });
        });
    });
    if !popup_open {
        save |= ctx.input(|i| i.key_pressed(Key::Enter));
        cancel |= ctx.input(|i| i.key_pressed(Key::Escape));
    }
    if cancel {
        ed.publish.export = None;
    } else if save && let Some(x) = ed.publish.export.take() {
        let ext = match (x.gif, x.format) {
            (true, _) => "gif",
            (_, Format::Png) => "png",
            (_, Format::Jpeg(_)) => "jpg",
        };
        let picked = rfd::FileDialog::new().add_filter(ext.to_uppercase(), &[ext]).set_file_name(format!("{}.{ext}", stem(ed))).save_file();
        match picked {
            Some(path) => export_to(ed, &x, frame, &path),
            None => ed.publish.export = Some(x),
        }
    }
}

/// Writes what an Export dialog asked for to `path`.
fn export_to(ed: &mut Editor, x: &Export, frame: u32, path: &Path) {
    let scale = x.percent / 100.0;
    let made = if x.gif {
        export::gif(&ed.doc, 0..export::frame_count(&ed.doc), scale, x.transparent, x.repeat)
    } else {
        export::image(&ed.doc, frame, scale, x.transparent, x.format)
    };
    let written = made.and_then(|(bytes, skipped)| {
        std::fs::write(path, bytes)?;
        Ok(skipped)
    });
    report(ed, written.map(|skipped| (vec![path.to_path_buf()], skipped)));
}

/// Logs what an export wrote, or shows why it couldn't.
fn report(ed: &mut Editor, done: minnamate_core::Result<(Vec<PathBuf>, std::collections::BTreeSet<String>)>) {
    match done {
        Ok((files, skipped)) => {
            match &files[..] {
                [one] => ed.log(format!("Exported {}", one.display())),
                many => ed.log(format!("Exported {} files, {} to {}", many.len(), many.first().map_or(String::new(), |f| f.display().to_string()), many.last().map_or(String::new(), |f| f.display().to_string()))),
            }
            for s in skipped {
                ed.log(format!("Warning: export skipped {s}"));
            }
        }
        Err(e) => ed.error = Some(format!("Couldn't export.\n\n{e}")),
    }
}

/// File > Export > Export Movie: the SWF, or a PNG per frame when the file picked is a .png.
fn export_movie(ed: &mut Editor) {
    let stem = stem(ed);
    let picker = rfd::FileDialog::new().add_filter("SWF Movie", &["swf"]).add_filter("PNG Sequence", &["png"]).set_file_name(format!("{stem}.swf"));
    let Some(path) = picker.save_file() else { return };
    let png = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("png"));
    let done = if png {
        export::png_sequence(&ed.doc, &path, 1.0, false)
    } else {
        swf_export::export(&ed.doc).and_then(|(bytes, skipped)| {
            std::fs::write(&path, bytes)?;
            Ok((vec![path.clone()], skipped))
        })
    };
    report(ed, done);
}

/// File > Export > Export Video/Media, through ffmpeg.
// ponytail: at the stage's size, with no progress shown while it encodes.
fn export_video(ed: &mut Editor) {
    if !export::ffmpeg_found() {
        ed.error = Some(export::FFMPEG_MISSING.into());
        return;
    }
    let picker = rfd::FileDialog::new().add_filter("H.264 Video", &["mp4"]).add_filter("QuickTime", &["mov"]).add_filter("WebM", &["webm"]);
    let Some(path) = picker.set_file_name(format!("{}.mp4", stem(ed))).save_file() else { return };
    let done = export::video(&ed.doc, &path, 1.0);
    report(ed, done.map(|skipped| (vec![path], skipped)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::testkit::*;

    #[test]
    fn publish_settings_edit_a_copy_until_ok() {
        use egui_kittest::kittest::Queryable;
        let mut h = harness(false);
        h.run();
        h.key_press_modifiers(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::F12);
        h.run();
        assert!(h.state().publish.dialog.is_some(), "Ctrl+Shift+F12 opens Publish Settings");
        // A click on PNG Image's name shows its options; its box turns it on, and off again.
        let click = |h: &mut Harness<'static, Editor>, label: &str, x: f32| {
            let row = h.get_by_label(label).rect();
            click_at(h, pos2(row.left() + x, row.center().y));
            // Long enough apart that two clicks aren't a double-click.
            h.run_steps(30);
        };
        click(&mut h, "PNG Image", 80.0);
        let d = h.state().publish.dialog.as_ref().unwrap();
        assert_eq!((d.shown, d.settings.current().png.target.on), (Shown::Png, false));
        click(&mut h, "PNG Image", 19.0);
        assert!(h.state().publish.dialog.as_ref().unwrap().settings.current().png.target.on);
        assert!(!h.state().publish.settings.current().png.target.on, "not until OK");
        click(&mut h, "PNG Image", 19.0);
        click(&mut h, "GIF Image", 19.0);
        h.get_by_label("OK").click();
        h.run();
        let ed = h.state();
        let p = ed.publish.settings.current();
        assert!(ed.publish.dialog.is_none() && p.gif.target.on && !p.png.target.on && ed.dirty);
        // Saved, they go into PublishSettings.xml and come back.
        let xml = String::from_utf8(ed.publish.files(Path::new("tween.fla"))[0].1.clone()).unwrap();
        assert!(xml.contains("<gif>1</gif>") && xml.contains("<gifFileName>tween.gif</gifFileName>"), "{xml}");
    }

    #[test]
    fn saving_keeps_a_settings_file_it_cant_read() {
        let dir = std::env::temp_dir().join(format!("minnamate-unread-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        minnamate_core::xfl_write::save(&minnamate_core::doc::Document::new(550.0, 400.0, 24.0), &dir).unwrap();
        let mangled = "<flash_profiles><flash_profile name=\"Mine\">";
        std::fs::write(dir.join(publish::FILE), mangled).unwrap();
        let mut h = Harness::builder().with_size(vec2(1600.0, 900.0)).build_eframe(|cc| Editor::new(&cc.egui_ctx, Some(dir.clone())));
        h.run();
        assert!(h.state_mut().save());
        let kept = std::fs::read_to_string(dir.join(publish::FILE)).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(kept, mangled, "not replaced by a new document's settings");
    }

    #[test]
    fn enter_and_escape_end_a_profile_rename_only() {
        let mut h = harness(false);
        h.run();
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::PublishSettings, &ctx);
        h.run();
        let rename = |h: &mut Harness<'static, Editor>, name: &str, key: Key| {
            h.state_mut().publish.dialog.as_mut().unwrap().renaming = Some(name.into());
            h.run();
            h.key_press(key);
            h.run();
        };
        rename(&mut h, "Web", Key::Enter);
        let d = h.state().publish.dialog.as_ref().expect("Enter took the name, and the dialog stays");
        assert_eq!((d.renaming.as_deref(), d.settings.current().name.as_str()), (None, "Web"));
        rename(&mut h, "Print", Key::Escape);
        let d = h.state().publish.dialog.as_ref().expect("Escape dropped the name, and the dialog stays");
        assert_eq!(d.settings.current().name, "Web");
    }

    /// Writes target/screenshots/publish_settings.png: Publish Settings over the tween fixture,
    /// on the SWF's options, and publish_settings_gif.png on the GIF's.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn publish_settings_screenshot() {
        use egui_kittest::kittest::Queryable;
        let mut h = harness(true);
        h.run();
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::PublishSettings, &ctx);
        h.run();
        screenshot(&mut h, "publish_settings");
        let row = h.get_by_label("GIF Image").rect();
        click_at(&mut h, pos2(row.left() + 80.0, row.center().y));
        h.run();
        screenshot(&mut h, "publish_settings_gif");
    }

    /// Writes target/screenshots/export_image.png: Export Image's options over its preview.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn export_image_screenshot() {
        let mut h = harness(true);
        h.run();
        let ctx = h.ctx.clone();
        h.state_mut().set_frame(5);
        h.state_mut().run(Action::ExportImage, &ctx);
        h.run();
        screenshot(&mut h, "export_image");
    }
}
