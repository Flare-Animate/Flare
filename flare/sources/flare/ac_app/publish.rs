//! Publish Settings (Ctrl+Shift+F12) and background export jobs.

use crate::files::FileAction;
use crate::App;
use ac_render::ExportOpts;
use eframe::egui::{self, Color32, RichText};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc};

pub struct PublishSettings {
    pub open: bool,
    pub scale: f32,
    pub transparent: bool,
    pub whole: bool,
    pub from: u32,
    pub to: u32,
    pub gif_loops: u16,
    pub gif_quality: i32,
    pub seq_prefix: String,
    pub ffmpeg: String,
    pub html_loop: bool,
    pub sheet: ac_render::formats::SheetOpts,
    pub job: Option<Job>,
    ffmpeg_checked: Option<Option<String>>,
    /// File › Export › Export Image… window.
    pub image_open: bool,
    /// 0 = PNG, 1 = JPEG, 2 = GIF.
    pub image_format: u8,
    pub jpeg_quality: u8,
    pub gif_colors: u16,
}

impl Default for PublishSettings {
    fn default() -> Self {
        PublishSettings {
            open: false,
            scale: 1.0,
            transparent: false,
            whole: true,
            from: 1,
            to: 1,
            gif_loops: 0,
            gif_quality: 70,
            seq_prefix: "frame".into(),
            ffmpeg: "ffmpeg".into(),
            html_loop: true,
            sheet: Default::default(),
            job: None,
            ffmpeg_checked: None,
            image_open: false,
            image_format: 0,
            jpeg_quality: 80,
            gif_colors: 256,
        }
    }
}

pub struct Job {
    pub label: String,
    pub done: Arc<AtomicU32>,
    pub total: Arc<AtomicU32>,
    pub cancel: Arc<AtomicBool>,
    rx: mpsc::Receiver<Result<String, String>>,
}

impl PublishSettings {
    /// The still-image format chosen in Export Image (or implied by `ext`).
    pub fn still_format(&self, ext: Option<&str>) -> ac_render::StillFormat {
        let f = match ext.map(|e| e.to_ascii_lowercase()) {
            Some(e) if e == "jpg" || e == "jpeg" => 1,
            Some(e) if e == "gif" => 2,
            Some(e) if e == "png" => 0,
            _ => self.image_format,
        };
        match f {
            1 => ac_render::StillFormat::Jpeg { quality: self.jpeg_quality },
            2 => ac_render::StillFormat::Gif { colors: self.gif_colors, transparent: self.transparent },
            _ => ac_render::StillFormat::Png { transparent: self.transparent },
        }
    }
    pub fn opts(&self) -> ExportOpts {
        ExportOpts {
            scale: self.scale.clamp(0.05, 8.0),
            range: if self.whole { None } else { Some((self.from.max(1) - 1, self.to.max(self.from).max(1) - 1)) },
            transparent: self.transparent,
            loops: self.gif_loops,
            // quality 100 → speed 1 (best), 0 → 30 (fastest)
            gif_speed: (30.0 - self.gif_quality as f32 / 100.0 * 29.0).round() as i32,
        }
    }
}

/// Exports that take a while run on a worker thread with progress + cancel.
fn spawn(app: &mut App, label: String, work: impl FnOnce(&mut dyn FnMut(u32, u32) -> bool) -> Result<String, String> + Send + 'static) {
    if app.publish.job.is_some() {
        app.status = "An export is already running.".into();
        return;
    }
    let (tx, rx) = mpsc::channel();
    let done = Arc::new(AtomicU32::new(0));
    let total = Arc::new(AtomicU32::new(1));
    let cancel = Arc::new(AtomicBool::new(false));
    let (d, t, c) = (done.clone(), total.clone(), cancel.clone());
    let ctx = app.ctx.clone();
    std::thread::spawn(move || {
        let mut progress = |a: u32, b: u32| {
            d.store(a, Ordering::Relaxed);
            t.store(b, Ordering::Relaxed);
            ctx.request_repaint();
            !c.load(Ordering::Relaxed)
        };
        let r = work(&mut progress);
        let _ = tx.send(r);
        ctx.request_repaint();
    });
    app.status = format!("{label}…");
    app.publish.job = Some(Job { label, done, total, cancel, rx });
}

