//! Carrying out what the workspace asks of the outside world, through `flare-platform`.

use flare_animate::DialogRequest;
use flare_platform::{dialogs, dirs, jobs};
use winit::event_loop::EventLoopProxy;

use super::UserEvent;
use super::app::App;

impl App {
    /// Shows the dialog the workspace asked for and hands it the answer.
    pub(super) fn handle_dialog(&mut self) {
        let Some(request) = self.workspace.take_dialog() else {
            return;
        };
        match request {
            DialogRequest::Open => {
                if let Some(path) = dialogs::pick_project() {
                    self.workspace.open(&path);
                }
            }
            DialogRequest::SaveAs => {
                if let Some(path) = dialogs::save_project() {
                    self.workspace.save_as(&path);
                }
            }
            DialogRequest::ImportProject => {
                if let Some(path) = dialogs::pick_import() {
                    self.workspace.import_project(&path);
                }
            }
            DialogRequest::ImportSound => {
                if let Some(path) = dialogs::pick_sound() {
                    self.workspace.import_sound(&path);
                }
            }
            DialogRequest::ExportImage => {
                if let Some(path) = dialogs::save_png() {
                    let (document, frame) = self.workspace.image_export();
                    let done = exported(&self.proxy);
                    if let Err(error) = jobs::export_png(document, frame, path, done) {
                        dialogs::message("Cannot export", &error.to_string());
                    }
                }
            }
            DialogRequest::PublishSwf => {
                if let Some(path) = dialogs::save_swf() {
                    let (document, _) = self.workspace.image_export();
                    let done = exported(&self.proxy);
                    if let Err(error) = jobs::publish_swf(document, path, done) {
                        dialogs::message("Cannot publish", &error.to_string());
                    }
                }
            }
        }
    }

    /// Plays the document in an external Ruffle when the workspace asks to.
    pub(super) fn handle_test_movie(&mut self) {
        // Forget players that have closed; this also removes their temporary files.
        self.movies
            .retain_mut(|movie| matches!(movie.try_wait(), Ok(None)));
        if !self.workspace.take_test_movie_request() {
            return;
        }
        let proxy = self.proxy.clone();
        let document = self.workspace.main_document().clone();
        let started = jobs::test_movie(document, move |result| {
            // A closed event loop means the app is exiting; the movie is dropped with it.
            let _ = proxy.send_event(UserEvent::MovieStarted(result));
        });
        match started {
            Ok(Ok(())) => {}
            Ok(Err(jobs::TestMovieStart::NoPlayer(text))) => {
                dialogs::message("Ruffle not found", text);
            }
            Err(error) => dialogs::message("Cannot start Test Movie", &error.to_string()),
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
        if !self.workspace.is_dirty() {
            return true;
        }
        let discard = dialogs::confirm_discard();
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
