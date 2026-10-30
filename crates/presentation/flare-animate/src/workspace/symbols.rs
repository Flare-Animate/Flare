//! In-place symbol editing and the library.

use flare_commands::Command;
use flare_core::SymbolId;
use flare_document::Content;
use flare_history::EditorSession;
use flare_tools::ops::{self};
use glamx::DVec2;

use super::*;

impl Workspace {
    // ---- in-place symbol editing -----------------------------------------------------

    pub fn in_symbol(&self) -> bool {
        !self.contexts.is_empty()
    }

    /// Names from the scene down to the symbol being edited, for a breadcrumb.
    pub fn breadcrumb(&self) -> Vec<String> {
        let scene = self
            .session
            .document()
            .scenes()
            .first()
            .map(|s| s.name().to_owned());
        scene
            .into_iter()
            .chain(self.contexts.iter().map(|c| c.name.clone()))
            .collect()
    }

    pub(super) fn reset_view(&mut self) {
        self.cancel_drag();
        self.selected = None;
        self.also_selected.clear();
        self.current_layer = None;
        self.frame = 0;
        self.texture = None;
        self.frame_cache.clear();
        self.context_serial += 1;
    }

    /// Starts editing a symbol's own timeline in place. Returns whether it started.
    pub fn enter_symbol(&mut self, symbol: SymbolId) -> bool {
        let Some((scratch, _)) = flare_editor::symbol_workspace(self.document(), symbol) else {
            self.report("That symbol no longer exists");
            return false;
        };
        let name = self
            .document()
            .symbol(symbol)
            .map_or_else(String::new, |s| s.name().to_owned());
        self.contexts.push(Context {
            symbol,
            name,
            session: EditorSession::new(scratch),
        });
        self.reset_view();
        self.status = None;
        true
    }

    /// The symbol behind the selected instance, if an instance is selected.
    pub(super) fn selected_instance_symbol(&self) -> Option<SymbolId> {
        let (selected, scene) = (self.selected?, self.scene_id()?);
        match ops::find_span(self.document(), scene, selected)?.content() {
            Content::Instance(instance) => Some(instance.symbol()),
            _ => None,
        }
    }

    pub fn can_edit_selected_symbol(&self) -> bool {
        self.selected_instance_symbol().is_some()
    }

    pub fn edit_selected_symbol(&mut self) {
        match self.selected_instance_symbol() {
            Some(symbol) => {
                self.enter_symbol(symbol);
            }
            None => self.report("Select a symbol instance to edit it"),
        }
    }

    /// Leaves the innermost symbol, applying its edits to the parent as one undo step.
    /// Symbols created inside it (Convert to Symbol) join the library in the same step.
    /// If the parent rejects the result (for example a symbol that contains itself) the
    /// edit stays open and the reason is shown.
    pub fn exit_symbol(&mut self) -> bool {
        let Some(context) = self.contexts.pop() else {
            return false;
        };
        let inner = context.session.document();
        let timeline = inner
            .scenes()
            .first()
            .map(|s| s.timeline().clone())
            .unwrap_or_default();
        let parent = self.active().document();
        let new_symbols: Vec<flare_document::Symbol> = inner
            .symbols()
            .iter()
            .filter(|s| parent.symbol(s.id()).is_none())
            .cloned()
            .collect();
        let changed = parent
            .symbol(context.symbol)
            .is_none_or(|s| s.timeline() != &timeline);
        if changed || !new_symbols.is_empty() {
            let merge = Command::MergeSymbolEdit {
                symbol: context.symbol,
                timeline,
                new_symbols,
            };
            if let Err(error) = self.active_mut().apply_labeled([merge], "Edit symbol") {
                self.contexts.push(context);
                self.report(format!("Cannot leave the symbol: {error}"));
                return false;
            }
        }
        self.reset_view();
        self.status = None;
        true
    }

    /// Leaves symbols until only `depth` remain (0 = back to the scene).
    pub fn exit_to_depth(&mut self, depth: usize) -> bool {
        while self.contexts.len() > depth {
            if !self.exit_symbol() {
                return false;
            }
        }
        true
    }

    pub(super) fn exit_all_symbols(&mut self) -> bool {
        self.exit_to_depth(0)
    }

    /// Adds an instance of the library-selected symbol, centered on the stage.
    /// Adds an empty symbol to the library and selects it. Library edits belong to the main
    /// document, so they wait until symbol editing is left.
    pub fn new_symbol(&mut self) {
        if self.in_symbol() {
            self.report("Leave symbol editing to change the library");
            return;
        }
        let taken: std::collections::HashSet<String> = self
            .document()
            .symbols()
            .iter()
            .map(|s| s.name().to_owned())
            .collect();
        let name = (1..)
            .map(|n| format!("Symbol {n}"))
            .find(|name| !taken.contains(name))
            .unwrap_or_default();
        let id = SymbolId::new();
        let symbol = flare_document::Symbol::new(id, name, flare_document::Timeline::default())
            .with_kind(self.new_symbol_kind);
        self.run_batch(vec![Command::AddSymbol(symbol)], "add the symbol");
        if self.status.is_none() {
            self.selected_symbol = Some(id);
        }
    }

    /// Makes a library symbol another kind, as one undo step.
    pub fn set_symbol_kind(&mut self, id: SymbolId, kind: flare_document::SymbolKind) {
        if self.in_symbol() {
            self.report("Leave symbol editing to change the library");
            return;
        }
        if self.document().symbol(id).is_none_or(|s| s.kind() == kind) {
            return;
        }
        self.run_batch(
            vec![Command::SetSymbolKind { symbol: id, kind }],
            "change the symbol's type",
        );
    }

    /// Renames a library symbol; a blank name is ignored.
    pub fn rename_symbol(&mut self, id: SymbolId, name: &str) {
        let name = name.trim();
        if name.is_empty() || self.in_symbol() {
            return;
        }
        if self.document().symbol(id).is_some_and(|s| s.name() == name) {
            return;
        }
        let command = Command::RenameSymbol {
            symbol: id,
            name: name.to_owned(),
        };
        self.run_batch(vec![command], "rename the symbol");
    }

    /// Deletes a library symbol; the edit is refused while an instance still uses it.
    pub fn delete_symbol(&mut self, id: SymbolId) {
        if self.in_symbol() {
            self.report("Leave symbol editing to change the library");
            return;
        }
        self.run_batch(vec![Command::RemoveSymbol(id)], "delete the symbol");
        if self.selected_symbol == Some(id) {
            self.selected_symbol = None;
        }
    }

    pub fn place_selected_symbol(&mut self) {
        let (Some(symbol), Some(scene)) = (self.selected_symbol, self.scene_id()) else {
            self.report("Select a symbol in the library first");
            return;
        };
        if self.contexts.iter().any(|c| c.symbol == symbol) {
            self.report("A symbol cannot contain itself");
            return;
        }
        let stage = self.document().stage();
        let center = DVec2::new(
            f64::from(stage.width()) / 2.0,
            f64::from(stage.height()) / 2.0,
        );
        if let Some(commands) = ops::place_instance(self.document(), scene, symbol, center) {
            self.run_batch(commands, "place instance");
        }
    }
}
