//! Files: native open/save dialogs, recent files, settings, autosave and
//! crash recovery, and the "unsaved changes" guard.

use crate::{App, Dialog};
use ac_core::{demo::demo_document, Document, Editor};
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum FileAction {
    Open,
    Save,
    ExportGif,
    ExportPng,
    ExportSvg,
    ExportPngSequence,
    ExportVideo,
    ImportImage,
    ImportSvg,
    ImportSequence,
    ImportPsd,
    ImportXdts,
    ExportXfl,
    ImportSound,
    ExportHtml5,
    ExportSpriteSheet,
    ExportAnimatedSvg,
    ExportLottie,
    ExportXdts,
    ImportSwatches,
    ExportSwatches,
}

impl FileAction {
    pub fn title(self) -> &'static str {
        match self {
            FileAction::Open => "Open document",
            FileAction::Save => "Save document as",
            FileAction::ExportGif => "Export animated GIF",
            FileAction::ExportPng => "Export current frame as an image (.png, .jpg or .gif)",
            FileAction::ExportSvg => "Export current frame as SVG",
            FileAction::ExportPngSequence => "Export PNG sequence (choose a folder)",
            FileAction::ExportVideo => "Export video (MP4 / WebM / MOV)",
            FileAction::ImportImage => "Import image (PNG / JPG / GIF / BMP)",
            FileAction::ImportSvg => "Import SVG",
            FileAction::ImportSequence => "Import image sequence (pick the first numbered image)",
            FileAction::ImportPsd => "Import Photoshop file (one layer per Photoshop layer)",
            FileAction::ImportXdts => "Import XDTS timing sheet",
            FileAction::ExportXfl => "Export as .fla / XFL",
            FileAction::ImportSound => "Import sound to library (WAV / MP3 / OGG / FLAC / AIFF)",
            FileAction::ExportHtml5 => "Publish HTML5 Canvas (a folder, or name.html for one file)",
            FileAction::ExportSpriteSheet => "Generate sprite sheet (PNG + data file)",
            FileAction::ExportAnimatedSvg => "Export animated SVG",
            FileAction::ExportLottie => "Export Lottie JSON",
            FileAction::ExportXdts => "Export Timing Sheet (XDTS)",
            FileAction::ImportSwatches => "Swatches from a palette (.aco, .ase, .gpl, .act) or gradients (.grd)",
            FileAction::ExportSwatches => "Save swatches (colours: .ase, .aco, .gpl; gradients: .grd)",
        }
    }
    fn default_name(self, doc_name: &str) -> String {
        match self {
            FileAction::Open | FileAction::ImportImage | FileAction::ImportSvg | FileAction::ImportSequence | FileAction::ImportSound | FileAction::ImportPsd | FileAction::ImportXdts | FileAction::ImportSwatches => String::new(),
            FileAction::ExportSwatches => "Swatches.ase".into(),
            FileAction::Save => format!("{doc_name}.acx"),
            FileAction::ExportGif => format!("{doc_name}.gif"),
            FileAction::ExportPng => format!("{doc_name}.png"),
            FileAction::ExportSvg => format!("{doc_name}.svg"),
            FileAction::ExportPngSequence => format!("{doc_name}-frames"),
            FileAction::ExportVideo => format!("{doc_name}.mp4"),
            FileAction::ExportXfl => format!("{doc_name}.fla"),
            FileAction::ExportHtml5 => format!("{doc_name}-html5"),
            FileAction::ExportSpriteSheet => format!("{doc_name}-sheet.png"),
            FileAction::ExportAnimatedSvg => format!("{doc_name}-animated.svg"),
            FileAction::ExportLottie => format!("{doc_name}.json"),
            FileAction::ExportXdts => format!("{doc_name}.xdts"),
        }
    }
    fn filters(self) -> (&'static str, &'static [&'static str]) {
        match self {
            FileAction::Open => ("Documents (.acx, .acx.json, .fla, .xfl)", &["acx", "json", "fla", "xfl"]),
            FileAction::Save => ("Flare document (.acx; .acx.json = plain JSON)", &["acx", "json"]),
            FileAction::ExportXfl => ("Animation document (.fla, .xfl)", &["fla", "xfl"]),
            FileAction::ImportSound => ("Sounds", &["wav", "mp3", "ogg", "oga", "flac", "aif", "aiff"]),
            FileAction::ExportHtml5 => ("Folder or .html", &[]),
            FileAction::ExportSpriteSheet => ("PNG", &["png"]),
            FileAction::ExportAnimatedSvg => ("SVG", &["svg"]),
            FileAction::ExportLottie => ("Lottie JSON", &["json"]),
            FileAction::ExportXdts => ("XDTS timing sheet", &["xdts"]),
            FileAction::ExportGif => ("Animated GIF", &["gif"]),
            FileAction::ExportPng => ("Image", &["png", "jpg", "jpeg", "gif"]),
            FileAction::ExportSvg | FileAction::ImportSvg => ("SVG", &["svg"]),
            FileAction::ExportPngSequence => ("Folder", &[]),
            FileAction::ExportVideo => ("Video", &["mp4", "webm", "mov"]),
            FileAction::ImportImage | FileAction::ImportSequence => ("Images", ac_core::codecs::EXTENSIONS),
            FileAction::ImportXdts => ("XDTS timing sheet", &["xdts"]),
            FileAction::ImportPsd => ("Photoshop files", &["psd"]),
            FileAction::ImportSwatches => ("Palettes and gradients", &["aco", "ase", "gpl", "act", "grd"]),
            FileAction::ExportSwatches => ("Palettes (.ase/.aco/.gpl) or gradients (.grd)", &["ase", "aco", "gpl", "grd"]),
        }
    }
    fn picks_existing(self) -> bool {
        matches!(self, FileAction::Open | FileAction::ImportImage | FileAction::ImportSvg | FileAction::ImportSequence | FileAction::ImportSound | FileAction::ImportPsd | FileAction::ImportXdts | FileAction::ImportSwatches)
    }
}