/// File actions handled here: multi-frame exports and imports.
pub(crate) fn run_file_action(app: &mut App, action: FileAction, path: &str) -> Result<String, String> {
    let doc = app.ed.saved_doc();
    let o = app.publish.opts();
    let p = std::path::PathBuf::from(path);
    match action {
        FileAction::ExportGif => {
            spawn(app, "Exporting GIF".into(), move |prog| ac_render::export_gif_opts(&doc, &o, &p, prog).map(|_| format!("Exported animated GIF to {}", p.display())));
            Ok("Exporting GIF…".into())
        }
        FileAction::ExportPngSequence => {
            let prefix = app.publish.seq_prefix.clone();
            spawn(app, "Exporting PNG sequence".into(), move |prog| ac_render::export_png_sequence(&doc, &o, &p, &prefix, prog).map(|n| format!("Exported {n} PNG frames to {}", p.display())));
            Ok("Exporting PNG sequence…".into())
        }
        FileAction::ExportVideo => {
            let ff = app.publish.ffmpeg.clone();
            if ac_render::ffmpeg_version(&ff).is_none() {
                return Err(format!("video export needs ffmpeg, and “{ff}” wasn't found. Install ffmpeg (https://ffmpeg.org; on Windows: winget install ffmpeg) or set its path in File › Publish Settings"));
            }
            spawn(app, "Exporting video".into(), move |prog| ac_render::export_video(&doc, &o, &p, &ff, prog).map(|_| format!("Exported video to {}", p.display())));
            Ok("Exporting video…".into())
        }
        FileAction::ExportHtml5 => {
            let ho = ac_render::html5::Html5Opts { loop_play: app.publish.html_loop, range: o.range, single_file: false };
            spawn(app, "Publishing HTML5 Canvas".into(), move |_| ac_render::html5::publish_to(&doc, &ho, &p).map(|page| format!("Published HTML5 Canvas: {}", page.display())));
            Ok("Publishing HTML5 Canvas…".into())
        }
        FileAction::ExportSpriteSheet => {
            let so = app.publish.sheet.clone();
            spawn(app, "Generating sprite sheet".into(), move |_| ac_render::formats::export_sprite_sheet(&doc, &o, &so, &p).map(|(a, b)| format!("Sprite sheet: {} + {}", a.display(), b.display())));
            Ok("Generating sprite sheet…".into())
        }
        FileAction::ExportAnimatedSvg => std::fs::write(&p, ac_render::export_animated_svg(&doc, &o)).map_err(|e| e.to_string()).map(|_| format!("Exported animated SVG to {}", p.display())),
        FileAction::ExportLottie => {
            let (json, warn) = ac_render::formats::lottie(&doc, &o);
            std::fs::write(&p, json).map_err(|e| e.to_string())?;
            for w in &warn {
                app.scripting.push(crate::scripting::OutKind::Info, format!("Lottie: {w}"));
            }
            Ok(format!("Exported Lottie to {}{}", p.display(), if warn.is_empty() { String::new() } else { format!(" ({} notes, see Output)", warn.len()) }))
        }
        FileAction::ExportXdts => std::fs::write(&p, ac_core::xdts::export(&doc, app.ed.scene_tl())).map_err(|e| e.to_string()).map(|_| format!("Exported timing sheet to {}", p.display())),
        FileAction::ImportImage => crate::import::import_image(app, path),
        FileAction::ImportSvg => crate::import::import_svg(app, path),
        FileAction::ImportSequence => crate::import::import_sequence(app, path),
        FileAction::ImportPsd => crate::import::import_psd(app, path),
        FileAction::ImportSwatches => crate::swatches::import(app, path),
        FileAction::ExportSwatches => crate::swatches::export(app, path),
        FileAction::ImportXdts => ac_core::xdts::import_file(&mut app.ed, std::path::Path::new(path)).map(|r| r.summary()),
        _ => Err("unsupported".into()),
    }
}

