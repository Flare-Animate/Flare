//! The shortcut registry: which keys run which [`MenuAction`], in named sets a person starts from
//! and customises. The default tables are data in `sets.rs`; [`MenuAction::shortcut`] stays the
//! built-in Flare table that native menus register, and a [`ShortcutMap`] is what a person's
//! edits produce. A command may have several shortcuts (up to [`MAX_KEYS_PER_COMMAND`]): the
//! first is the one its menu item shows, and every one of them runs it. Nothing here knows a
//! window, so every surface reads the same rules.

mod accel;
mod animate_xml;
mod file;
mod keys;
mod search;
mod sets;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

pub use accel::{Accelerator, ShortcutError};
pub use animate_xml::{AnimateImport, MAX_ANIMATE_SHORTCUT_BYTES, Skipped};
pub use file::MAX_SHORTCUT_FILE_BYTES;
pub use keys::ShortcutEdit;
pub use search::{MAX_SEARCH_CHARS, ShortcutQuery};
pub use sets::{DEFAULT_SET, SHORTCUT_SET_IDS, set_bindings};

use crate::MenuAction;

/// The most shortcuts one command can have.
pub const MAX_KEYS_PER_COMMAND: usize = 4;

/// Two actions that share one accelerator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    pub accelerator: Accelerator,
    pub actions: Vec<MenuAction>,
}

/// The bindings in force: a base set plus the person's changes. Bindings are only ever changed
/// through the methods here, each of which leaves the map unchanged when it refuses, so a map
/// never holds a conflict, and never a command with no keys in `bindings` or more than
/// [`MAX_KEYS_PER_COMMAND`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShortcutMap {
    set: &'static str,
    /// Each command's keys, its first one first.
    bindings: BTreeMap<String, Vec<Accelerator>>,
    /// Actions the person freed on purpose: they stay free when the set's own key is wanted.
    unbound: BTreeSet<String>,
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
            unbound: BTreeSet::new(),
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
            map.add_key(action, text)?;
        }
        // A set is data written here, not a person's edit: nothing in it counts as freed.
        map.unbound.clear();
        Ok(map)
    }

    /// The set this map started from.
    pub fn set(&self) -> &'static str {
        self.set
    }

    /// The first accelerator of an action: the one its menu item shows.
    pub fn get(&self, action: MenuAction) -> Option<&Accelerator> {
        self.keys(action).first()
    }

    /// Every accelerator of an action, its first one first.
    pub fn keys(&self, action: MenuAction) -> &[Accelerator] {
        self.bindings
            .get(&action.id())
            .map_or(&[], |keys| keys.as_slice())
    }

    /// The action an accelerator runs.
    pub fn owner(&self, accel: &Accelerator) -> Option<MenuAction> {
        self.bindings
            .iter()
            .find(|(_, keys)| keys.contains(accel))
            .and_then(|(id, _)| MenuAction::from_id(id))
    }

    /// The first shortcut as a person reads it, as [`MenuAction::shortcut_text`] does: what a
    /// menu item shows beside its name.
    pub fn text(&self, action: MenuAction) -> Option<String> {
        self.get(action).map(Accelerator::display)
    }

    /// Every shortcut of an action as a person reads them, in order, between commas.
    pub fn all_text(&self, action: MenuAction) -> Option<String> {
        let keys = self.keys(action);
        (!keys.is_empty()).then(|| {
            keys.iter()
                .map(Accelerator::display)
                .collect::<Vec<_>>()
                .join(", ")
        })
    }

    /// Every binding, one row for each key, in the order of the menu's own action list: what a
    /// surface reads to know which keys run what.
    pub fn entries(&self) -> Vec<(MenuAction, &Accelerator)> {
        MenuAction::ALL
            .into_iter()
            .flat_map(|action| self.keys(action).iter().map(move |accel| (action, accel)))
            .collect()
    }

    /// The keys the set itself gives an action.
    fn own_keys(&self, action: MenuAction) -> Vec<&'static str> {
        set_bindings(self.set)
            .into_iter()
            .filter(|(a, _)| *a == action)
            .map(|(_, text)| text)
            .collect()
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
