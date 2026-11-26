//! The Game panel: the document's game description (gravity, entities and the components each
//! has) and a run of it. Every edit is `flare_tools::game`'s, the same `flare game` and the MCP
//! server make, so each is one undo step and the library refuses what is not valid.

use egui::Ui;
use flare_game::{ComponentKind, EntityId, parse_fields};
use flare_tools::game as model;
use flare_ui::{chrome, theme};

use crate::workspace::Workspace;

/// What the panel remembers while it is open.
pub(crate) struct GamePanel {
    pub(crate) selected: Option<EntityId>,
    pub(crate) new_name: String,
    pub(crate) kind: ComponentKind,
    pub(crate) fields: String,
    pub(crate) frames: u32,
    /// Input for the run, one frame per `;`-separated part.
    pub(crate) script: String,
    /// Gravity as edited, applied when the drag or typing is done.
    pub(crate) gravity: Option<(f64, f64)>,
    /// What the last run said.
    pub(crate) last_run: Option<String>,
}

impl Default for GamePanel {
    fn default() -> Self {
        Self {
            selected: None,
            new_name: String::new(),
            kind: ComponentKind::ALL[0],
            fields: String::new(),
            frames: 60,
            script: String::new(),
            gravity: None,
            last_run: None,
        }
    }
}

/// The most frames one run in the panel steps, so a click cannot hold the window.
pub(crate) const MAX_RUN_FRAMES: u32 = 600;

impl Workspace {
    fn game_failed(&mut self, error: impl std::fmt::Display) {
        self.report(tr!(self, "game-cannot", &[("error", &error.to_string())]));
    }

    fn game_edit(&mut self, commands: Vec<flare_commands::Command>) -> bool {
        match self.active_mut().apply_labeled(commands, "Edit the game") {
            Ok(()) => true,
            Err(error) => {
                self.game_failed(error);
                false
            }
        }
    }

    /// Adds an entity called `name` to the game and chooses it.
    pub(crate) fn game_add_entity(&mut self, name: &str) {
        match model::add_entity(self.document(), name.trim(), None) {
            Ok((id, commands)) => {
                if self.game_edit(commands) {
                    self.game.selected = Some(id);
                }
            }
            Err(error) => self.game_failed(error),
        }
    }

    /// Sets the component `kind` of the chosen entity from `name=value` words.
    pub(crate) fn game_set_component(&mut self, kind: ComponentKind, fields: &str) {
        let Some(entity) = self.game.selected else {
            return;
        };
        let tokens: Vec<&str> = fields.split_whitespace().collect();
        match parse_fields(&tokens)
            .and_then(|fields| model::set_component(entity, kind.name(), &fields))
        {
            Ok(commands) => {
                self.game_edit(commands);
            }
            Err(error) => self.game_failed(error),
        }
    }

    pub(crate) fn game_remove_component(&mut self, kind: ComponentKind) {
        let Some(entity) = self.game.selected else {
            return;
        };
        match model::remove_component(entity, kind.name()) {
            Ok(commands) => {
                self.game_edit(commands);
            }
            Err(error) => self.game_failed(error),
        }
    }

    pub(crate) fn game_remove_entity(&mut self) {
        if let Some(entity) = self.game.selected.take() {
            self.game_edit(model::remove_entity(entity));
        }
    }

    pub(crate) fn game_set_gravity(&mut self, x: f64, y: f64) {
        self.game_edit(model::set_gravity(x, y));
    }

    /// Steps the game for the panel's frames at the document's rate under the panel's script and
    /// keeps what it says.
    pub(crate) fn game_run(&mut self) {
        let fps = self.document().frame_rate().per_second();
        let frames = self.game.frames.clamp(1, MAX_RUN_FRAMES);
        let script = self.game.script.trim();
        let script = (!script.is_empty()).then_some(script);
        match model::run(self.document(), fps, frames, script) {
            Ok(playback) => {
                let (n, events, digest) = (
                    playback.frames.to_string(),
                    playback.events.len().to_string(),
                    format!("{:016x}", playback.digest),
                );
                self.game.last_run = Some(tr!(
                    self,
                    "game-ran",
                    &[("frames", &n), ("events", &events), ("digest", &digest)]
                ));
            }
            Err(error) => self.game_failed(error),
        }
    }

