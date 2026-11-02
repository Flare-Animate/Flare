//! The question a close of a tab with unsaved work asks: save it, close without saving, or keep
//! it open.

use crate::workspace::*;

impl Workspace {
    /// The prompt for the tab being closed, and finishing a close that waited for its save.
    pub(super) fn close_prompt_window(&mut self, ctx: &egui::Context) {
        self.finish_close_after_save();
        let Some(index) = self.close_prompt else {
            return;
        };
        if index > self.parked.len() || !self.tab_is_dirty(index) {
            self.close_prompt = None;
            return;
        }
        let untitled = tr!(self, "untitled");
        let name = self.tabs().get(index).map_or(untitled, |t| t.title.clone());
        let (text, save, discard, cancel) = (
            tr!(self, "unsaved-save-changes", &[("name", &name)]),
            tr!(self, "unsaved-save"),
            tr!(self, "unsaved-dont-save"),
            tr!(self, "start-cancel"),
        );
        let mut choice = None;
        let modal = egui::Modal::new(egui::Id::unique("unsaved-prompt")).show(ctx, |ui| {
            ui.set_max_width(320.0);
            ui.label(text);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(save).clicked() {
                    choice = Some(Choice::Save);
                }
                if ui.button(discard).clicked() {
                    choice = Some(Choice::Discard);
                }
                if ui.button(cancel).clicked() {
                    choice = Some(Choice::Keep);
                }
            });
        });
        if modal.should_close() && choice.is_none() {
            choice = Some(Choice::Keep);
        }
        let Some(choice) = choice else {
            return;
        };
        self.apply_close_choice(index, choice);
    }

    /// Does what the person chose for the tab the prompt was about.
    pub(super) fn apply_close_choice(&mut self, index: usize, choice: Choice) {
        self.close_prompt = None;
        match choice {
            Choice::Keep => {}
            Choice::Discard => self.discard_tab(index),
            Choice::Save => {
                self.switch_to_tab(index);
                // A tab that could not be brought forward (a file is still being read) stays.
                if self.position == index {
                    self.close_after_save = true;
                    self.save();
                }
            }
        }
    }

    /// Closes the document on show once the save the prompt asked for is on disk. A save that was
    /// cancelled in the file dialog, or failed, leaves the document open and unsaved.
    fn finish_close_after_save(&mut self) {
        if !self.close_after_save || self.dialog.is_some() || self.has_pending_io() {
            return;
        }
        self.close_after_save = false;
        if !self.is_dirty() {
            self.remove_tab(self.position);
        }
    }
}

pub(super) enum Choice {
    Save,
    Discard,
    Keep,
}
