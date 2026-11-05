//! One key chord, parsed and written in one canonical form.

use std::fmt;

use crate::MenuAction;

/// Why a shortcut was refused.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ShortcutError {
    #[error("`{0}` is not a shortcut (write it like `CmdOrCtrl+Shift+Z` or `F5`)")]
    BadAccelerator(String),
    #[error("a bare key such as `{0}` can only pick a tool")]
    BareKey(String),
    #[error("`{accelerator}` already runs {with}")]
    Conflict { accelerator: String, with: String },
    #[error("there is no shortcut set `{0}`")]
    UnknownSet(String),
    #[error("there is no command `{0}`")]
    UnknownAction(String),
    #[error("the shortcut file is not valid: {0}")]
    BadFile(String),
    #[error("the shortcut file is larger than {0} bytes")]
    TooLarge(usize),
}

/// A chord: modifiers (`CmdOrCtrl`, `Alt`, `Shift`) and one key, in canonical order and case so
/// that two spellings of one chord compare equal.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Accelerator {
    cmd: bool,
    alt: bool,
    shift: bool,
    key: String,
}

const NAMED_KEYS: &[&str] = &[
    "Enter",
    "Space",
    "Tab",
    "Escape",
    "Delete",
    "Backspace",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "Up",
    "Down",
    "Left",
    "Right",
    ",",
    ".",
    "-",
    "=",
    "/",
    ";",
    "[",
    "]",
];

impl Accelerator {
    /// Parse `text`. A bare letter or digit is accepted only for a tool, where the workspace
    /// handles it when nothing is being typed.
    pub fn parse(text: &str, action: MenuAction) -> Result<Self, ShortcutError> {
        let bad = || ShortcutError::BadAccelerator(text.to_owned());
        if text.is_empty() || text.len() > 40 {
            return Err(bad());
        }
        let (mut cmd, mut alt, mut shift) = (false, false, false);
        let mut parts: Vec<&str> = text.split('+').collect();
        // A literal plus key is written `Plus`; `+` alone or a trailing `+` is malformed.
        let key = parts.pop().filter(|k| !k.is_empty()).ok_or_else(bad)?;
        for part in parts {
            let flag = match part.to_ascii_lowercase().as_str() {
                "cmdorctrl" | "cmd" | "ctrl" | "control" | "command" => &mut cmd,
                "alt" | "option" => &mut alt,
                "shift" => &mut shift,
                _ => return Err(bad()),
            };
            if *flag {
                return Err(bad());
            }
            *flag = true;
        }
        let key = canonical_key(key).ok_or_else(bad)?;
        let accel = Self {
            cmd,
            alt,
            shift,
            key,
        };
        let bare = !accel.cmd && !accel.alt && !accel.shift;
        let typeable = accel.key.chars().count() == 1 && !",.-=/;[]".contains(accel.key.as_str());
        if bare && typeable && action.tool_key().is_none() {
            return Err(ShortcutError::BareKey(accel.key));
        }
        Ok(accel)
    }

    /// The key, as egui and muda name it (`V`, `F5`, `Enter`).
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn has_command(&self) -> bool {
        self.cmd
    }

    pub fn has_alt(&self) -> bool {
        self.alt
    }

    pub fn has_shift(&self) -> bool {
        self.shift
    }

    /// True for a chord with no modifier at all.
    pub fn is_bare(&self) -> bool {
        !self.cmd && !self.alt && !self.shift
    }

    /// The chord in muda's syntax (`CmdOrCtrl+Shift+Z`), which the native menu registers.
    pub fn muda_text(&self) -> String {
        let mut out = String::new();
        if self.cmd {
            out.push_str("CmdOrCtrl+");
        }
        if self.alt {
            out.push_str("Alt+");
        }
        if self.shift {
            out.push_str("Shift+");
        }
        out.push_str(&self.key);
        out
    }

    /// As a person reads it: `Ctrl+O`, or with symbols on macOS.
    pub fn display(&self) -> String {
        let mac = cfg!(target_os = "macos");
        let mut out = String::new();
        if self.cmd {
            out.push_str(if mac { "\u{2318}" } else { "Ctrl+" });
        }
        if self.alt {
            out.push_str(if mac { "\u{2325}" } else { "Alt+" });
        }
        if self.shift {
            out.push_str(if mac { "\u{21e7}" } else { "Shift+" });
        }
        out.push_str(&self.key);
        out
    }
}

/// The muda spelling, which is also the file's: `CmdOrCtrl+Alt+Shift+Key`.
impl fmt::Display for Accelerator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.cmd {
            f.write_str("CmdOrCtrl+")?;
        }
        if self.alt {
            f.write_str("Alt+")?;
        }
        if self.shift {
            f.write_str("Shift+")?;
        }
        f.write_str(&self.key)
    }
}

fn canonical_key(key: &str) -> Option<String> {
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next())
        && c.is_ascii_alphanumeric()
    {
        return Some(c.to_ascii_uppercase().to_string());
    }
    let lower = key.to_ascii_lowercase();
    if let Some(n) = lower.strip_prefix('f')
        && let Ok(n) = n.parse::<u8>()
        && (1..=12).contains(&n)
    {
        return Some(format!("F{n}"));
    }
    match lower.as_str() {
        "plus" => return Some("Plus".to_owned()),
        "esc" => return Some("Escape".to_owned()),
        "return" => return Some("Enter".to_owned()),
        "arrowup" => return Some("Up".to_owned()),
        "arrowdown" => return Some("Down".to_owned()),
        "arrowleft" => return Some("Left".to_owned()),
        "arrowright" => return Some("Right".to_owned()),
        _ => {}
    }
    NAMED_KEYS
        .iter()
        .find(|name| name.eq_ignore_ascii_case(key))
        .map(|name| (*name).to_owned())
}
