//! Carrying out what the workspace asks of the outside world, through `flare-platform`.

use flare_animate::DialogRequest;
use flare_platform::{dialogs, dirs, jobs};
use winit::event_loop::EventLoopProxy;

use super::UserEvent;
use super::app::App;

impl App {
    /// A message box whose title is the message called `title_key` in the interface's language.
    /// What `detail` says comes from the library that failed, in English for now.
    pub(super) fn say_error(&self, title_key: &str, detail: &str) {
        dialogs::message(&self.workspace.interface_text(title_key), detail);
    }

    /// Shows the dialog the workspace asked for and hands it the answer.
    pub(super) fn handle_dialog(&mut self) {
        let Some(request) = self.workspace.take_dialog() else {
            return;
        };
        match request {
            DialogRequest::Open => {
                if let Some(path) =
                    dialogs::pick_open(&self.workspace.interface_text("dlg-filter-open"))
                {
                    self.workspace.open_any(&path);
                }
            }
            DialogRequest::SaveAs => {
                let (folder, name) = self.workspace.save_as_start();
                if let Some(path) = dialogs::save_document(
                    &self.workspace.interface_text("dlg-filter-fla"),
                    &self.workspace.interface_text("dlg-filter-xfl"),
                    &self.workspace.interface_text("dlg-filter-project"),
                    &name,
                    folder.as_deref(),
                ) {
                    self.workspace.save_as(&path);
                }
            }
            DialogRequest::ImportSound => {
                if let Some(path) =
                    dialogs::pick_sound(&self.workspace.interface_text("dlg-filter-sound"))
                {
                    self.workspace.import_sound(&path);
                }
            }
            DialogRequest::ExportImage => {
                if let Some(path) =
                    dialogs::save_png(&self.workspace.interface_text("dlg-filter-png"))
                {
                    let (document, frame) = self.workspace.image_export();
                    let done = exported(&self.proxy);
                    if let Err(error) = jobs::export_png(document, frame, path, done) {
                        self.say_error("dlg-cannot-export", &error.to_string());
                    }
                }
            }
            DialogRequest::ExportMovie => {
                if let Some(path) =
                    dialogs::save_gif(&self.workspace.interface_text("dlg-filter-gif"))
                {
                    let (document, _) = self.workspace.image_export();
                    let done = exported(&self.proxy);
                    if let Err(error) = jobs::export_gif(document, path, done) {
                        self.say_error("dlg-cannot-export", &error.to_string());
                    }
                }
            }
            DialogRequest::ExportFla => {
                if let Some(path) =
                    dialogs::save_fla(&self.workspace.interface_text("dlg-filter-fla"))
                {
                    let (document, _) = self.workspace.image_export();
                    let done = exported(&self.proxy);
                    if let Err(error) = jobs::export_fla(document, path, done) {
                        self.say_error("dlg-cannot-export", &error.to_string());
                    }
                }
            }
            DialogRequest::ExportSvg => {
                if let Some(path) =
                    dialogs::save_svg(&self.workspace.interface_text("dlg-filter-svg"))
                {
                    let (document, frame) = self.workspace.image_export();
                    let done = exported(&self.proxy);
                    if let Err(error) = jobs::export_svg(document, frame, path, done) {
                        self.say_error("dlg-cannot-export", &error.to_string());
                    }
                }
            }
            DialogRequest::Share(target) => {
                if let Err(error) = flare_platform::browser::open_url(target.url()) {
                    self.say_error("dlg-cannot-open-browser", &error.to_string());
                }
            }
            DialogRequest::Publish => {
                let settings = self.workspace.publish_settings();
                if !settings.any() {
                    self.say_error(
                        "dlg-publish",
                        &self.workspace.interface_text("dlg-choose-format"),
                    );
                } else if let Some(path) =
                    dialogs::save_publish(&self.workspace.interface_text("dlg-publish"))
                {
                    let (document, _) = self.workspace.image_export();
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
                    };
                    let done = exported(&self.proxy);
                    if let Err(error) = jobs::publish(document, path, plan, done) {
                        self.say_error("dlg-cannot-publish", &error.to_string());
                    }
                }
            }
        }
    }

    /// Plays the document when the workspace asks to: in a window of this app, or in a Ruffle
    /// of its own, depending on how the app was built.
    pub(super) fn handle_test_movie(&mut self) {
        // Forget players that have closed; this also removes their temporary files.
        self.movies
            .retain_mut(|movie| matches!(movie.try_wait(), Ok(None)));
        if !self.workspace.take_test_movie_request() {
            return;
        }
        let proxy = self.proxy.clone();
        let document = self.workspace.main_document().clone();
        self.start_movie(document, proxy);
    }

    /// Publishes on a thread; the window opens when the bytes come back.
    #[cfg(feature = "ruffle")]
    fn start_movie(
        &mut self,
        document: flare_document::Document,
        proxy: EventLoopProxy<UserEvent>,
    ) {
        let started = jobs::publish_movie(document, move |result| {
            // A closed event loop means the app is exiting; nothing to do.
            let _ = proxy.send_event(UserEvent::MovieBytes(result));
        });
        if let Err(error) = started {
            self.say_error("dlg-cannot-start-movie", &error.to_string());
        }
    }

    /// Opens the Test Movie window, a window of its own on the editor's GPU device.
    #[cfg(feature = "ruffle")]
    pub(super) fn open_movie(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        result: Result<Vec<u8>, String>,
    ) {
        let bytes = match result {
            Ok(bytes) => bytes,
            Err(error) => return self.say_error("dlg-cannot-start-movie", &error),
        };
        let movie_words = super::movie::MovieWords::from_lookup(|key, args| {
            self.workspace.interface_text_with(key, args)
        });
        let Some(gpu) = &mut self.gpu else { return };
        // One Test Movie window at a time. The old one goes first: the new one would share its
        // viewport, and with it the old window's surface, and so have nothing to draw on.
        if let Some(old) = self.movie.take() {
            old.release(&mut gpu.painter);
        }
        let scale = gpu.window.scale_factor() as f32;
        match super::movie_window::MovieViewport::open(
            event_loop,
            &self.ctx,
            &mut gpu.painter,
            bytes,
            scale,
            movie_words,
        ) {
            Ok(window) => self.movie = Some(window),
            Err(error) => self.say_error("dlg-cannot-start-movie", &format!("{error:#}")),
        }
    }

    /// Starts a Ruffle program of its own on the movie.
    #[cfg(not(feature = "ruffle"))]
    fn start_movie(
        &mut self,
        document: flare_document::Document,
        proxy: EventLoopProxy<UserEvent>,
    ) {
        let started = jobs::test_movie(document, move |result| {
            // A closed event loop means the app is exiting; the movie is dropped with it.
            let _ = proxy.send_event(UserEvent::MovieStarted(result));
        });
        match started {
            Ok(Ok(())) => {}
            Ok(Err(jobs::TestMovieStart::NoPlayer(text))) => {
                self.say_error("dlg-player-missing", text);
            }
            Err(error) => self.say_error("dlg-cannot-start-movie", &error.to_string()),
        }
    }

    /// Remembers the panel arrangement for the next session. Failure is not worth blocking an
    /// exit over, so it is only reported.
    pub(super) fn persist_layout(&self) {
        let (Some(path), Ok(json)) = (dirs::layout_file(), self.workspace.layout_json()) else {
            return;
        };
        if let Err(error) = dirs::write_layout(&path, &json) {
            eprintln!("could not save the layout: {error}");
        }
    }

    /// Remembers the preferences for the next session.
    pub(super) fn persist_preferences(&self) {
        let (Some(path), Ok(json)) = (dirs::preferences_file(), self.workspace.preferences_json())
        else {
            return;
        };
        if let Err(error) = dirs::write_file(&path, &json) {
            eprintln!("could not save the preferences: {error}");
        }
    }

    /// Whether closing may go ahead, asking first if there is unsaved work.
    pub(super) fn confirm_close(&mut self) -> bool {
        // A save still being written must finish first, or its work would be reported as lost.
        self.workspace.finish_background();
        if !self.workspace.any_dirty() {
            return true;
        }
        let discard = dialogs::confirm_discard(
            &self.workspace.interface_text("dlg-unsaved-title"),
            &self.workspace.interface_text("dlg-unsaved-text"),
        );
        if discard {
            // Closing without saving really discards the work, including its recovery file.
            self.workspace.discard_recovery();
        }
        discard
    }
}

/// A callback that reports a finished export or publish to the event loop.
fn exported(proxy: &EventLoopProxy<UserEvent>) -> impl FnOnce(Result<(), String>) + Send + 'static {
    let proxy = proxy.clone();
    move |result| {
        // A closed event loop means the app is exiting.
        let _ = proxy.send_event(UserEvent::Exported(result));
    }
}
