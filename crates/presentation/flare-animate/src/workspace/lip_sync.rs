//! Lip Syncing in the Object tab, for a selected Graphic that holds mouth drawings: which sound
//! to follow, how to read it, which of the symbol's frames shows each mouth shape, and a button
//! that keys the mouth to the voice. The reading runs on the sound worker
//! (`flare_runtime::AudioJob::LipSync`); its commands are applied as one undo step when they come
//! back, if the document is still the one they were made for.

use std::collections::HashMap;
use std::sync::Arc;

use flare_core::{SoundId, SymbolId};
use flare_document::Content;
use flare_runtime::{Analysis, AudioJob, LipSyncMade, LipSyncRequest};
use flare_tools::lipsync::{MouthMap, Viseme};

use super::*;

/// What the Lip Syncing section remembers while the program runs.
#[derive(Default)]
pub(super) struct LipSyncPanel {
    /// The mapping of each mouth symbol, so syncing again needs no setting up.
    maps: HashMap<SymbolId, MouthMap>,
    sound: Option<SoundId>,
    analysis: Analysis,
    /// The job being read; its result is dropped when another has been asked for since.
    pending: Option<u64>,
}

/// A browser cannot run a program, so Rhubarb is offered on a desktop only.
const RHUBARB_RUNS_HERE: bool = !cfg!(target_arch = "wasm32");

impl Workspace {
    /// The sounds cued in the scene being edited, by name, each once.
    pub(super) fn lip_sync_sounds(&self) -> Vec<(SoundId, String)> {
        let Some(scene) = self.scene_id() else {
            return Vec::new();
        };
        let document = self.document();
        let mut sounds: Vec<(SoundId, String)> = Vec::new();
        for scene in document.scenes().iter().filter(|s| s.id() == scene) {
            for (_, cue) in scene.timeline().cues() {
                if sounds.iter().all(|(id, _)| *id != cue.sound)
                    && let Some(sound) = document.sound(cue.sound)
                {
                    sounds.push((cue.sound, sound.name().to_owned()));
                }
            }
        }
        sounds
    }

    /// The mouth symbol of the selection: the Graphic its one instance shows.
    fn lip_sync_symbol(&self) -> Option<SymbolId> {
        self.selected_playback()?;
        let (selected, scene) = (self.selected?, self.scene_id()?);
        match ops::find_element(self.document(), scene, selected)? {
            Content::Instance(instance) => Some(instance.symbol()),
            _ => None,
        }
    }

    /// The mapping used for the selected mouth: the one set for it, or the chart's order.
    pub(super) fn lip_sync_map(&self) -> MouthMap {
        self.lip_sync_symbol()
            .and_then(|symbol| self.lip_sync.maps.get(&symbol).copied())
            .unwrap_or_default()
    }

    pub(super) fn set_lip_sync_map(&mut self, map: MouthMap) {
        if let Some(symbol) = self.lip_sync_symbol() {
            self.lip_sync.maps.insert(symbol, map);
        }
    }

    pub(super) fn set_lip_sync_analysis(&mut self, analysis: Analysis) {
        self.lip_sync.analysis = analysis;
    }

    pub(super) fn lip_sync_pending(&self) -> bool {
        self.lip_sync.pending.is_some()
    }

    /// Hands the selected mouth and the chosen sound to the sound worker to be read and keyed.
    pub(super) fn start_lip_sync(&mut self) {
        let sounds = self.lip_sync_sounds();
        let sound = self
            .lip_sync
            .sound
            .filter(|s| sounds.iter().any(|(id, _)| id == s))
            .or_else(|| sounds.first().map(|(id, _)| *id));
        let (Some(at), Some(scene), Some(sound)) = (self.selected, self.scene_id(), sound) else {
            self.report(tr!(self, "lipsync-no-sound"));
            return;
        };
        let analysis = match self.lip_sync.analysis {
            Analysis::Rhubarb if !RHUBARB_RUNS_HERE => Analysis::BuiltIn,
            analysis => analysis,
        };
        let request = LipSyncRequest {
            scene,
            at,
            sound,
            map: self.lip_sync_map(),
            analysis,
        };
        let id = self.background.next_id();
        self.lip_sync.pending = Some(id);
        self.status = Some(tr!(self, "lipsync-working"));
        self.background.submit_audio(AudioJob::LipSync {
            id,
            document: self.active().snapshot(),
            request,
            // Found on the worker: looking along the PATH is file system work.
            rhubarb: None,
        });
    }

