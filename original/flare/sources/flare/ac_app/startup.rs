//! The Startup window, after Flare's "Flare Startup" popup
//! (`flare/sources/flare/startuppopup.cpp`): create a new scene (name, folder,
//! size preset, frame rate, background) or open an existing one (recent files,
//! Open Another…, the example animation), with "Show this at startup".
//! Our own egui implementation; Flare's project folders have no counterpart
//! here, so that part of the popup is left out.

use crate::files::{FileAction, Pending};
use crate::App;
use ac_core::{Color, Document};
use eframe::egui::{self, Color32, RichText};
use std::path::{Path, PathBuf};

/// Size presets: name, width, height, frame rate.
pub(crate) const PRESETS: &[(&str, f32, f32, f32)] = &[
    ("Flash / Animate default  550 × 400", 550.0, 400.0, 24.0),
    ("HD  1280 × 720", 1280.0, 720.0, 24.0),
    ("Full HD  1920 × 1080", 1920.0, 1080.0, 24.0),
    ("4K UHD  3840 × 2160", 3840.0, 2160.0, 24.0),
    ("Square  1080 × 1080", 1080.0, 1080.0, 24.0),
    ("Vertical video  1080 × 1920", 1080.0, 1920.0, 30.0),
    ("Web banner  728 × 90", 728.0, 90.0, 30.0),
];

pub(crate) struct Startup {
    pub name: String,
    pub folder: String,
    /// Index into PRESETS; PRESETS.len() = custom.
    pub preset: usize,
    pub width: f32,
    pub height: f32,
    pub fps: f32,
    pub background: Color32,
    pub error: Option<String>,
}

impl Startup {
    pub(crate) fn new(app: &App) -> Startup {
        let folder = app
            .settings
            .last_dir
            .clone()
            .or_else(|| dirs::document_dir().map(|d| d.display().to_string()))
            .unwrap_or_default();
        let (_, w, h, fps) = PRESETS[0];
        Startup { name: next_name(&folder), folder, preset: 0, width: w, height: h, fps, background: Color32::WHITE, error: None }
    }
}

/// "Scene 1", "Scene 2", … — the first name with no file in `folder`.
fn next_name(folder: &str) -> String {
    (1..1000).map(|i| format!("Scene {i}")).find(|n| !Path::new(folder).join(format!("{n}.acx")).exists()).unwrap_or_else(|| "Scene".into())
}

/// Show the Startup window at launch? (not when a file was given, in screenshots
/// or tests, or when the user turned it off)
pub(crate) fn at_launch(app: &App, opened_file: bool) -> bool {
    !opened_file && app.settings.show_startup && std::env::var_os("AC_SCREENSHOT").is_none() && std::env::var_os("AC_NO_STARTUP").is_none_or(|v| v.is_empty())
}

pub(crate) fn icon_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let id = egui::Id::new("flare.startup.icon");
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(t);
    }
    let img = image::load_from_memory(include_bytes!("../assets/icon/flare.png")).ok()?.to_rgba8();
    let ci = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
    let t = ctx.load_texture("flare-startup-icon", ci, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, t.clone()));
    Some(t)
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.label(RichText::new(title).strong().size(15.0).color(Color32::from_gray(225)));
    ui.add_space(4.0);
}

