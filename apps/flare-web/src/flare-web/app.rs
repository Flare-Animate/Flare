//! The page's application: the workspace, plus what a page does when the workspace asks for a
//! file.

use std::cell::RefCell;
use std::rc::Rc;

use crate::sound::BrowserSound;
use crate::{files, flare_file, preferences, ruffle};
use eframe::egui;
use flare_animate::{DialogRequest, Workspace};

/// A file the person picked, waiting to be handed to the workspace.
struct Picked {
    name: String,
    bytes: Vec<u8>,
}

/// Whether `name` is a Flare project (`.flare`), which the page reads itself, rather than a
/// Flash document to import.
fn is_flare(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".flare")
}

pub struct WebApp {
    workspace: Workspace,
    picked: Rc<RefCell<Vec<Picked>>>,
    picked_sounds: Rc<RefCell<Vec<Picked>>>,
    /// Animate shortcut files picked, read on the next frame.
    picked_shortcuts: Rc<RefCell<Vec<Picked>>>,
    picked_references: Rc<RefCell<Vec<Picked>>>,
    /// What background work (the movie player) wants the person told, delivered on the next frame.
    messages: Rc<RefCell<Vec<String>>>,
    /// The preferences as last written, so storage is touched only when they change.
    saved_preferences: String,
    /// The name of the file last opened or saved, offered again by Save and, without its ending,
    /// by Publish; none for a new document.
    last_name: Option<String>,
    /// Browser audio output; playback is driven by the same events as the desktop player.
    sound: BrowserSound,
}

impl WebApp {
    pub fn new(workspace: Workspace) -> Self {
        let saved_preferences = workspace.preferences_json().unwrap_or_default();
        Self {
            workspace,
            picked: Rc::default(),
            picked_sounds: Rc::default(),
            picked_shortcuts: Rc::default(),
            picked_references: Rc::default(),
            messages: Rc::default(),
            saved_preferences,
            last_name: None,
            sound: BrowserSound::default(),
        }
    }