/// Poll the running job; show Publish Settings when open.
/// File › Export › Export Image…: format, size and quality, then a file.
pub fn image_window(app: &mut App, ctx: &egui::Context) {
    if !app.publish.image_open {
        return;
    }
    let mut open = true;
    let mut go = false;
    let (w, h) = (app.ed.doc.width, app.ed.doc.height);
    egui::Window::new("Export Image").open(&mut open).resizable(false).show(ctx, |ui| {
        let ps = &mut app.publish;
        egui::Grid::new("img-grid").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            ui.label("Format");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut ps.image_format, 0, "PNG");
                ui.selectable_value(&mut ps.image_format, 1, "JPEG");
                ui.selectable_value(&mut ps.image_format, 2, "GIF");
            });
            ui.end_row();
            ui.label("Size");
            ui.horizontal(|ui| {
                let mut pw = (w * ps.scale).round();
                if ui.add(egui::DragValue::new(&mut pw).range(1.0..=16000.0).suffix(" px")).changed() {
                    ps.scale = (pw / w.max(1.0)).clamp(0.05, 8.0);
                }
                ui.label(format!("× {:.0} px", (h * ps.scale).round()));
                ui.add(egui::DragValue::new(&mut ps.scale).range(0.05..=8.0).speed(0.01).prefix("scale ").max_decimals(2));
            });
            ui.end_row();
            match ps.image_format {
                1 => {
                    ui.label("Quality");
                    ui.add(egui::Slider::new(&mut ps.jpeg_quality, 1..=100));
                    ui.end_row();
                }
                2 => {
                    ui.label("Colours");
                    ui.add(egui::Slider::new(&mut ps.gif_colors, 2..=256).logarithmic(true));
                    ui.end_row();
                    ui.label("");
                    ui.checkbox(&mut ps.transparent, "Transparent background");
                    ui.end_row();
                }
                _ => {
                    ui.label("");
                    ui.checkbox(&mut ps.transparent, "Transparent background");
                    ui.end_row();
                }
            }
        });
        ui.label(RichText::new("Exports the current frame. JPEG has no transparency (the stage colour fills it).").small().color(Color32::from_gray(150)));
        if ui.button("Export…").clicked() {
            go = true;
        }
    });
    if go {
        app.publish.image_open = false;
        app.file(FileAction::ExportPng);
    } else if !open {
        app.publish.image_open = false;
    }
}

