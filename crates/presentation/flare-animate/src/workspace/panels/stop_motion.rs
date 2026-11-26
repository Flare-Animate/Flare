//! The Stop Motion panel: the project's capture session. Frames come from picture files (no camera
//! backend is built), each held for a number of timeline frames, then laid on a layer. Every edit
//! is `flare_editor::stop_motion_library`'s, the same `flare stop-motion-project` and the MCP
//! server make, so each is one undo step and the library refuses what is not valid.

use egui::Ui;
use flare_document::{Bitmap, MAX_STOP_MOTION_FPS, MAX_STOP_MOTION_HOLD};
use flare_editor::stop_motion_library as library;
use flare_ui::{chrome, theme};

use crate::workspace::{DialogRequest, Workspace};

const LABEL: &str = "Edit stop motion";

/// What the panel remembers while it is open.
pub(crate) struct StopMotionPanel {
    pub(crate) fps: u32,
    pub(crate) hold: u32,
}

impl Default for StopMotionPanel {
    fn default() -> Self {
        Self { fps: 12, hold: 1 }
    }
}

/// Pictures read from files, decoded off the UI thread by whoever picked them: the name and the
/// bitmap, or why the file is not one.
pub type Pictures = Vec<(String, Result<Bitmap, String>)>;

/// Decodes picture files (name and bytes). Slow for big pictures, so the caller runs it where the
/// files were read, not on the UI thread.
pub fn decode_stop_motion_pictures(files: Vec<(String, Vec<u8>)>) -> Pictures {
    files
        .into_iter()
        .map(|(name, bytes)| {
            let picture = flare_import::import_bitmap(&name, &bytes).map_err(|e| e.to_string());
            (name, picture)
        })
        .collect()
}

impl Workspace {
    fn stop_motion_failed(&mut self, error: impl std::fmt::Display) {
        self.report(tr!(
            self,
            "stop-motion-cannot",
            &[("error", &error.to_string())]
        ));
    }

    fn stop_motion_apply(
        &mut self,
        commands: Result<Vec<flare_commands::Command>, library::StopMotionLibraryError>,
    ) -> bool {
        match commands {
            Ok(commands) => match self.active_mut().apply_labeled(commands, LABEL) {
                Ok(()) => true,
                Err(error) => {
                    self.stop_motion_failed(error);
                    false
                }
            },
            Err(error) => {
                self.stop_motion_failed(error);
                false
            }
        }
    }

    pub(crate) fn stop_motion_start(&mut self, fps: u32, hold: u32) {
        let command = library::start(self.document(), fps, hold).map(|c| vec![c]);
        self.stop_motion_apply(command);
    }

    pub(crate) fn stop_motion_end(&mut self) {
        let command = library::end(self.document()).map(|c| vec![c]);
        self.stop_motion_apply(command);
    }

    pub(crate) fn stop_motion_set_hold(&mut self, index: usize, hold: u32) {
        let command = library::set_hold(self.document(), index, hold).map(|c| vec![c]);
        self.stop_motion_apply(command);
    }

    pub(crate) fn stop_motion_remove(&mut self, index: usize) {
        let command = library::remove_frame(self.document(), index).map(|c| vec![c]);
        self.stop_motion_apply(command);
    }

    pub(crate) fn stop_motion_set_rate(&mut self, fps: u32, hold: u32) {
        let command = library::set_rate(self.document(), fps, hold).map(|c| vec![c]);
        self.stop_motion_apply(command);
    }

    /// Lays the frames on a new layer of the scene being edited.
    pub(crate) fn stop_motion_lay(&mut self) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let name = tr!(self, "stop-motion-layer");
        match library::lay_on_timeline(self.document(), scene, &name) {
            Ok((_, commands)) => {
                self.stop_motion_apply(Ok(commands));
            }
            Err(error) => self.stop_motion_failed(error),
        }
    }

    /// Adds each decoded picture as the next frame, each one undo step; a file that is not a
    /// picture is named and the others still go in.
    pub fn add_stop_motion_pictures(&mut self, pictures: Pictures) {
        for (name, picture) in pictures {
            match picture {
                Ok(bitmap) => {
                    let commands = library::add_frame(self.document(), bitmap, None);
                    if !self.stop_motion_apply(commands) {
                        return;
                    }
                }
                Err(error) => self.stop_motion_failed(format!("{name}: {error}")),
            }
        }
    }

    pub(crate) fn stop_motion_panel(&mut self, ui: &mut Ui) {
        let Some(session) = self.document().stop_motion().cloned() else {
            chrome::empty_state(ui, "panel-stop-motion", &tr!(self, "stop-motion-empty"));
            self.stop_motion_rate_row(ui);
            if ui.button(tr!(self, "stop-motion-start")).clicked() {
                let (fps, hold) = (self.stop_motion.fps, self.stop_motion.hold);
                self.stop_motion_start(fps, hold);
            }
            return;
        };
        ui.label(tr!(
            self,
            "stop-motion-summary",
            &[
                ("frames", &session.frames().len().to_string()),
                ("duration", &session.duration().to_string()),
                ("fps", &session.fps().to_string()),
            ]
        ));
        self.stop_motion_rate_row(ui);
        if ui.button(tr!(self, "stop-motion-rate")).clicked() {
            let (fps, hold) = (self.stop_motion.fps, self.stop_motion.hold);
            self.stop_motion_set_rate(fps, hold);
        }
        ui.separator();
        for (index, frame) in session.frames().iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{index}"));
                let mut hold = frame.hold();
                if ui
                    .add(egui::DragValue::new(&mut hold).range(1..=MAX_STOP_MOTION_HOLD))
                    .changed()
                {
                    self.stop_motion_set_hold(index, hold);
                }
                if ui.small_button(tr!(self, "stop-motion-remove")).clicked() {
                    self.stop_motion_remove(index);
                }
            });
        }
        ui.horizontal(|ui| {
            if ui.button(tr!(self, "stop-motion-add")).clicked() {
                self.dialog = Some(DialogRequest::AddStopMotionFrames);
            }
            if ui.button(tr!(self, "stop-motion-lay")).clicked() {
                self.stop_motion_lay();
            }
            if ui.button(tr!(self, "stop-motion-end")).clicked() {
                self.stop_motion_end();
            }
        });
        ui.add_space(6.0);
        ui.label(egui::RichText::new(tr!(self, "stop-motion-note")).color(theme::text_dim()));
    }

    fn stop_motion_rate_row(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(tr!(self, "stop-motion-fps"));
            ui.add(egui::DragValue::new(&mut self.stop_motion.fps).range(1..=MAX_STOP_MOTION_FPS));
            ui.label(tr!(self, "stop-motion-hold"));
            ui.add(
                egui::DragValue::new(&mut self.stop_motion.hold).range(1..=MAX_STOP_MOTION_HOLD),
            );
        });
    }
}