    pub(crate) fn game_panel(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| self.game_contents(ui));
    }

    fn game_contents(&mut self, ui: &mut Ui) {
        let entities: Vec<(EntityId, String, Vec<ComponentKind>)> = self
            .document()
            .game()
            .map(|g| {
                g.entities()
                    .map(|e| {
                        (
                            e.id(),
                            e.name().to_owned(),
                            e.components().map(|c| c.kind()).collect(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let gravity = self.document().game().map_or((0.0, 0.0), |g| g.gravity());
        if self
            .game
            .selected
            .is_some_and(|id| !entities.iter().any(|(e, _, _)| *e == id))
        {
            self.game.selected = None;
        }
        ui.label(tr!(self, "game-gravity"));
        let (mut x, mut y) = self.game.gravity.unwrap_or(gravity);
        let (mut done, mut edited) = (false, false);
        ui.horizontal(|ui| {
            for value in [&mut x, &mut y] {
                let response = ui.add(egui::DragValue::new(value).speed(1.0));
                edited |= response.changed();
                done |= response.drag_stopped() || response.lost_focus();
            }
        });
        if edited {
            self.game.gravity = Some((x, y));
        }
        if done
            && let Some((x, y)) = self.game.gravity.take()
            && (x, y) != gravity
        {
            self.game_set_gravity(x, y);
        }
        ui.separator();
        let name_hint = tr!(self, "game-entity-name");
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.game.new_name)
                    .hint_text(name_hint)
                    .desired_width(120.0),
            );
            let ready = !self.game.new_name.trim().is_empty();
            if ui
                .add_enabled(ready, egui::Button::new(tr!(self, "game-add-entity")))
                .clicked()
            {
                let name = std::mem::take(&mut self.game.new_name);
                self.game_add_entity(&name);
            }
        });
        if entities.is_empty() {
            chrome::empty_state(ui, "panel-game", &tr!(self, "game-empty"));
            return;
        }
        for (id, name, kinds) in &entities {
            let selected = self.game.selected == Some(*id);
            let kinds: Vec<&str> = kinds.iter().map(|k| k.name()).collect();
            let label = format!("{name} (#{id})  {}", kinds.join(", "));
            if ui.selectable_label(selected, label).clicked() {
                self.game.selected = Some(*id);
            }
        }
        if self.game.selected.is_some() {
            ui.separator();
            self.game_component_form(ui);
        }
        ui.separator();
        ui.label(tr!(self, "game-script-label"));
        let script_hint = tr!(self, "game-script");
        ui.add(
            egui::TextEdit::singleline(&mut self.game.script)
                .hint_text(script_hint)
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            ui.label(tr!(self, "game-frames"));
            ui.add(egui::DragValue::new(&mut self.game.frames).range(1..=MAX_RUN_FRAMES));
            if ui.button(tr!(self, "game-run")).clicked() {
                self.game_run();
            }
        });
        if let Some(text) = &self.game.last_run {
            ui.label(text);
        }
        ui.add_space(6.0);
        ui.label(egui::RichText::new(tr!(self, "game-note")).color(theme::text_dim()));
    }

    fn game_component_form(&mut self, ui: &mut Ui) {
        let shown = self.tr("game-kind");
        egui::ComboBox::from_label(shown)
            .selected_text(self.game.kind.name())
            .show_ui(ui, |ui| {
                for kind in ComponentKind::ALL {
                    ui.selectable_value(&mut self.game.kind, kind, kind.name());
                }
            });
        let fields_hint = tr!(self, "game-fields");
        ui.add(
            egui::TextEdit::singleline(&mut self.game.fields)
                .hint_text(fields_hint)
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            if ui.button(tr!(self, "game-set-component")).clicked() {
                let (kind, fields) = (self.game.kind, self.game.fields.clone());
                self.game_set_component(kind, &fields);
            }
            if ui.button(tr!(self, "game-remove-component")).clicked() {
                self.game_remove_component(self.game.kind);
            }
            if ui.button(tr!(self, "game-remove-entity")).clicked() {
                self.game_remove_entity();
            }
        });
    }
}