pub fn window(app: &mut App, ctx: &egui::Context) {
    if let Some(job) = &app.publish.job {
        if let Ok(r) = job.rx.try_recv() {
            app.status = match r {
                Ok(s) => s,
                Err(e) => format!("{} failed: {e}", job.label),
            };
            app.publish.job = None;
        }
    }
    if let Some(job) = &app.publish.job {
        let (d, t) = (job.done.load(Ordering::Relaxed), job.total.load(Ordering::Relaxed).max(1));
        let label = job.label.clone();
        let cancel = job.cancel.clone();
        egui::Window::new("Exporting").collapsible(false).resizable(false).anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -40.0]).show(ctx, |ui| {
            ui.label(format!("{label}: frame {d} of {t}"));
            ui.add(egui::ProgressBar::new(d as f32 / t as f32).desired_width(240.0));
            if ui.button("Cancel").clicked() {
                cancel.store(true, Ordering::Relaxed);
            }
        });
    }
    if !app.publish.open {
        return;
    }
    let mut open = true;
    let n = app.ed.doc.total_frames();
    let (w, h) = (app.ed.doc.width, app.ed.doc.height);
    egui::Window::new("Publish Settings").open(&mut open).resizable(false).show(ctx, |ui| {
        let ps = &mut app.publish;
        egui::Grid::new("pub").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            ui.label("Scale");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut ps.scale).range(0.05..=8.0).speed(0.01).suffix("×"));
                ui.label(RichText::new(format!("{} × {} px", (w * ps.scale).round(), (h * ps.scale).round())).color(Color32::from_gray(160)));
            });
            ui.end_row();
            ui.label("Frames");
            ui.horizontal(|ui| {
                ui.radio_value(&mut ps.whole, true, format!("All ({n})"));
                ui.radio_value(&mut ps.whole, false, "From");
                ui.add_enabled(!ps.whole, egui::DragValue::new(&mut ps.from).range(1..=n));
                ui.label("to");
                ui.add_enabled(!ps.whole, egui::DragValue::new(&mut ps.to).range(1..=n));
            });
            ui.end_row();
            ui.label("Background");
            ui.checkbox(&mut ps.transparent, "Transparent (PNG, GIF, WebM, MOV)");
            ui.end_row();
            ui.label("GIF");
            ui.horizontal(|ui| {
                ui.label("Loop");
                ui.add(egui::DragValue::new(&mut ps.gif_loops).range(0..=100).custom_formatter(|v, _| if v == 0.0 { "forever".into() } else { format!("{v}×") }));
                ui.label("Quality");
                ui.add(egui::Slider::new(&mut ps.gif_quality, 0..=100));
            });
            ui.end_row();
            ui.label("PNG sequence");
            ui.horizontal(|ui| {
                ui.label("File prefix");
                ui.add(egui::TextEdit::singleline(&mut ps.seq_prefix).desired_width(90.0));
                ui.label(RichText::new(format!("e.g. {}0001.png", ps.seq_prefix)).color(Color32::from_gray(150)));
            });
            ui.end_row();
            ui.label("HTML5 Canvas");
            ui.horizontal(|ui| {
                ui.checkbox(&mut ps.html_loop, "Loop timeline");
                ui.label(RichText::new("folder = index.html + movie.js + images · name.html = one file").small().color(Color32::from_gray(150)));
            });
            ui.end_row();
            ui.label("Sprite sheet");
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("sheet-fmt").selected_text(ps.sheet.format.name()).show_ui(ui, |ui| {
                    for f in ac_render::formats::AtlasFormat::ALL {
                        ui.selectable_value(&mut ps.sheet.format, f, f.name());
                    }
                });
                ui.checkbox(&mut ps.sheet.trim, "Trim");
                ui.checkbox(&mut ps.sheet.dedupe, "Detect duplicates");
                ui.label("Padding");
                ui.add(egui::DragValue::new(&mut ps.sheet.padding).range(0..=32));
            });
            ui.end_row();
            ui.label("Video");
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label("ffmpeg");
                    if ui.add(egui::TextEdit::singleline(&mut ps.ffmpeg).desired_width(180.0)).changed() {
                        ps.ffmpeg_checked = None;
                    }
                });
                let found = ps.ffmpeg_checked.get_or_insert_with(|| ac_render::ffmpeg_version(&ps.ffmpeg)).clone();
                match found {
                    Some(v) => ui.label(RichText::new(format!("✔ {}", v.split(" Copyright").next().unwrap_or(&v))).small().color(Color32::from_rgb(0x70, 0xD0, 0x80))),
                    None => ui.label(RichText::new("ffmpeg not found: install it (Windows: winget install ffmpeg) or give its full path").small().color(Color32::from_rgb(0xFF, 0xB0, 0x40))),
                };
                ui.label(RichText::new(".mp4 = H.264 · .webm = VP9 (alpha) · .mov = ProRes 4444 (alpha)").small().color(Color32::from_gray(150)));
            });
            ui.end_row();
        });
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.label("Export:");
            for (a, l) in [(FileAction::ExportGif, "GIF…"), (FileAction::ExportPngSequence, "PNG sequence…"), (FileAction::ExportVideo, "Video…"), (FileAction::ExportPng, "Image (PNG/JPEG/GIF)…"), (FileAction::ExportSvg, "SVG frame…"), (FileAction::ExportHtml5, "HTML5 Canvas…"), (FileAction::ExportSpriteSheet, "Sprite sheet…"), (FileAction::ExportAnimatedSvg, "Animated SVG…"), (FileAction::ExportLottie, "Lottie…"), (FileAction::ExportXfl, ".fla / XFL…")] {
                if ui.button(l).clicked() {
                    app.file(a);
                }
            }
        });
    });
    if !open {
        app.publish.open = false;
    }
}

/// Control › Test Movie (Ctrl+Enter): publish the movie as one HTML page in a
/// temporary folder and open it in the default browser, where it plays with
/// its scripts, buttons and sounds (our HTML5 runtime). Animate opens its own
/// player window; the published page is the closest thing we have.
pub fn test_movie(app: &mut App) {
    let doc = app.ed.saved_doc();
    let dir = std::env::temp_dir().join("flare-test-movie");
    let name: String = app.doc_name().chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    let page = dir.join(format!("{}.html", if name.is_empty() { "movie".into() } else { name }));
    let ho = ac_render::html5::Html5Opts { loop_play: app.loop_play, range: None, single_file: true };
    let launch = crate::files::native_dialogs();
    spawn(app, "Test Movie".into(), move |_| {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let out = ac_render::html5::publish_to(&doc, &ho, &page)?;
        if launch {
            open_in_browser(&out)?;
            Ok(format!("Test Movie: playing in your browser ({})", out.display()))
        } else {
            Ok(format!("Test Movie: published {}", out.display()))
        }
    });
}

fn open_in_browser(p: &std::path::Path) -> Result<(), String> {
    let r = if cfg!(target_os = "windows") {
        std::process::Command::new("cmd").args(["/C", "start", ""]).arg(p).spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(p).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(p).spawn()
    };
    r.map(|_| ()).map_err(|e| format!("couldn't open a browser: {e}"))
}
