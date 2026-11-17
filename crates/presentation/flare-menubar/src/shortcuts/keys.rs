//! Changing a [`ShortcutMap`]: giving a command its keys, adding one more, taking one off. Every
//! change either happens whole or is refused with the map as it was.

use super::{Accelerator, MAX_KEYS_PER_COMMAND, ShortcutError, ShortcutMap};
use crate::MenuAction;

impl ShortcutMap {
    /// A conflict error when `accel` runs a command other than `action`.
    fn check_free(&self, action: MenuAction, accel: &Accelerator) -> Result<(), ShortcutError> {
        match self.owner(accel) {
            Some(other) if other != action => Err(ShortcutError::Conflict {
                accelerator: accel.to_string(),
                with: other.id(),
            }),
            _ => Ok(()),
        }
    }

    /// Takes `accel` from whichever other command has it and returns that command. A command
    /// left with no keys is freed, as [`unbind`](Self::unbind) frees one.
    fn take(&mut self, action: MenuAction, accel: &Accelerator) -> Option<MenuAction> {
        let other = self.owner(accel).filter(|other| *other != action)?;
        self.remove_key(other, accel);
        Some(other)
    }

    fn set_keys(&mut self, action: MenuAction, keys: Vec<Accelerator>) {
        self.unbound.remove(&action.id());
        if keys.is_empty() {
            self.bindings.remove(&action.id());
        } else {
            self.bindings.insert(action.id(), keys);
        }
    }

    /// Gives an action this one shortcut in place of all it had. Refused, with the map unchanged,
    /// when the text is not an accelerator, a bare letter is bound to something that is not a
    /// tool, or another action already has it.
    pub fn bind(&mut self, action: MenuAction, text: &str) -> Result<(), ShortcutError> {
        let accel = Accelerator::parse(text, action)?;
        self.check_free(action, &accel)?;
        self.set_keys(action, vec![accel]);
        Ok(())
    }

    /// Like [`bind`](Self::bind), taking the keys from whichever action holds them: that action
    /// loses only those keys (and is freed if they were its last), and is returned. Refused, with
    /// the map unchanged, only when the text is not an accelerator or a bare letter is bound to
    /// something that is not a tool, so the map still never holds a conflict.
    pub fn bind_taking(
        &mut self,
        action: MenuAction,
        text: &str,
    ) -> Result<Option<MenuAction>, ShortcutError> {
        let accel = Accelerator::parse(text, action)?;
        let taken = self.take(action, &accel);
        self.set_keys(action, vec![accel]);
        Ok(taken)
    }

    /// Gives an action one more shortcut, after those it has. Refused, with the map unchanged,
    /// when the text is not an accelerator, another action has it, or the action already has
    /// [`MAX_KEYS_PER_COMMAND`]. A key the action already has changes nothing.
    pub fn add_key(&mut self, action: MenuAction, text: &str) -> Result<(), ShortcutError> {
        let accel = Accelerator::parse(text, action)?;
        self.check_free(action, &accel)?;
        self.push_key(action, accel)
    }

    /// Like [`add_key`](Self::add_key), taking the key from whichever action holds it, as
    /// [`bind_taking`](Self::bind_taking) does; that action is returned. A refusal (a bad key,
    /// or too many) takes nothing.
    pub fn add_key_taking(
        &mut self,
        action: MenuAction,
        text: &str,
    ) -> Result<Option<MenuAction>, ShortcutError> {
        let accel = Accelerator::parse(text, action)?;
        let mut keys = self.keys(action).to_vec();
        if keys.contains(&accel) {
            return Ok(None);
        }
        if keys.len() >= MAX_KEYS_PER_COMMAND {
            return Err(ShortcutError::TooManyKeys(MAX_KEYS_PER_COMMAND));
        }
        let taken = self.take(action, &accel);
        keys.push(accel);
        self.set_keys(action, keys);
        Ok(taken)
    }

