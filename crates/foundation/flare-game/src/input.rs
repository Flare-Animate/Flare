//! Scripted input: which keys are held and where the pointer is, one frame at a time. A script
//! is text the command line and MCP can send, so a test, a CLI run and an agent drive a game the
//! same way. Nothing here reads a real keyboard.

use std::collections::BTreeSet;

use crate::limits::{MAX_COORD, MAX_INPUT_TOKENS, MAX_RUN_FRAMES};
use crate::{GameError, Key};

/// What the player does in one frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Input {
    keys: BTreeSet<Key>,
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

    pub fn pointer(&self) -> Option<(f64, f64)> {
        self.pointer
    }

    /// Whether the pointer was pressed this frame (a click, not a hover).
    pub fn pressed(&self) -> bool {
        self.pressed
    }

    /// Checks a value built some other way: the step calls this before it reads the input.
    pub(crate) fn validate(&self) -> Result<(), GameError> {
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
