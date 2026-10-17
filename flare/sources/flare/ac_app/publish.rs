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
    /// The result, and the publish report's notes (what the format couldn't hold exactly).
    rx: mpsc::Receiver<(Result<String, String>, Vec<String>)>,
}

/// Every publish target the core can write, with the id used by scripts and
/// the command line. `ALL` is the list shown in Publish Settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublishTarget {
    Html5,
    Svg,
    SvgAnimated,
    Gif,
    Png,
    PngSequence,
    SpriteSheet,
    Lottie,
    Video,
    Xfl,
    Acx,
    Xdts,
}

impl PublishTarget {
    pub const ALL: [PublishTarget; 12] = [
        PublishTarget::Html5,
        PublishTarget::Svg,
        PublishTarget::SvgAnimated,
        PublishTarget::Gif,
        PublishTarget::Png,
        PublishTarget::PngSequence,
        PublishTarget::SpriteSheet,
        PublishTarget::Lottie,
        PublishTarget::Video,
        PublishTarget::Xfl,
        PublishTarget::Acx,
        PublishTarget::Xdts,
    ];
    pub fn id(self) -> &'static str {
        match self {
            PublishTarget::Html5 => "html5",
            PublishTarget::Svg => "svg",
            PublishTarget::SvgAnimated => "svg-animated",
            PublishTarget::Gif => "gif",
            PublishTarget::Png => "png",
            PublishTarget::PngSequence => "png-sequence",
            PublishTarget::SpriteSheet => "sprite-sheet",
            PublishTarget::Lottie => "lottie",
            PublishTarget::Video => "video",
            PublishTarget::Xfl => "xfl",
            PublishTarget::Acx => "acx",
            PublishTarget::Xdts => "xdts",
        }
    }
    /// Button label in Publish Settings.
    pub fn label(self) -> &'static str {
        match self {
            PublishTarget::Html5 => "HTML5 Canvas…",
            PublishTarget::Svg => "SVG frame…",
            PublishTarget::SvgAnimated => "Animated SVG…",
            PublishTarget::Gif => "GIF…",
            PublishTarget::Png => "Image (PNG/JPEG/GIF)…",
            PublishTarget::PngSequence => "PNG sequence…",
            PublishTarget::SpriteSheet => "Sprite sheet…",
            PublishTarget::Lottie => "Lottie…",
            PublishTarget::Video => "Video…",
            PublishTarget::Xfl => ".fla / XFL…",
            PublishTarget::Acx => "Copy as .acx…",
            PublishTarget::Xdts => "Timing sheet (XDTS)…",
        }
    }
    /// The target for an id (case-insensitive, a few common aliases). An unknown
    /// id says what the valid ones are.
    pub fn parse(id: &str) -> Result<PublishTarget, String> {
        let k = id.trim().to_ascii_lowercase().replace(['_', ' '], "-");
        let alias = match k.as_str() {
            "html" | "canvas" | "html5-canvas" => "html5",
            "animated-svg" | "svga" => "svg-animated",
            "image" | "jpeg" | "jpg" => "png",
            "png-seq" | "frames" | "sequence" => "png-sequence",
            "sprites" | "spritesheet" | "atlas" => "sprite-sheet",
            "ffmpeg" | "mp4" | "webm" | "mov" => "video",
            "fla" => "xfl",
            "document" | "package" => "acx",
            other => other,
        };
        PublishTarget::ALL.into_iter().find(|t| t.id() == alias).ok_or_else(|| format!("unknown publish format \"{id}\"; valid ids: {}", PublishTarget::ALL.map(PublishTarget::id).join(", ")))
    }
    /// The file action that runs this target.
    pub(crate) fn action(self) -> FileAction {
        match self {
            PublishTarget::Html5 => FileAction::ExportHtml5,
            PublishTarget::Svg => FileAction::ExportSvg,
            PublishTarget::SvgAnimated => FileAction::ExportAnimatedSvg,
            PublishTarget::Gif => FileAction::ExportGif,
            PublishTarget::Png => FileAction::ExportPng,
            PublishTarget::PngSequence => FileAction::ExportPngSequence,
            PublishTarget::SpriteSheet => FileAction::ExportSpriteSheet,
            PublishTarget::Lottie => FileAction::ExportLottie,
            PublishTarget::Video => FileAction::ExportVideo,
            PublishTarget::Xfl => FileAction::ExportXfl,
            PublishTarget::Acx => FileAction::ExportAcx,
            PublishTarget::Xdts => FileAction::ExportXdts,
        }
    }
    /// The publish target a file action belongs to (None for open / import / save).
    /// Exhaustive on purpose: a new action must be placed here.
    pub(crate) fn of_action(a: FileAction) -> Option<PublishTarget> {
        Some(match a {
            FileAction::ExportHtml5 => PublishTarget::Html5,
            FileAction::ExportSvg => PublishTarget::Svg,
            FileAction::ExportAnimatedSvg => PublishTarget::SvgAnimated,
            FileAction::ExportGif => PublishTarget::Gif,
            FileAction::ExportPng => PublishTarget::Png,
            FileAction::ExportPngSequence => PublishTarget::PngSequence,
            FileAction::ExportSpriteSheet => PublishTarget::SpriteSheet,
            FileAction::ExportLottie => PublishTarget::Lottie,
            FileAction::ExportVideo => PublishTarget::Video,
            FileAction::ExportXfl => PublishTarget::Xfl,
            FileAction::ExportAcx => PublishTarget::Acx,
            FileAction::ExportXdts => PublishTarget::Xdts,
            FileAction::Open
            | FileAction::Save
            | FileAction::ImportImage
            | FileAction::ImportSvg
            | FileAction::ImportSequence
            | FileAction::ImportPsd
            | FileAction::ImportXdts
            | FileAction::ImportSound
            | FileAction::ImportSwatches
            | FileAction::ExportSwatches => return None,
        })
    }
}

