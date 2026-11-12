//! Finding a command in the shortcut list by what a person types: words of its name, its id, or
//! its keys. Every surface searches the same way; the editor gives the name in the person's
//! language, the command line and the MCP server give the English one.

use super::{Accelerator, ShortcutMap};
use crate::{LabelState, MenuAction};

/// The longest query read, in characters; the rest is ignored.
pub const MAX_SEARCH_CHARS: usize = 100;

/// The modifiers a query can name, by any of the names people use for them.
const COMMAND: &[&str] = &["cmd", "command", "ctrl", "control", "cmdorctrl", "\u{2318}"];
const ALT: &[&str] = &["alt", "option", "opt", "\u{2325}"];
const SHIFT: &[&str] = &["shift", "\u{21e7}"];

/// A query read once: either keys (`Cmd+S`, `⌘⇧S`, `shift+f5`), or words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShortcutQuery {
    /// Nothing typed: everything matches.
    All,
    /// Words, each of which must be found.
    Words(Vec<String>),
    /// A key chord: these modifiers and, if one was typed, this key.
    Keys {
        command: bool,
        alt: bool,
        shift: bool,
        key: Option<String>,
    },
}

fn modifier(word: &str) -> Option<usize> {
    [COMMAND, ALT, SHIFT]
        .iter()
        .position(|names| names.contains(&word))
}

impl ShortcutQuery {
    /// Reads what was typed. A `+` or a modifier sign makes it keys; anything else is words.
    pub fn parse(text: &str) -> Self {
        let text: String = text.chars().take(MAX_SEARCH_CHARS).collect();
        let text = text.trim().to_lowercase();
        if text.is_empty() {
            return Self::All;
        }
        let signs = ['\u{2318}', '\u{2325}', '\u{21e7}'];
        if !text.contains('+') && !text.contains(signs) {
            return Self::Words(text.split_whitespace().map(str::to_owned).collect());
        }
        // Each sign stands alone, so `⌘⇧s` reads as `⌘ ⇧ s`.
        let mut spaced = String::new();
        for c in text.chars() {
            if signs.contains(&c) {
                spaced.push(' ');
                spaced.push(c);
                spaced.push(' ');
            } else if c == '+' {
                spaced.push(' ');
            } else {
                spaced.push(c);
            }
        }
        let mut flags = [false; 3];
        let mut key = None;
        for part in spaced.split_whitespace() {
            match modifier(part) {
                Some(i) => flags[i] = true,
                // A second key is not a chord any shortcut can be; it then matches nothing.
                None if key.is_some() => return Self::Words(vec![text.clone()]),
                None => key = Some(part.to_owned()),
            }
        }
        Self::Keys {
            command: flags[0],
            alt: flags[1],
            shift: flags[2],
            key,
        }
    }

    /// Whether a command with this name, id and keys answers the query.
    pub fn matches(&self, name: &str, id: &str, keys: Option<&Accelerator>) -> bool {
        match self {
            Self::All => true,
            Self::Words(words) => {
                let (name, id) = (name.to_lowercase(), id.to_lowercase());
                words.iter().all(|word| {
                    name.contains(word.as_str())
                        || id.contains(word.as_str())
                        || keys.is_some_and(|a| key_word(a, word))
                })
            }
            Self::Keys {
                command,
                alt,
                shift,
                key,
            } => keys.is_some_and(|a| {
                let same =
                    a.has_command() == *command && a.has_alt() == *alt && a.has_shift() == *shift;
                match key {
                    Some(key) => same && a.key().eq_ignore_ascii_case(key),
                    // Only modifiers typed (`cmd+`): every chord that holds them.
                    None => {
                        (!command || a.has_command())
                            && (!alt || a.has_alt())
                            && (!shift || a.has_shift())
                    }
                }
            }),
        }
    }
}

/// A word that names a command's keys: its key itself (`f5`, `s`) or one of its modifiers.
fn key_word(accel: &Accelerator, word: &str) -> bool {
    if accel.key().eq_ignore_ascii_case(word) {
        return true;
    }
    match modifier(word) {
        Some(0) => accel.has_command(),
        Some(1) => accel.has_alt(),
        Some(2) => accel.has_shift(),
        _ => false,
    }
}

impl ShortcutMap {
    /// Every command that answers `query`, whether it has keys or not, in the menu's order: what
    /// the command palette lists. `name` gives each one's name in the person's language.
    pub fn find_commands(
        &self,
        query: &str,
        name: impl Fn(MenuAction) -> String,
    ) -> Vec<MenuAction> {
        let query = ShortcutQuery::parse(query);
        MenuAction::ALL
            .into_iter()
            .filter(|action| query.matches(&name(*action), &action.id(), self.get(*action)))
            .collect()
    }

    /// The commands the shortcut list shows (those with keys now, or keys of their own in a set,
    /// or a tool letter) that answer `query`, with `name` giving each one's name in the
    /// person's language, in the menu's order.
    pub fn search(&self, query: &str, name: impl Fn(MenuAction) -> String) -> Vec<MenuAction> {
        let query = ShortcutQuery::parse(query);
        MenuAction::ALL
            .into_iter()
            .filter(|action| self.listed(*action))
            .filter(|action| query.matches(&name(*action), &action.id(), self.get(*action)))
            .collect()
    }

    /// Whether the shortcut list has a row for `action`.
    pub fn listed(&self, action: MenuAction) -> bool {
        self.get(action).is_some() || action.shortcut().is_some() || action.tool_key().is_some()
    }

    /// [`Self::list_json`] narrowed to what answers `query`, searched by English name.
    pub fn search_json(&self, query: &str) -> serde_json::Value {
        let state = LabelState::default();
        let found = self.search(query, |action| action.label(&state));
        let mut out = self.list_json();
        let shortcuts: Vec<_> = found
            .into_iter()
            .map(|action| {
                serde_json::json!({
                    "action": action.id(),
                    "name": action.label(&state),
                    "shortcut": self.get(action).map(ToString::to_string),
                })
            })
            .collect();
        out["search"] = query
            .chars()
            .take(MAX_SEARCH_CHARS)
            .collect::<String>()
            .into();
        out["shortcuts"] = shortcuts.into();
        out
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
#[path = "search_tests.rs"]
mod tests;