    /// Does what the workspace asked of the outside world.
    fn handle_dialog(&mut self, ctx: &egui::Context) {
        let Some(request) = self.workspace.take_dialog() else {
            return;
        };
        match request {
            DialogRequest::Open => {
                let (inbox, ctx) = (Rc::clone(&self.picked), ctx.clone());
                wasm_bindgen_futures::spawn_local(async move {
                    if let Some((name, bytes)) = files::pick(
                        "Flare project, Flash, Moho, Toonz, BVH or SVG file",
                        &[
                            "flare",
                            "fla",
                            "swf",
                            "moho",
                            "anime",
                            "mohoproj",
                            "animeproj",
                            "bvh",
                            "svg",
                            "jpg",
                            "jpeg",
                            "bmp",
                            "tga",
                            "psd",
                            "mp4",
                            "mov",
                            "avi",
                            "mkv",
                            "webm",
                            "mpeg",
                            "mpg",
                            "flv",
                            "f4v",
                            "f4p",
                            "spl",
                            "swt",
                            "swc",
                            "flp",
                            "tif",
                            "tiff",
                            "webp",
                            "ico",
                            "qoi",
                            "pnm",
                            "pbm",
                            "pgm",
                            "pam",
                            "hdr",
                            "exr",
                            "dds",
                            "tnz",
                            "tpl",
                            "plt",
                            "pli",
                            "tlv",
                            "fc",
                            "wick",
                        ],
                    )
                    .await
                    {
                        inbox.borrow_mut().push(Picked { name, bytes });
                        ctx.request_repaint();
                    }
                });
            }
            DialogRequest::SaveAs => self.save_document(),
            DialogRequest::ExportImage => {
                let (document, frame) = self.workspace.image_export();
                self.deliver(
                    "frame.png",
                    "image/png",
                    flare_runtime::png_bytes(&document, frame),
                );
            }
            DialogRequest::ExportMovie => {
                let (document, _) = self.workspace.image_export();
                self.deliver(
                    "movie.gif",
                    "image/gif",
                    flare_runtime::gif_bytes(&document),
                );
            }
            DialogRequest::ExportFla => {
                let (document, _) = self.workspace.image_export();
                let bytes = flare_export::export_xfl(&document)
                    .and_then(|output| flare_export::fla_bytes(&output))
                    .map_err(|e| e.to_string());
                self.deliver("project.fla", "application/zip", bytes);
            }
            DialogRequest::ExportSvg => {
                let (document, frame) = self.workspace.image_export();
                let bytes: Result<Vec<u8>, String> =
                    flare_runtime::svg_export(&document, frame).map(|svg| svg.text.into_bytes());
                self.deliver("frame.svg", "image/svg+xml", bytes);
            }
            DialogRequest::Publish => {
                let (document, _) = self.workspace.image_export();
                let name = self.publish_name();
                let settings = self.workspace.publish_settings();
                if !settings.any() {
                    self.workspace
                        .report_message("Choose a format in Publish Settings first");
                }
                if settings.swf {
                    let options = flare_publish::PublishOptions {
                        compress: settings.compress,
                    };
                    let result = flare_publish::publish_swf(&document, &options)
                        .map(|published| published.bytes)
                        .map_err(|e| e.to_string());
                    self.deliver(
                        &format!("{name}.swf"),
                        "application/x-shockwave-flash",
                        result,
                    );
                }
                if settings.mp4 {
                    self.workspace
                        .report_message("MP4 is not available in the browser yet");
                }
                if settings.mov {
                    self.workspace
                        .report_message("MOV is not available in the browser yet");
                }
                if settings.gif {
                    self.deliver(
                        &format!("{name}.gif"),
                        "image/gif",
                        flare_runtime::gif_bytes(&document),
                    );
                }
                if settings.html5 {
                    let options = flare_publish::Html5Options {
                        title: name.clone(),
                        loop_timeline: settings.loop_timeline,
                        center_stage: settings.center_stage,
                        responsive: settings.responsive,
                    };
                    let result = flare_publish::publish_html5(&document, &options)
                        .map(|published| published.html.into_bytes())
                        .map_err(|e| e.to_string());
                    self.deliver(&format!("{name}.html"), "text/html", result);
                }
            }
            DialogRequest::ImportMannequinReference => {
                let (inbox, messages, ctx) = (
                    Rc::clone(&self.picked_references),
                    Rc::clone(&self.messages),
                    ctx.clone(),
                );
                let filter = self.workspace.interface_text("mannequin-filter-picture");
                wasm_bindgen_futures::spawn_local(async move {
                    match files::pick_bounded(
                        &filter,
                        &flare_animate::REFERENCE_EXTENSIONS,
                        flare_animate::MAX_REFERENCE_BYTES,
                    )
                    .await
                    {
                        Ok(Some((name, bytes))) => {
                            inbox.borrow_mut().push(Picked { name, bytes });
                            ctx.request_repaint();
                        }
                        Ok(None) => {}
                        Err(error) => {
                            messages.borrow_mut().push(error.to_string());
                            ctx.request_repaint();
                        }
                    }
                });
            }
            DialogRequest::ImportAnimateShortcuts => {
                let (inbox, messages, ctx) = (
                    Rc::clone(&self.picked_shortcuts),
                    Rc::clone(&self.messages),
                    ctx.clone(),
                );
                let filter = self
                    .workspace
                    .interface_text("dlg-filter-animate-shortcuts");
                wasm_bindgen_futures::spawn_local(async move {
                    match files::pick_bounded(
                        &filter,
                        &["xml"],
                        flare_menubar::MAX_ANIMATE_SHORTCUT_BYTES,
                    )
                    .await
                    {
                        Ok(Some((name, bytes))) => {
                            inbox.borrow_mut().push(Picked { name, bytes });
                            ctx.request_repaint();
                        }
                        Ok(None) => {}
                        Err(error) => {
                            messages.borrow_mut().push(error.to_string());
                            ctx.request_repaint();
                        }
                    }
                });
            }
            // Dimmed in the browser, which has no ffmpeg; said again should anything ask.
            DialogRequest::ImportVideo => self
                .messages
                .borrow_mut()
                .push(self.workspace.interface_text("video-browser")),
            DialogRequest::ImportSound => {
                let (inbox, messages, ctx) = (
                    Rc::clone(&self.picked_sounds),
                    Rc::clone(&self.messages),
                    ctx.clone(),
                );
                wasm_bindgen_futures::spawn_local(async move {
                    match files::pick_bounded(
                        "WAV, MP3, Ogg Vorbis or FLAC sound",
                        &["wav", "mp3", "ogg", "flac"],
                        flare_runtime::MAX_AUDIO_INPUT_BYTES,
                    )
                    .await
                    {
                        Ok(Some((name, bytes))) => {
                            inbox.borrow_mut().push(Picked { name, bytes });
                            ctx.request_repaint();
                        }
                        Ok(None) => {}
                        Err(error) => {
                            messages
                                .borrow_mut()
                                .push(format!("Cannot import sound: {error}"));
                            ctx.request_repaint();
                        }
                    }
                });
            }
        }
    }