/// Write `doc` as an .acx package, or plain JSON for a `.json` name, without
/// touching the open document's file name (File › Save a copy).
pub fn save_copy(doc: &ac_core::Document, path: &std::path::Path) -> Result<String, String> {
    let json = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"));
    if json {
        std::fs::write(path, doc.to_json()).map_err(|e| format!("{}: {e}", path.display()))?;
    } else {
        ac_package::save(doc, path, None)?;
    }
    Ok(format!("Saved a copy to {}", path.display()))
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
    spawn_notes(app, label, move |prog, _| work(prog));
}

/// `spawn`, where the work can also return notes for the Output panel.
fn spawn_notes(app: &mut App, label: String, work: impl FnOnce(&mut dyn FnMut(u32, u32) -> bool, &mut Vec<String>) -> Result<String, String> + Send + 'static) {
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
        let mut notes = vec![];
        let r = work(&mut progress, &mut notes);
        let _ = tx.send((r, notes));
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
            spawn_notes(app, "Publishing HTML5 Canvas".into(), move |_, notes| {
                let (page, warnings) = ac_render::html5::publish_to_report(&doc, &ho, &p)?;
                let n = warnings.len();
                notes.extend(warnings);
                Ok(format!("Published HTML5 Canvas: {}{}", page.display(), if n == 0 { String::new() } else { format!(" ({n} notes about what the page can't show exactly, see Output)") }))
            });
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
        FileAction::ExportAcx => save_copy(&doc, &p),
        FileAction::ExportXdts => std::fs::write(&p, ac_core::xdts::export(&doc, app.ed.scene_tl())).map_err(|e| e.to_string()).map(|_| format!("Exported timing sheet to {}", p.display())),
        FileAction::ImportImage => crate::import::import_image(app, path),
        FileAction::ImportSvg => crate::import::import_svg(app, path),
        FileAction::ImportSequence => crate::import::import_sequence(app, path),
        FileAction::ImportPsd => crate::import::import_psd(app, path),
        FileAction::ImportSwatches => crate::swatches::import(app, path),
        FileAction::ExportSwatches => crate::swatches::export(app, path),
        FileAction::ImportXdts => ac_core::xdts::import_file(&mut app.ed, std::path::Path::new(path)).map(|r| r.summary()),
        // handled by File ▸ Open / Save / Export in `App::do_file` before it gets here
        FileAction::Open | FileAction::Save | FileAction::ExportPng | FileAction::ExportSvg | FileAction::ExportXfl | FileAction::ImportSound => {
            Err(format!("internal error: “{}” is not a Publish job (it is handled by File › Open / Save / Export directly)", action.title()))
        }
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
        if let Ok((r, notes)) = job.rx.try_recv() {
            app.status = match r {
                Ok(s) => s,
                Err(e) => format!("{} failed: {e}", job.label),
            };
            let label = job.label.clone();
            app.publish.job = None;
            if !notes.is_empty() {
                app.scripting.push(crate::scripting::OutKind::Info, format!("{label}: {} notes", notes.len()));
                for w in notes {
                    app.scripting.push(crate::scripting::OutKind::Info, format!("  • {w}"));
                }
            }
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
            for t in PublishTarget::ALL {
                if ui.button(t.label()).on_hover_text(format!("id: {}", t.id())).clicked() {
                    app.file(t.action());
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
    spawn_notes(app, "Test Movie".into(), move |_, notes| {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let (out, warnings) = ac_render::html5::publish_to_report(&doc, &ho, &page)?;
        notes.extend(warnings);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_target_has_a_unique_id_that_parses_back() {
        let mut seen = std::collections::HashSet::new();
        for t in PublishTarget::ALL {
            assert!(seen.insert(t.id()), "duplicate id {}", t.id());
            assert_eq!(PublishTarget::parse(t.id()), Ok(t));
            assert_eq!(PublishTarget::parse(&t.id().to_uppercase()), Ok(t), "case-insensitive");
            // each target runs a real file action, and the action maps back to the target
            assert_eq!(PublishTarget::of_action(t.action()), Some(t), "{}", t.id());
        }
        // the formats the core supports
        for id in ["html5", "svg", "gif", "png-sequence", "sprite-sheet", "lottie", "video", "xfl", "acx"] {
            assert!(PublishTarget::parse(id).is_ok(), "{id}");
        }
        assert_eq!(PublishTarget::parse("fla"), Ok(PublishTarget::Xfl));
        assert_eq!(PublishTarget::parse("sprite_sheet"), Ok(PublishTarget::SpriteSheet));
    }

    #[test]
    fn unknown_format_error_lists_the_valid_ids() {
        let e = PublishTarget::parse("swf").unwrap_err();
        assert!(e.contains("unknown publish format \"swf\""), "{e}");
        for t in PublishTarget::ALL {
            assert!(e.contains(t.id()), "{e} lacks {}", t.id());
        }
    }

    #[test]
    fn non_publish_actions_are_not_targets() {
        for a in [FileAction::Open, FileAction::Save, FileAction::ImportImage, FileAction::ImportSound, FileAction::ExportSwatches] {
            assert_eq!(PublishTarget::of_action(a), None);
        }
    }

    #[test]
    fn save_copy_writes_a_package_or_json_that_opens_again() {
        let dir = std::env::temp_dir().join(format!("flare-pub-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut doc = ac_core::Document::default();
        doc.width = 321.0;
        let acx = dir.join("copy.acx");
        assert!(save_copy(&doc, &acx).unwrap().contains("copy.acx"));
        assert_eq!(ac_package::open(&acx).unwrap().doc.width, 321.0);
        let json = dir.join("copy.json");
        save_copy(&doc, &json).unwrap();
        assert_eq!(ac_core::Document::from_json(&std::fs::read_to_string(&json).unwrap()).unwrap().width, 321.0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
