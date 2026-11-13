//! The shortcut registry: which key runs which [`MenuAction`], in named sets a person starts from
//! and customises. The default tables are data in `sets.rs`; [`MenuAction::shortcut`] stays the
//! built-in Flare table that native menus register, and a [`ShortcutMap`] is what a person's
//! edits produce. Nothing here knows a window, so every surface reads the same rules.

mod accel;
mod animate_xml;
mod file;
mod search;
mod sets;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

pub use accel::{Accelerator, ShortcutError};
pub use animate_xml::{AnimateImport, MAX_ANIMATE_SHORTCUT_BYTES, Skipped};
pub use file::MAX_SHORTCUT_FILE_BYTES;
pub use search::{MAX_SEARCH_CHARS, ShortcutQuery};
pub use sets::{DEFAULT_SET, SHORTCUT_SET_IDS, set_bindings};

use crate::MenuAction;

/// Two actions that share one accelerator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    pub accelerator: Accelerator,
    pub actions: Vec<MenuAction>,
}

/// The bindings in force: a base set plus the person's changes. Bindings are only ever changed
/// through [`bind`](Self::bind), [`unbind`](Self::unbind) and [`reset`](Self::reset), each of which
/// leaves the map unchanged when it refuses, so a map never holds a conflict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShortcutMap {
    set: &'static str,
    bindings: BTreeMap<String, Accelerator>,
    /// Actions the person freed on purpose: they stay free when the set's own key is wanted.
    unbound: Vec<String>,
}

impl Default for ShortcutMap {
    fn default() -> Self {
        Self::from_set(DEFAULT_SET).unwrap_or_else(|_| Self::empty(DEFAULT_SET))
    }
}

impl ShortcutMap {
    fn empty(set: &'static str) -> Self {
        Self {
            set,
            bindings: BTreeMap::new(),
            unbound: Vec::new(),
        }
    }

    /// The map of a named set, with no edits.
    pub fn from_set(id: &str) -> Result<Self, ShortcutError> {
        let set = SHORTCUT_SET_IDS
            .iter()
            .find(|known| **known == id)
            .ok_or_else(|| ShortcutError::UnknownSet(id.to_owned()))?;
        let mut map = Self::empty(set);
        for (action, text) in set_bindings(set) {
            let accel = Accelerator::parse(text, action)?;
            if let Some(other) = map.owner(&accel) {
                return Err(ShortcutError::Conflict {
                    accelerator: accel.to_string(),
                    with: other.id(),
                });
            }
            map.bindings.insert(action.id(), accel);
        }
        Ok(map)
    }

    /// The set this map started from.
    pub fn set(&self) -> &'static str {
        self.set
    }

    /// The accelerator of an action.
    pub fn get(&self, action: MenuAction) -> Option<&Accelerator> {
        self.bindings.get(&action.id())
    }

    /// The action an accelerator runs.
    pub fn owner(&self, accel: &Accelerator) -> Option<MenuAction> {
        self.bindings
            .iter()
            .find(|(_, bound)| *bound == accel)
            .and_then(|(id, _)| MenuAction::from_id(id))
    }

    /// The shortcut as a person reads it, as [`MenuAction::shortcut_text`] does.
    pub fn text(&self, action: MenuAction) -> Option<String> {
        self.get(action).map(Accelerator::display)
    }

    /// Every binding, in the order of the menu's own action list.
    pub fn entries(&self) -> Vec<(MenuAction, &Accelerator)> {
        MenuAction::ALL
            .into_iter()
            .filter_map(|action| self.get(action).map(|accel| (action, accel)))
            .collect()
    }

    /// Bind an action. Refused, with the map unchanged, when the text is not an accelerator, a
    /// bare letter is bound to something that is not a tool, or another action already has it.
    pub fn bind(&mut self, action: MenuAction, text: &str) -> Result<(), ShortcutError> {
        let accel = Accelerator::parse(text, action)?;
        if let Some(other) = self.owner(&accel)
            && other != action
        {
            return Err(ShortcutError::Conflict {
                accelerator: accel.to_string(),
                with: other.id(),
            });
        }
        self.unbound.retain(|id| *id != action.id());
        self.bindings.insert(action.id(), accel);
        Ok(())
    }

    /// Bind an action, taking the keys from whichever action holds them: that action is left with
    /// no shortcut, freed as [`unbind`](Self::unbind) frees one, and is returned. Refused, with the
    /// map unchanged, only when the text is not an accelerator or a bare letter is bound to
    /// something that is not a tool, so the map still never holds a conflict.
    pub fn bind_taking(
        &mut self,
        action: MenuAction,
        text: &str,
    ) -> Result<Option<MenuAction>, ShortcutError> {
        let accel = Accelerator::parse(text, action)?;
        let taken = self.owner(&accel).filter(|other| *other != action);
        if let Some(other) = taken {
            self.unbind(other);
        }
        self.unbound.retain(|id| *id != action.id());
        self.bindings.insert(action.id(), accel);
        Ok(taken)
    }

    /// Put the set's own key back on an action, taking it from whichever action holds it now, as
    /// [`bind_taking`](Self::bind_taking) does; that action is returned.
    pub fn reset_taking(
        &mut self,
        action: MenuAction,
    ) -> Result<Option<MenuAction>, ShortcutError> {
        match self.own_key(action) {
            Some(text) => self.bind_taking(action, text),
            None => {
                self.reset(action)?;
                Ok(None)
            }
        }
    }

    /// The key the set itself gives an action.
    fn own_key(&self, action: MenuAction) -> Option<&'static str> {
        set_bindings(self.set)
            .into_iter()
            .find(|(a, _)| *a == action)
            .map(|(_, text)| text)
    }

    /// Free an action's shortcut.
    pub fn unbind(&mut self, action: MenuAction) {
        if self.bindings.remove(&action.id()).is_some() {
            self.unbound.push(action.id());
        }
    }

    /// Put the set's own key back on an action; refused if another action holds that key now.
    pub fn reset(&mut self, action: MenuAction) -> Result<(), ShortcutError> {
        match self.own_key(action) {
            Some(text) => self.bind(action, text),
            None => {
                self.unbind(action);
                self.unbound.retain(|id| *id != action.id());
                Ok(())
            }
        }
    }

    /// Accelerators held by more than one action. Empty for any map built through this API; it is
    /// the check a loader and the editor run again rather than trust.
    pub fn conflicts(&self) -> Vec<Conflict> {
        let mut seen: BTreeMap<&Accelerator, Vec<MenuAction>> = BTreeMap::new();
        for (action, accel) in self.entries() {
            seen.entry(accel).or_default().push(action);
        }
        seen.into_iter()
            .filter(|(_, actions)| actions.len() > 1)
            .map(|(accel, actions)| Conflict {
                accelerator: accel.clone(),
                actions,
            })
            .collect()
    }
}