pub(crate) fn window(app: &mut App, ctx: &egui::Context) {
    let Some(mut st) = app.startup.take() else { return };
    let mut open = true;
    let mut close = false;
    let mut action: Option<Pending> = None;
    let mut create = false;
    let screen = ctx.content_rect();
    egui::Window::new("Flare Startup")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .fixed_size([760.0_f32.min(screen.width() - 40.0), 0.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(t) = icon_texture(ctx) {
                    ui.add(egui::Image::new(&t).fit_to_exact_size(egui::vec2(40.0, 40.0)));
                }
                ui.vertical(|ui| {
                    ui.label(RichText::new("Flare").size(22.0).strong().color(Color32::from_gray(235)));
                    ui.label(RichText::new(format!("{} · Rust", env!("CARGO_PKG_VERSION"))).small().color(Color32::from_gray(150)));
                });
            });
            ui.separator();
            ui.columns(2, |cols| {
                // ---------------------------------------------------------- new
                let ui = &mut cols[0];
                section(ui, "Create a New Scene");
                egui::Grid::new("startup.new").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                    ui.label("Scene Name:");
                    ui.add(egui::TextEdit::singleline(&mut st.name).desired_width(220.0));
                    ui.end_row();
                    ui.label("Save In:");
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut st.folder).desired_width(170.0).hint_text("(choose when saving)"));
                        if crate::files::native_dialogs() && ui.small_button("…").on_hover_text("Choose a folder").clicked() {
                            if let Some(d) = rfd::FileDialog::new().set_directory(&st.folder).pick_folder() {
                                st.folder = d.display().to_string();
                            }
                        }
                    });
                    ui.end_row();
                    ui.label("Preset:");
                    let label = PRESETS.get(st.preset).map(|p| p.0).unwrap_or("<custom>");
                    egui::ComboBox::from_id_salt("startup.preset").selected_text(label).width(220.0).show_ui(ui, |ui| {
                        for (i, p) in PRESETS.iter().enumerate() {
                            if ui.selectable_label(st.preset == i, p.0).clicked() {
                                st.preset = i;
                                st.width = p.1;
                                st.height = p.2;
                                st.fps = p.3;
                            }
                        }
                        let _ = ui.selectable_label(st.preset >= PRESETS.len(), "<custom>");
                    });
                    ui.end_row();
                    ui.label("Width:");
                    let w = ui.add(egui::DragValue::new(&mut st.width).range(1.0..=8192.0).speed(1.0).suffix(" px"));
                    ui.end_row();
                    ui.label("Height:");
                    let h = ui.add(egui::DragValue::new(&mut st.height).range(1.0..=8192.0).speed(1.0).suffix(" px"));
                    ui.end_row();
                    if w.changed() || h.changed() {
                        st.preset = PRESETS.iter().position(|p| p.1 == st.width && p.2 == st.height).unwrap_or(PRESETS.len());
                    }
                    ui.label("Frame Rate:");
                    ui.add(egui::DragValue::new(&mut st.fps).range(1.0..=120.0).speed(0.1).suffix(" fps"));
                    ui.end_row();
                    ui.label("Background:");
                    ui.color_edit_button_srgba(&mut st.background);
                    ui.end_row();
                });
                // aspect preview
                let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().min(240.0), 70.0), egui::Sense::hover());
                let k = (rect.width() / st.width.max(1.0)).min(rect.height() / st.height.max(1.0));
                let r = egui::Rect::from_center_size(rect.center(), egui::vec2(st.width * k, st.height * k).max(egui::vec2(2.0, 2.0)));
                ui.painter().rect(r, 1.0, st.background, egui::Stroke::new(1.0_f32, Color32::from_gray(110)), egui::StrokeKind::Inside);
                if let Some(e) = &st.error {
                    ui.label(RichText::new(e).color(Color32::from_rgb(0xFF, 0x80, 0x60)));
                }
                ui.add_space(4.0);
                if ui.add(egui::Button::new(RichText::new("Create Scene").strong()).min_size(egui::vec2(140.0, 26.0))).clicked() {
                    create = true;
                }

                // --------------------------------------------------------- open
                let ui = &mut cols[1];
                section(ui, "Open Existing Scene");
                ui.label(RichText::new("Recent Scenes").color(Color32::from_gray(170)));
                egui::ScrollArea::vertical().max_height(220.0).id_salt("startup.recent").show(ui, |ui| {
                    if app.settings.recent.is_empty() {
                        ui.label(RichText::new("No Recent Scenes").italics().color(Color32::from_gray(130)));
                    }
                    for p in app.settings.recent.clone() {
                        let path = Path::new(&p);
                        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.clone());
                        let dir = path.parent().map(|d| d.display().to_string()).unwrap_or_default();
                        let exists = path.exists();
                        let text = if exists { RichText::new(&name) } else { RichText::new(format!("{name}  (missing)")).color(Color32::from_gray(120)) };
                        let r = ui.add_enabled(exists, egui::Button::new(text).frame(false)).on_hover_text(&p);
                        ui.label(RichText::new(dir).small().color(Color32::from_gray(120)));
                        if r.clicked() {
                            action = Some(Pending::OpenPath(p.clone()));
                        }
                    }
                });
                ui.add_space(6.0);
                if ui.button("Open Another Scene…").clicked() {
                    action = Some(Pending::Open);
                }
                if ui.button("Open Example Animation").clicked() {
                    action = Some(Pending::OpenDemo);
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.checkbox(&mut app.settings.show_startup, "Show this at startup").changed() {
                    app.settings.save();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });
            });
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = true;
    }
    if create {
        match create_scene(app, &st) {
            Ok(()) => return,
            Err(e) => st.error = Some(e),
        }
    }
    if let Some(p) = action {
        // the startup window closes; anything unsaved is asked about first
        app.guarded(p);
        return;
    }
    if open && !close {
        app.startup = Some(st);
    }
}

/// Make the new document; with a folder, save it there right away (as Flare does).
fn create_scene(app: &mut App, st: &Startup) -> Result<(), String> {
    let name = st.name.trim();
    if name.is_empty() {
        return Err("The name cannot be empty.".into());
    }
    if name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|']) {
        return Err("The name can't contain / \\ : * ? \" < > |".into());
    }
    if st.width <= 0.0 || st.height <= 0.0 {
        return Err("The width and height must be greater than zero.".into());
    }
    if st.fps < 1.0 {
        return Err("The frame rate must be 1 or more.".into());
    }
    let c = st.background;
    let mut doc = Document { width: st.width.round(), height: st.height.round(), fps: st.fps, background: Color::rgb(c.r(), c.g(), c.b()), ..Document::default() };
    doc.scene.name = "Scene 1".into();
    let path: Option<PathBuf> = (!st.folder.trim().is_empty()).then(|| Path::new(st.folder.trim()).join(format!("{name}.acx")));
    if let Some(p) = &path {
        if p.exists() {
            return Err(format!("{} already exists: choose another name or folder.", p.display()));
        }
        std::fs::create_dir_all(p.parent().unwrap_or(Path::new("."))).map_err(|e| format!("Failed to create the folder: {e}"))?;
    }
    app.load_doc(doc, None);
    app.untitled_name = Some(name.to_string());
    app.startup = None;
    if let Some(p) = path {
        app.do_file(FileAction::Save, &p.display().to_string());
    } else {
        app.status = format!("New scene “{name}” ({} × {}, {} fps). Save with Ctrl+S.", st.width.round(), st.height.round(), st.fps);
    }
    Ok(())
}