    /// Save and Save As are a download: a page has no file to write back to. The person names it,
    /// and the ending picks the format as on the desktop: `.fla` (the default, as in Animate) is a
    /// Flash document Animate opens, `.flare` is Flare's own project (the page brings SQLite and
    /// zstd as WebAssembly for it, see `flare_file`). A folder cannot be downloaded, so `.xfl`
    /// is saved as a `.fla`, which holds the same files.
    fn save_document(&mut self) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let message = self.workspace.interface_text("dlg-save-prompt");
        let typed = window
            .prompt_with_message_and_default(
                &message,
                self.last_name.as_deref().unwrap_or("project.fla"),
            )
            .ok()
            .flatten()
            .unwrap_or_default();
        // A name only: a path typed in is cut to its last part.
        let Some(name) = std::path::Path::new(typed.trim()).file_name() else {
            return;
        };
        let (named, format) = flare_runtime::resolve_save_path(std::path::Path::new(name));
        let named = if format == flare_runtime::FileFormat::Xfl {
            named.with_extension("fla")
        } else {
            named
        };
        let name = named.to_string_lossy().into_owned();
        let (document, _) = self.workspace.image_export();
        let saved = if format == flare_runtime::FileFormat::Flare {
            let bytes = flare_file::FlareFile::registered()
                .and_then(|library| library.write(&document))
                .map_err(|e| e.to_string());
            (bytes, None)
        } else {
            match flare_export::export_xfl(&document) {
                Ok(output) => (
                    flare_export::fla_bytes(&output).map_err(|e| e.to_string()),
                    Some(flare_runtime::summarize(&output)),
                ),
                Err(error) => (Err(error.to_string()), None),
            }
        };
        let (bytes, summary) = saved;
        let ok = bytes.is_ok();
        let mime = if summary.is_some() {
            "application/zip"
        } else {
            "application/octet-stream"
        };
        self.deliver(&name, mime, bytes);
        if ok {
            self.last_name = Some(name.clone());
            self.workspace.report_saved(&flare_runtime::Saved {
                path: named,
                format: if summary.is_some() {
                    flare_runtime::FileFormat::Fla
                } else {
                    flare_runtime::FileFormat::Flare
                },
                summary,
            });
        }
    }

    /// Opens a `.flare` the person picked, which the page reads itself.
    fn open_flare(&mut self, name: &str, bytes: &[u8]) {
        let read = flare_file::FlareFile::registered().and_then(|library| library.read(bytes));
        match read {
            Ok(document) => self.workspace.open_loaded(name, document),
            Err(error) => self
                .workspace
                .report_message(&format!("Cannot open {name}: {error}")),
        }
    }

    /// Plays the document in the page's Ruffle when the workspace asks to.
    fn handle_test_movie(&mut self, ctx: &egui::Context) {
        if !self.workspace.take_test_movie_request() {
            return;
        }
        // The main document, so a symbol being edited is included, as on the desktop.
        let document = self.workspace.main_document().clone();
        let swf = flare_publish::publish_swf(&document, &flare_publish::PublishOptions::default())
            .map(|published| published.bytes)
            .map_err(|e| e.to_string());
        let (inbox, ctx) = (Rc::clone(&self.messages), ctx.clone());
        wasm_bindgen_futures::spawn_local(async move {
            let played = match swf {
                Ok(bytes) => match ruffle::registered() {
                    Ok(host) => ruffle::play_in(&host, &bytes).await,
                    Err(error) => Err(error),
                },
                Err(error) => Err(error),
            };
            if let Err(error) = played {
                inbox
                    .borrow_mut()
                    .push(format!("Cannot start Test Movie: {error}"));
                ctx.request_repaint();
            }
        });
    }

    /// What Publish names its downloads: the file last opened or saved, without its ending, or
    /// the workspace's name for a new document.
    fn publish_name(&self) -> String {
        self.last_name
            .as_deref()
            .and_then(|name| std::path::Path::new(name).file_stem())
            .map_or_else(
                || self.workspace.publish_start().1,
                |stem| stem.to_string_lossy().into_owned(),
            )
    }

    fn deliver(&mut self, name: &str, mime: &str, bytes: Result<Vec<u8>, String>) {
        match bytes.and_then(|b| files::download(name, mime, &b)) {
            Ok(()) => {}
            Err(error) => self.workspace.report_message(&error),
        }
    }

    fn handle_audio(&mut self) {
        let events = self.workspace.take_audio_events();
        if let Err(message) = self.sound.apply(events) {
            self.workspace.report_message(&message);
        }
    }
}

impl eframe::App for WebApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let picked: Vec<Picked> = self.picked.borrow_mut().drain(..).collect();
        for file in picked {
            self.last_name = Some(file.name.clone());
            if is_flare(&file.name) {
                self.open_flare(&file.name, &file.bytes);
            } else {
                self.workspace.import_project_bytes(&file.name, file.bytes);
            }
        }
        let sounds: Vec<Picked> = self.picked_sounds.borrow_mut().drain(..).collect();
        for sound in sounds {
            self.workspace.import_sound_bytes(&sound.name, sound.bytes);
        }
        let shortcuts: Vec<Picked> = self.picked_shortcuts.borrow_mut().drain(..).collect();
        for file in shortcuts {
            self.workspace.import_animate_shortcuts(&file.bytes);
        }
        let references: Vec<Picked> = self.picked_references.borrow_mut().drain(..).collect();
        for file in references {
            self.workspace
                .import_mannequin_reference_bytes(&file.name, file.bytes);
        }
        let messages: Vec<String> = self.messages.borrow_mut().drain(..).collect();
        for message in messages {
            self.workspace.report_message(&message);
        }
        self.workspace.show(ui);
        self.handle_audio();
        self.handle_dialog(&ctx);
        self.handle_test_movie(&ctx);
        if let Ok(json) = self.workspace.preferences_json()
            && json != self.saved_preferences
        {
            preferences::save(&json);
            self.saved_preferences = json;
        }
    }
}
