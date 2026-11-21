//! Showing the dialogs the workspace asks for. A file picker blocks until the person answers, so
//! it runs on a thread of its own and the answer comes back through the event loop: a picker
//! opened on the UI thread stops the window painting, and on some desktops the picker itself then
//! stays blank until the person touches it (docs/PERFORMANCE.md: nothing slow on the UI thread).

use flare_animate::DialogRequest;
use flare_platform::{dialogs, jobs};

use super::UserEvent;
use super::app::App;

/// The follow-up of a picker: what is done with the answer, back on the UI thread.
pub(super) type Answer = Box<dyn FnOnce(&mut App) + Send>;

impl App {
    /// Runs `pick` (a file picker) off the UI thread; `then` gets what it returned, on the UI
    /// thread, unless the person cancelled. One picker is open at a time.
    fn ask<T: Send + 'static>(
        &mut self,
        pick: impl FnOnce() -> Option<T> + Send + 'static,
        then: impl FnOnce(&mut App, T) + Send + 'static,
    ) {
        self.dialog_open = true;
        let proxy = self.proxy.clone();
        let started = jobs::ask(pick, move |answer| {
            let follow: Answer = Box::new(move |app| {
                app.dialog_open = false;
                if let Some(answer) = answer {
                    then(app, answer);
                }
            });
            // A closed event loop means the app is exiting.
            let _ = proxy.send_event(UserEvent::Answer(follow));
        });
        if let Err(error) = started {
            self.dialog_open = false;
            self.say_error("dlg-cannot-read", &error.to_string());
        }
    }

    /// Shows the dialog the workspace asked for and hands it the answer.
    pub(super) fn handle_dialog(&mut self) {
        if self.dialog_open {
            return;
        }
        let Some(request) = self.workspace.take_dialog() else {
            return;
        };
        let text = |app: &App, key: &str| app.workspace.interface_text(key);
        match request {
            DialogRequest::Open => {
                let filter = text(self, "dlg-filter-open");
                self.ask(
                    move || dialogs::pick_open(&filter),
                    |app, path| app.workspace.open_any(&path),
                );
            }
            DialogRequest::SaveAs => {
                let (folder, name) = self.workspace.save_as_start();
                let filters = ["dlg-filter-fla", "dlg-filter-xfl", "dlg-filter-project"]
                    .map(|key| text(self, key));
                self.ask(
                    move || {
                        let [fla, xfl, flare] = &filters;
                        dialogs::save_document(fla, xfl, flare, &name, folder.as_deref())
                    },
                    |app, path| app.workspace.save_as(&path),
                );
            }
            DialogRequest::ImportSound => {
                let filter = text(self, "dlg-filter-sound");
                self.ask(
                    move || dialogs::pick_sound(&filter),
                    |app, path| app.workspace.import_sound(&path),
                );
            }
            DialogRequest::ImportVideo => {
                let filter = text(self, "dlg-filter-video");
                self.ask(
                    move || dialogs::pick_video(&filter),
                    |app, path| app.workspace.import_video(&path),
                );
            }
            DialogRequest::ImportMannequinReference => {
                let filter = text(self, "mannequin-filter-picture");
                self.ask(
                    move || dialogs::pick_image(&filter, &flare_animate::REFERENCE_EXTENSIONS),
                    |app, path| app.workspace.import_mannequin_reference(&path),
                );
            }
            DialogRequest::ImportBrushProfiles => {
                let filter = text(self, "dlg-filter-brushes");
                self.ask(
                    move || dialogs::pick_image(&filter, flare_animate::PROFILE_EXTENSIONS),
                    |app, path| app.import_brush_profiles_from(&path),
                );
            }
            DialogRequest::ImportAnimateShortcuts => {
                let filter = text(self, "dlg-filter-animate-shortcuts");
                self.ask(
                    move || dialogs::pick_shortcut_xml(&filter),
                    |app, path| app.import_shortcuts_from(&path),
                );
            }
            DialogRequest::ExportImage => {
                let filter = text(self, "dlg-filter-image");
                self.ask(
                    move || dialogs::save_image(&filter, &jobs::IMAGE_ENDINGS),
                    |app, path| {
                        let (document, frame) = app.workspace.image_export();
                        let backdrop = app.workspace.publish_settings().backdrop();
                        let done = exported(&app.proxy);
                        if let Err(error) =
                            jobs::export_image(document, frame, path, backdrop, done)
                        {
                            app.say_error("dlg-cannot-export", &error.to_string());
                        }
                    },
                );
            }
            DialogRequest::ExportMovie => {
                let names = [
                    "dlg-filter-gif",
                    "dlg-filter-mp4",
                    "dlg-filter-mov",
                    "dlg-filter-movie",
                ]
                .map(|key| text(self, key));
                self.ask(
                    move || {
                        let filters = [
                            (names[0].as_str(), &["gif"][..]),
                            (names[1].as_str(), &["mp4"][..]),
                            (names[2].as_str(), &["mov"][..]),
                            (names[3].as_str(), &jobs::OTHER_MOVIE_ENDINGS),
                        ];
                        dialogs::save_movie(&filters)
                    },
                    |app, path| {
                        let (document, _) = app.workspace.image_export();
                        let backdrop = app.workspace.publish_settings().backdrop();
                        let done = exported(&app.proxy);
                        if let Err(error) = jobs::export_movie(document, path, backdrop, done) {
                            app.say_error("dlg-cannot-export", &error.to_string());
                        }
                    },
                );
            }
            DialogRequest::ExportFla => {
                let filter = text(self, "dlg-filter-fla");
                self.ask(
                    move || dialogs::save_fla(&filter),
                    |app, path| {
                        let (document, _) = app.workspace.image_export();
                        let done = exported(&app.proxy);
                        if let Err(error) = jobs::export_fla(document, path, done) {
                            app.say_error("dlg-cannot-export", &error.to_string());
                        }
                    },
                );
            }
            DialogRequest::ExportSvg => {
                let filter = text(self, "dlg-filter-svg");
                self.ask(
                    move || dialogs::save_svg(&filter),
                    |app, path| {
                        let (document, frame) = app.workspace.image_export();
                        let backdrop = app.workspace.publish_settings().backdrop();
                        let done = exported(&app.proxy);
                        if let Err(error) = jobs::export_svg(document, frame, path, backdrop, done)
                        {
                            app.say_error("dlg-cannot-export", &error.to_string());
                        }
                    },
                );
            }
            DialogRequest::Publish => self.ask_publish(),
        }
    }

    fn ask_publish(&mut self) {
        if !self.workspace.publish_settings().any() {
            let detail = self.workspace.interface_text("dlg-choose-format");
            self.say_error("dlg-publish", &detail);
            return;
        }
        let (folder, name) = self.workspace.publish_start();
        let title = self.workspace.interface_text("dlg-publish");
        self.ask(
            move || dialogs::save_publish(&title, &name, folder.as_deref()),
            |app, path| {
                let settings = app.workspace.publish_settings();
                let (document, _) = app.workspace.image_export();
                let plan = jobs::PublishPlan {
                    swf: settings.swf.then_some(jobs::PublishOptions {
                        compress: settings.compress,
                    }),
                    html5: settings.html5.then(|| jobs::Html5Options {
                        loop_timeline: settings.loop_timeline,
                        center_stage: settings.center_stage,
                        responsive: settings.responsive,
                        ..jobs::Html5Options::default()
                    }),
                    gif: settings.gif,
                    mp4: settings.mp4,
                    mov: settings.mov,
                    backdrop: settings.backdrop(),
                };
                let done = exported(&app.proxy);
                if let Err(error) = jobs::publish(document, path, plan, done) {
                    app.say_error("dlg-cannot-publish", &error.to_string());
                }
            },
        );
    }

    /// Reads a brush profile collection the person picked. One larger than any real one is
    /// refused before its bytes are read.
    fn import_brush_profiles_from(&mut self, path: &std::path::Path) {
        let too_large = std::fs::metadata(path)
            .is_ok_and(|m| m.len() > flare_animate::MAX_BRUSH_PROFILE_FILE_BYTES);
        match std::fs::read(path) {
            Ok(bytes) if !too_large => self
                .workspace
                .import_brush_profiles(&path.to_string_lossy(), &bytes),
            Ok(_) => self.say_error("dlg-cannot-read", "the file is too large"),
            Err(error) => self.say_error("dlg-cannot-read", &error.to_string()),
        }
    }

    /// Reads an Animate shortcut file the person picked; one larger than any Animate writes is
    /// not read.
    fn import_shortcuts_from(&mut self, path: &std::path::Path) {
        let too_large = std::fs::metadata(path)
            .is_ok_and(|m| m.len() > flare_menubar::MAX_ANIMATE_SHORTCUT_BYTES as u64);
        match std::fs::read(path) {
            Ok(bytes) if !too_large => self.workspace.import_animate_shortcuts(&bytes),
            Ok(_) => self.say_error(
                "dlg-cannot-read",
                &flare_menubar::ShortcutError::TooLarge(flare_menubar::MAX_ANIMATE_SHORTCUT_BYTES)
                    .to_string(),
            ),
            Err(error) => self.say_error("dlg-cannot-read", &error.to_string()),
        }
    }
}

/// A callback that reports a finished export or publish to the event loop.
fn exported(
    proxy: &winit::event_loop::EventLoopProxy<UserEvent>,
) -> impl FnOnce(Result<(), String>) + Send + 'static {
    let proxy = proxy.clone();
    move |result| {
        // A closed event loop means the app is exiting.
        let _ = proxy.send_event(UserEvent::Exported(result));
    }
}