/// Flare packages (.acx).
pub(crate) fn is_package(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case(ac_package::EXTENSION))
}

/// First frame, 256 px wide, for the package thumbnail.
fn thumbnail(doc: &Document) -> Option<Vec<u8>> {
    let scale = (256.0 / doc.width.max(1.0)).min(1.0);
    ac_render::render_frame(doc, 0, scale).encode_png().ok()
}

/// .fla (zipped XFL), .xfl marker files and XFL folders.
pub(crate) fn is_xfl(p: &Path) -> bool {
    p.is_dir() || p.extension().map(|e| e.eq_ignore_ascii_case("fla") || e.eq_ignore_ascii_case("xfl")).unwrap_or(false)
}

/// Native dialogs can be turned off (headless UI tests): AC_NO_NATIVE_DIALOGS=1.
pub(crate) fn native_dialogs() -> bool {
    std::env::var_os("AC_NO_NATIVE_DIALOGS").is_none()
}

/// Persistent app settings (config dir / Flare / settings.json).
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct Settings {
    #[serde(default)]
    pub recent: Vec<String>,
    #[serde(default)]
    pub last_dir: Option<String>,
    #[serde(default)]
    pub library_left: bool,
    /// Properties › Filters › Presets: named filter lists.
    #[serde(default)]
    pub filter_presets: Vec<(String, Vec<ac_core::Filter>)>,
    /// Modify › Timeline › Keep Tween Curves When Splitting (Flare's timeline
    /// semantics). Off = Animate's behaviour.
    #[serde(default)]
    pub retain_tween_curves: bool,
    /// Startup window at launch (its "Show this at startup" box).
    #[serde(default = "yes")]
    pub show_startup: bool,
    /// Dock layout (Workspace menu / panel grips); None = standard.
    #[serde(default)]
    pub layout: Option<crate::dock::Layout>,
    /// Swatches (None = the standard set) and whether the column is shown.
    #[serde(default)]
    pub swatches: Option<Vec<ac_core::Swatch>>,
    #[serde(default)]
    pub swatches_open: bool,
    /// Window › Tool Options Bar.
    #[serde(default)]
    pub options_bar: bool,
    /// Convert to Symbol's registration cell, remembered ([column, row]).
    #[serde(default = "centre")]
    pub symbol_reg: [u8; 2],
    /// Document Settings' units (index into windows::UNITS).
    #[serde(default)]
    pub doc_units: u8,
    /// Edit › Preferences.
    #[serde(default)]
    pub prefs: Prefs,
    /// Timeline frame cell width (Frames menu sizes, Ctrl+wheel) and time display.
    #[serde(default = "cell_w")]
    pub tl_cell_w: f32,
    #[serde(default)]
    pub tl_seconds: bool,
}

