//! Scripted input: which keys are held and where the pointer is, one frame at a time. A script
//! is text the command line and MCP can send, so a test, a CLI run and an agent drive a game the
//! same way. Nothing here reads a real keyboard.

use std::collections::BTreeSet;

use crate::limits::{MAX_ACTION_CHARS, MAX_COORD, MAX_INPUT_TOKENS, MAX_RUN_FRAMES};
use crate::{GameError, Key};

/// What the player does in one frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Input {
    keys: BTreeSet<Key>,
    /// The named actions held this frame, which a surface sends as `action:<name>`. Unlike the
    /// six keys, an action is only read by what binds it: a hook with `when=action:<name>`.
    actions: BTreeSet<String>,
    pointer: Option<(f64, f64)>,
    pressed: bool,
}

impl Input {
    /// Nothing held and no pointer.
    pub fn none() -> Self {
        Self::default()
    }

    /// The keys held this frame.
    pub fn with_keys(keys: impl IntoIterator<Item = Key>) -> Self {
        Self {
            keys: keys.into_iter().collect(),
            ..Self::default()
        }
    }

    /// The named actions held this frame. Refused when a name is empty, too long or has
    /// control characters.
    pub fn with_actions(names: impl IntoIterator<Item = String>) -> Result<Self, GameError> {
        let mut input = Self::none();
        for name in names {
            check_action(&name)?;
            input.actions.insert(name);
        }
        Ok(input)
    }

    /// A click at `(x, y)` this frame. Refused unless both coordinates are finite and inside the
    /// coordinate limit.
    pub fn click(x: f64, y: f64) -> Result<Self, GameError> {
        check_point(x, y)?;
        Ok(Self {
            pointer: Some((x, y)),
            pressed: true,
            ..Self::default()
        })
    }

    pub fn key_down(&self, key: Key) -> bool {
        self.keys.contains(&key)
    }

    /// Whether the named action was held this frame.
    pub fn action(&self, name: &str) -> bool {
        self.actions.contains(name)
    }

    pub fn pointer(&self) -> Option<(f64, f64)> {
        self.pointer
    }

    /// Whether the pointer was pressed this frame (a click, not a hover).
    pub fn pressed(&self) -> bool {
        self.pressed
    }

    /// Checks a value built some other way: the step calls this before it reads the input.
    pub(crate) fn validate(&self) -> Result<(), GameError> {
        for name in &self.actions {
            check_action(name)?;
        }
        match self.pointer {
            None => Ok(()),
            Some((x, y)) => check_point(x, y),
        }
    }

    /// One frame of text: comma-separated key names (`left,space`) and `click:x:y` for a press.
    /// An empty frame is no input.
    pub fn parse(frame: &str) -> Result<Self, GameError> {
        let tokens: Vec<&str> = frame
            .split(',')
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .collect();
        if tokens.len() > MAX_INPUT_TOKENS {
            return Err(GameError::BadInput(format!(
                "a frame names more than {MAX_INPUT_TOKENS} keys or actions"
            )));
        }
        let mut input = Self::none();
        for token in tokens {
            if let Some(rest) = token.strip_prefix("click:") {
                let mut parts = rest.split(':');
                let (Some(x), Some(y), None) = (parts.next(), parts.next(), parts.next()) else {
                    return Err(GameError::BadInput(format!("{token:?} is not click:x:y")));
                };
                let x = x
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| GameError::BadInput(format!("{token:?} has a bad x")))?;
                let y = y
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| GameError::BadInput(format!("{token:?} has a bad y")))?;
                check_point(x, y)?;
                input.pointer = Some((x, y));
                input.pressed = true;
            } else if let Some(name) = token.strip_prefix("action:") {
                check_action(name)?;
                input.actions.insert(name.to_owned());
            } else {
                let key =
                    Key::parse(token).ok_or_else(|| GameError::UnknownKey(token.to_owned()))?;
                input.keys.insert(key);
            }
        }
        Ok(input)
    }
}

/// A script: one frame per `;`-separated part. `;` alone is an empty frame.
pub fn parse_script(text: &str) -> Result<Vec<Input>, GameError> {
    let frames: Vec<&str> = text.split(';').collect();
    if frames.len() > MAX_RUN_FRAMES as usize {
        return Err(GameError::TooManyFrames);
    }
    frames.into_iter().map(Input::parse).collect()
}

fn check_point(x: f64, y: f64) -> Result<(), GameError> {
    for (name, value) in [("x", x), ("y", y)] {
        if !value.is_finite() {
            return Err(GameError::NonFinite(name));
        }
        if value.abs() > MAX_COORD {
            return Err(GameError::BadInput(format!(
                "pointer {name} is outside the coordinate limit"
            )));
        }
    }
    Ok(())
}

/// A named input action: 1 to `MAX_ACTION_CHARS` characters, no control characters.
fn check_action(name: &str) -> Result<(), GameError> {
    let len = name.chars().count();
    if len == 0 || len > MAX_ACTION_CHARS || name.chars().any(char::is_control) {
        return Err(GameError::BadInput(format!(
            "action name must be 1 to {MAX_ACTION_CHARS} characters with no control characters"
        )));
    }
    Ok(())
}