    fn push_key(&mut self, action: MenuAction, accel: Accelerator) -> Result<(), ShortcutError> {
        let mut keys = self.keys(action).to_vec();
        if keys.contains(&accel) {
            return Ok(());
        }
        if keys.len() >= MAX_KEYS_PER_COMMAND {
            return Err(ShortcutError::TooManyKeys(MAX_KEYS_PER_COMMAND));
        }
        keys.push(accel);
        self.set_keys(action, keys);
        Ok(())
    }

    /// Takes one shortcut off an action; the others stay, and the next becomes the first. An
    /// action left with none is freed. False when the action did not have that key.
    pub fn remove_key(&mut self, action: MenuAction, accel: &Accelerator) -> bool {
        let mut keys = self.keys(action).to_vec();
        let before = keys.len();
        keys.retain(|k| k != accel);
        if keys.len() == before {
            return false;
        }
        if keys.is_empty() {
            self.unbind(action);
        } else {
            self.set_keys(action, keys);
        }
        true
    }

    /// Frees every shortcut of an action.
    pub fn unbind(&mut self, action: MenuAction) {
        if self.bindings.remove(&action.id()).is_some() {
            self.unbound.insert(action.id());
        }
    }

    /// Puts the set's own keys back on an action in place of the ones it has; refused, with the
    /// map unchanged, if another action holds one of those keys now.
    pub fn reset(&mut self, action: MenuAction) -> Result<(), ShortcutError> {
        let mut keys = Vec::new();
        for text in self.own_keys(action) {
            let accel = Accelerator::parse(text, action)?;
            self.check_free(action, &accel)?;
            keys.push(accel);
        }
        self.set_keys(action, keys);
        Ok(())
    }

    /// Puts the set's own keys back on an action, taking each from whichever action holds it
    /// now, as [`bind_taking`](Self::bind_taking) does; the first action they were taken from is
    /// returned.
    pub fn reset_taking(
        &mut self,
        action: MenuAction,
    ) -> Result<Option<MenuAction>, ShortcutError> {
        let mut keys = Vec::new();
        for text in self.own_keys(action) {
            keys.push(Accelerator::parse(text, action)?);
        }
        let mut first = None;
        for accel in &keys {
            let taken = self.take(action, accel);
            first = first.or(taken);
        }
        self.set_keys(action, keys);
        Ok(first)
    }
}

/// One change to a command's shortcuts, as the command line and the agent server ask for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShortcutEdit {
    /// These keys in place of all it has (taken from a command that had them).
    Set(String),
    /// These keys beside those it has (taken from a command that had them).
    Add(String),
    /// These keys off it; its others stay.
    Remove(String),
    /// No shortcut at all.
    Clear,
    /// The set's own keys again (taken from a command that had them).
    Reset,
}

impl ShortcutMap {
    /// Makes `edit` to the command named `id`; returns the command keys were taken from, if any.
    /// An unknown command, a bad key, too many keys or keys it does not have (to remove) are
    /// refused, with the map unchanged.
    pub fn edit(
        &mut self,
        id: &str,
        edit: &ShortcutEdit,
    ) -> Result<Option<MenuAction>, ShortcutError> {
        let action =
            MenuAction::from_id(id).ok_or_else(|| ShortcutError::UnknownAction(id.to_owned()))?;
        match edit {
            ShortcutEdit::Set(text) => self.bind_taking(action, text),
            ShortcutEdit::Add(text) => self.add_key_taking(action, text),
            ShortcutEdit::Remove(text) => {
                let accel = Accelerator::parse(text, action)?;
                if self.remove_key(action, &accel) {
                    Ok(None)
                } else {
                    Err(ShortcutError::NotBound {
                        accelerator: accel.to_string(),
                        action: action.id(),
                    })
                }
            }
            ShortcutEdit::Clear => {
                self.unbind(action);
                Ok(None)
            }
            ShortcutEdit::Reset => self.reset_taking(action),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
#[path = "keys_tests.rs"]
mod tests;