    pub(super) fn lip_synced(
        &mut self,
        id: u64,
        document: Arc<flare_document::Document>,
        result: Result<LipSyncMade, String>,
    ) {
        if self.lip_sync.pending.take_if(|p| *p == id).is_none() {
            return;
        }
        self.status = None;
        // The keys were made for that document; an edit since would make them wrong.
        if !Arc::ptr_eq(&document, &self.active().snapshot()) {
            self.report(tr!(self, "lipsync-changed"));
            return;
        }
        let made = match result {
            Ok(made) => made,
            Err(reason) => {
                self.report(tr!(self, "lipsync-failed", &[("reason", &reason)]));
                return;
            }
        };
        if let Some(reason) = &made.fell_back {
            self.report(tr!(self, "lipsync-fell-back", &[("reason", reason)]));
        }
        let keys = made.keyframes;
        self.run_batch(made.commands, "lip sync");
        let done = match made.used {
            Analysis::Rhubarb => tr_n!(self, "lipsync-done-rhubarb", keys),
            _ => tr_n!(self, "lipsync-done-built-in", keys),
        };
        self.report(done);
    }

    /// The Lip Syncing section of the Object tab, under Looping, for a selected Graphic.
    pub(super) fn lip_sync_section(&mut self, ui: &mut egui::Ui) {
        let Some((_, frames)) = self.selected_playback() else {
            return;
        };
        flare_ui::chrome::section(ui, "waveform", &tr!(self, "lipsync-title"));
        let sounds = self.lip_sync_sounds();
        if sounds.is_empty() {
            ui.weak(tr!(self, "lipsync-no-sound"));
            return;
        }
        let mut sound = self
            .lip_sync
            .sound
            .filter(|s| sounds.iter().any(|(id, _)| id == s))
            .unwrap_or(sounds[0].0);
        let name_of = |id: SoundId| {
            sounds
                .iter()
                .find(|(s, _)| *s == id)
                .map(|(_, n)| n.clone())
                .unwrap_or_default()
        };
        flare_ui::chrome::row(ui, &tr!(self, "lipsync-sound"), |ui| {
            egui::ComboBox::from_id_salt("lip_sync_sound")
                .selected_text(name_of(sound))
                .show_ui(ui, |ui| {
                    for (id, name) in &sounds {
                        ui.selectable_value(&mut sound, *id, name);
                    }
                });
        });
        self.lip_sync.sound = Some(sound);
        self.analysis_row(ui);
        let mut map = self.lip_sync_map();
        ui.label(tr!(self, "lipsync-shapes"))
            .on_hover_text(tr!(self, "lipsync-shapes-tip"));
        let label_of = |ws: &Self, v: Viseme| match v {
            Viseme::Neutral => tr!(ws, "lipsync-neutral"),
            Viseme::Etc => tr!(ws, "lipsync-etc"),
            other => other.name().to_uppercase(),
        };
        egui::Grid::new("lip_sync_map")
            .num_columns(4)
            .show(ui, |ui| {
                for (index, (viseme, frame)) in
                    map.pairs().collect::<Vec<_>>().into_iter().enumerate()
                {
                    ui.label(label_of(self, viseme));
                    // Counted from 1 on screen, as the timeline counts.
                    let mut shown = frame.min(frames - 1) + 1;
                    if ui
                        .add(egui::DragValue::new(&mut shown).range(1..=frames))
                        .changed()
                    {
                        map = map.with(viseme, shown - 1);
                    }
                    if index % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
        self.set_lip_sync_map(map);
        let busy = self.lip_sync_pending();
        let label = if busy {
            tr!(self, "lipsync-working")
        } else {
            tr!(self, "lipsync-sync")
        };
        if ui.add_enabled(!busy, egui::Button::new(label)).clicked() {
            self.start_lip_sync();
        }
    }

    fn analysis_row(&mut self, ui: &mut egui::Ui) {
        let mut analysis = self.lip_sync.analysis;
        let label_of = |ws: &Self, a: Analysis| match a {
            Analysis::Auto => tr!(ws, "lipsync-auto"),
            Analysis::BuiltIn => tr!(ws, "lipsync-built-in"),
            Analysis::Rhubarb => tr!(ws, "lipsync-rhubarb"),
        };
        let (auto_tip, web_tip) = (
            tr!(self, "lipsync-auto-tip"),
            tr!(self, "lipsync-rhubarb-web"),
        );
        let selected = label_of(self, analysis);
        let labels: Vec<(Analysis, String)> = Analysis::ALL
            .into_iter()
            .map(|a| (a, label_of(self, a)))
            .collect();
        flare_ui::chrome::row(ui, &tr!(self, "lipsync-analysis"), |ui| {
            egui::ComboBox::from_id_salt("lip_sync_analysis")
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    for (option, label) in labels {
                        let enabled = option != Analysis::Rhubarb || RHUBARB_RUNS_HERE;
                        let response = ui.add_enabled(
                            enabled,
                            egui::Button::selectable(analysis == option, label),
                        );
                        if response.clicked() {
                            analysis = option;
                        }
                        match option {
                            Analysis::Auto => {
                                response.on_hover_text(&auto_tip);
                            }
                            Analysis::Rhubarb if !enabled => {
                                response.on_disabled_hover_text(&web_tip);
                            }
                            _ => {}
                        }
                    }
                });
        });
        self.set_lip_sync_analysis(analysis);
    }
}