fn cell_w() -> f32 {
    11.0
}

/// Edit › Preferences (prefs.rs).
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub(crate) struct Prefs {
    /// Seconds before unsaved changes are written to the recovery file (0 = off).
    pub autosave_secs: u32,
    pub undo_levels: u32,
    /// Interface size (1 = 100 %).
    pub ui_scale: f32,
    /// Off: clicking adds to the selection without Shift.
    pub shift_select: bool,
    pub tooltips: bool,
    /// Snap distance in screen pixels.
    pub snap_px: f32,
    /// New sessions start in Object Drawing (else Merge Drawing).
    pub object_drawing: bool,
    pub text_size: f32,
    /// Run frame scripts while playing.
    pub frame_scripts: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs { autosave_secs: 60, undo_levels: 200, ui_scale: 1.0, shift_select: true, tooltips: true, snap_px: 6.0, object_drawing: false, text_size: 28.0, frame_scripts: true }
    }
}

fn centre() -> [u8; 2] {
    [1, 1]
}

fn yes() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Settings { recent: vec![], last_dir: None, library_left: false, filter_presets: vec![], retain_tween_curves: false, show_startup: true, layout: None, swatches: None, swatches_open: false, options_bar: false, symbol_reg: [1, 1], doc_units: 0, prefs: Prefs::default(), tl_cell_w: 11.0, tl_seconds: false }
    }
}

fn app_dir(base: Option<PathBuf>) -> Option<PathBuf> {
    let base = base?;
    let d = base.join("Flare");
    // first run after the rename: bring settings, recent files and recovery over
    let old = base.join("AnimationCraft");
    if !d.exists() && old.is_dir() && std::fs::create_dir_all(&d).is_ok() {
        if let Ok(rd) = std::fs::read_dir(&old) {
            for f in rd.flatten().filter(|f| f.path().is_file()) {
                let _ = std::fs::copy(f.path(), d.join(f.file_name()));
            }
        }
    }
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}
fn settings_path() -> Option<PathBuf> {
    std::env::var_os("AC_CONFIG_DIR").map(PathBuf::from).map(|p| { let _ = std::fs::create_dir_all(&p); p.join("settings.json") }).or_else(|| app_dir(dirs::config_dir()).map(|d| d.join("settings.json")))
}
pub(crate) fn recovery_path() -> Option<PathBuf> {
    std::env::var_os("AC_CONFIG_DIR").map(|p| PathBuf::from(p).join("recovery.acx.json")).or_else(|| app_dir(dirs::data_local_dir()).map(|d| d.join("recovery.acx.json")))
}
fn recovery_meta_path() -> Option<PathBuf> {
    recovery_path().map(|p| p.with_file_name("recovery.meta.json"))
}

#[derive(Serialize, Deserialize)]
pub(crate) struct RecoveryMeta {
    pub original: Option<String>,
    pub saved_at: u64,
}

impl Settings {
    pub fn load() -> Settings {
        settings_path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }
    pub fn save(&self) {
        if let (Some(p), Ok(s)) = (settings_path(), serde_json::to_string_pretty(self)) {
            let _ = std::fs::write(p, s);
        }
    }
    pub fn add_recent(&mut self, path: &str) {
        let full = std::fs::canonicalize(path).map(|p| p.display().to_string()).unwrap_or_else(|_| path.to_string());
        self.recent.retain(|r| r != &full);
        self.recent.insert(0, full.clone());
        self.recent.truncate(10);
        self.last_dir = Path::new(&full).parent().map(|p| p.display().to_string());
        self.save();
    }
}

/// Something the user asked for that would discard unsaved changes.
#[derive(Clone, Debug)]
pub(crate) enum Pending {
    New,
    OpenDemo,
    Open,
    OpenPath(String),
    Quit,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl App {
    pub(crate) fn doc_name(&self) -> String {
        self.file_path
            .as_deref()
            .and_then(|p| Path::new(p).file_name())
            .map(|n| n.to_string_lossy().trim_end_matches(".json").trim_end_matches(".acx").trim_end_matches(".fla").trim_end_matches(".xfl").to_string())
            .or_else(|| self.untitled_name.clone())
            .unwrap_or_else(|| "Untitled".into())
    }
    pub(crate) fn dirty(&self) -> bool {
        self.ed.revision != self.saved_rev
    }

    /// Start a file action: native dialog when available, typed path otherwise.
    pub(crate) fn file(&mut self, action: FileAction) {
        let mut name = action.default_name(&self.doc_name());
        if action == FileAction::ExportPng {
            name = format!("{}.{}", self.doc_name(), ["png", "jpg", "gif"][self.publish.image_format as usize % 3]);
        }
        if !native_dialogs() {
            self.dialog = Dialog::Path { title: action.title().into(), path: if name.is_empty() { "untitled.acx".into() } else { name }, action };
            return;
        }
        let mut d = rfd::FileDialog::new().set_title(action.title());
        if let Some(dir) = self.settings.last_dir.clone().or_else(|| self.file_path.as_ref().and_then(|p| Path::new(p).parent().map(|d| d.display().to_string()))) {
            d = d.set_directory(dir);
        }
        let (label, exts) = action.filters();
        if !exts.is_empty() {
            d = d.add_filter(label, exts);
        }
        let picked = if action == FileAction::ExportPngSequence {
            d.pick_folder()
        } else if action.picks_existing() {
            d.pick_file()
        } else {
            d.set_file_name(&name).save_file()
        };
        if let Some(p) = picked {
            self.do_file(action, &p.display().to_string());
        }
    }

    pub(crate) fn save(&mut self) {
        match self.file_path.clone() {
            Some(p) => self.do_file(FileAction::Save, &p),
            None => self.file(FileAction::Save),
        }
    }

    /// Run `p` now, or ask first when there are unsaved changes.
    pub(crate) fn guarded(&mut self, p: Pending) {
        if self.dirty() {
            self.dialog = Dialog::Unsaved(p);
        } else {
            self.run_pending(p);
        }
    }

    pub(crate) fn run_pending(&mut self, p: Pending) {
        match p {
            // File › New: the Startup window's Create a New Scene
            Pending::New => self.startup = Some(crate::startup::Startup::new(self)),
            Pending::OpenDemo => self.load_doc(demo_document(), None),
            Pending::Open => self.file(FileAction::Open),
            Pending::OpenPath(path) => self.do_file(FileAction::Open, &path),
            Pending::Quit => {
                self.quit_confirmed = true;
                self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    pub(crate) fn load_doc(&mut self, doc: Document, path: Option<String>) {
        self.ed = Editor::new(doc);
        self.saved_rev = self.ed.revision;
        self.autosave_rev = self.ed.revision;
        self.file_path = path;
        self.untitled_name = None;
        self.fit_pending = true;
        self.lib_selected = None;
        self.ft_pivot = None;
    }

    pub(crate) fn do_file(&mut self, action: FileAction, path: &str) {
        let p = Path::new(path);
        let res: Result<String, String> = match action {
            FileAction::Open if is_xfl(p) => ac_xfl::open(p).map(|im| {
                let n = im.warnings.len();
                self.report_warnings("Open", path, &im.warnings);
                self.load_doc(im.doc, Some(path.into()));
                self.settings.add_recent(path);
                if n == 0 { format!("Opened {path}") } else { format!("Opened {path} ({n} notes, see Output)") }
            }),
            FileAction::Save | FileAction::ExportXfl if is_xfl(p) => {
                let doc = self.ed.saved_doc();
                ac_xfl::save(&doc, p).map(|w| {
                    self.report_warnings("Export", path, &w);
                    if action == FileAction::Save {
                        self.file_path = Some(path.into());
                        self.saved_rev = self.ed.revision;
                        self.settings.add_recent(path);
                        clear_recovery();
                    }
                    let n = w.len();
                    if n == 0 { format!("Saved {path}") } else { format!("Saved {path} ({n} notes about what the format can't hold, see Output)") }
                })
            }
            FileAction::ExportXfl => Err("choose a .fla or .xfl file name".into()),
            FileAction::ImportSound => self.import_sound(path),
            FileAction::Open if is_package(p) => ac_package::open(p).map(|l| {
                let n = l.warnings.len();
                self.report_warnings("Open", path, &l.warnings);
                self.load_doc(l.doc, Some(path.into()));
                self.settings.add_recent(path);
                if n == 0 { format!("Opened {path}") } else { format!("Opened {path} ({n} notes, see Output)") }
            }),
            FileAction::Save if is_package(p) => {
                let doc = self.ed.saved_doc();
                let thumb = thumbnail(&doc);
                ac_package::save(&doc, p, thumb.as_deref()).map(|_| {
                    self.file_path = Some(path.into());
                    self.saved_rev = self.ed.revision;
                    self.settings.add_recent(path);
                    clear_recovery();
                    format!("Saved {}", std::fs::canonicalize(p).map(|c| c.display().to_string()).unwrap_or(path.into()))
                })
            }
            FileAction::Open => std::fs::read_to_string(p).map_err(|e| e.to_string()).and_then(|s| {
                Document::from_json(&s).map_err(|e| format!("not a Flare document ({e})")).map(|d| {
                    self.load_doc(d, Some(path.into()));
                    self.settings.add_recent(path);
                    format!("Opened {path}")
                })
            }),
            FileAction::Save => std::fs::write(p, self.ed.saved_doc().to_json()).map_err(|e| e.to_string()).map(|_| {
                self.file_path = Some(path.into());
                self.saved_rev = self.ed.revision;
                self.settings.add_recent(path);
                clear_recovery();
                format!("Saved {}", std::fs::canonicalize(p).map(|c| c.display().to_string()).unwrap_or(path.into()))
            }),
            FileAction::ExportPng => {
                let fmt = self.publish.still_format(p.extension().and_then(|e| e.to_str()));
                ac_render::export_still(&self.ed.doc, self.ed.frame, self.publish.scale.clamp(0.05, 8.0), fmt, p).map(|_| format!("Exported frame {} to {path}", self.ed.frame + 1))
            }
            FileAction::ExportSvg => std::fs::write(p, ac_render::export_svg(&self.ed.doc, self.ed.frame)).map_err(|e| e.to_string()).map(|_| format!("Exported SVG to {path}")),
            other => crate::publish::run_file_action(self, other, path),
        };
        let saved = action == FileAction::Save && res.is_ok();
        self.status = res.unwrap_or_else(|e| format!("Error: {e}"));
        if saved {
            if let Some(p) = self.after_save.take() {
                self.run_pending(p);
            }
        } else if action == FileAction::Save {
            self.after_save = None;
        }
    }

    /// File ▸ Import ▸ Import Sound: into the library; on an empty keyframe
    /// with no sound yet (an audio layer) it also starts there.
    pub(crate) fn import_sound(&mut self, path: &str) -> Result<String, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        let pcm = ac_audio::decode(&bytes).map_err(|e| format!("{path}: {e}"))?;
        let name = Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Sound".into());
        let id = self.ed.add_sound(&name, bytes);
        self.lib.sound = Some(id);
        self.lib.bitmap = None;
        self.lib_selected = None;
        let attach = self.ed.current_keyframe().is_some_and(|(_, k)| k.elements.is_empty() && k.sound.is_none());
        if attach {
            self.ed.set_frame_sound(Some(ac_core::sound::FrameSound::new(id)));
        }
        Ok(format!("Imported {name} ({:.1} s){}", pcm.seconds(), if attach { " onto this keyframe" } else { " to the library; drag it onto the stage to start it on the current keyframe" }))
    }

    fn report_warnings(&mut self, what: &str, path: &str, w: &[String]) {
        if w.is_empty() {
            return;
        }
        self.scripting.push(crate::scripting::OutKind::Info, format!("{what} {path}:"));
        for line in w {
            self.scripting.push(crate::scripting::OutKind::Info, format!("  • {line}"));
        }
    }

    /// Autosave once changes have sat unsaved for `AC_AUTOSAVE_SECS` (default 60 s).
    pub(crate) fn autosave_tick(&mut self, ctx: &egui::Context) {
        if self.ed.revision == self.autosave_rev {
            self.dirty_since = None;
            return;
        }
        let interval: f64 = match std::env::var("AC_AUTOSAVE_SECS").ok().and_then(|s| s.parse().ok()) {
            Some(v) => v,
            None if self.settings.prefs.autosave_secs == 0 => return,
            None => self.settings.prefs.autosave_secs as f64,
        };
        let t = ctx.input(|i| i.time);
        let since = *self.dirty_since.get_or_insert(t);
        if t - since < interval {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64((interval - (t - since)).max(0.2)));
            return;
        }
        if let (Some(p), Some(m)) = (recovery_path(), recovery_meta_path()) {
            let meta = RecoveryMeta { original: self.file_path.clone(), saved_at: now_secs() };
            if std::fs::write(&p, self.ed.saved_doc().to_json()).is_ok() {
                let _ = std::fs::write(m, serde_json::to_string(&meta).unwrap_or_default());
                self.autosave_rev = self.ed.revision;
                self.dirty_since = None;
            }
        }
    }

    /// On launch: offer to restore an autosave left behind by a crash.
    pub(crate) fn check_recovery(&mut self) {
        let Some(p) = recovery_path() else { return };
        if !p.exists() {
            return;
        }
        let meta: Option<RecoveryMeta> = recovery_meta_path().and_then(|m| std::fs::read_to_string(m).ok()).and_then(|s| serde_json::from_str(&s).ok());
        self.dialog = Dialog::Recover { original: meta.as_ref().and_then(|m| m.original.clone()), age_secs: meta.map(|m| now_secs().saturating_sub(m.saved_at)).unwrap_or(0) };
    }

    pub(crate) fn restore_recovery(&mut self) {
        let Some(p) = recovery_path() else { return };
        match std::fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|s| Document::from_json(&s).map_err(|e| e.to_string())) {
            Ok(d) => {
                let original: Option<String> = recovery_meta_path().and_then(|m| std::fs::read_to_string(m).ok()).and_then(|s| serde_json::from_str::<RecoveryMeta>(&s).ok()).and_then(|m| m.original);
                self.load_doc(d, original);
                self.saved_rev = u64::MAX; // restored work is unsaved
                self.status = "Restored your work from the last autosave. Save it to keep it.".into();
            }
            Err(e) => self.status = format!("Couldn't restore the autosave: {e}"),
        }
    }
}

pub(crate) fn clear_recovery() {
    for p in [recovery_path(), recovery_meta_path()].into_iter().flatten() {
        let _ = std::fs::remove_file(p);
    }
}

pub(crate) fn fmt_age(secs: u64) -> String {
    match secs {
        0..=89 => "just now".into(),
        90..=5399 => format!("{} minutes ago", secs / 60),
        5400..=172_799 => format!("{} hours ago", secs / 3600),
        _ => format!("{} days ago", secs / 86400),
    }
}
