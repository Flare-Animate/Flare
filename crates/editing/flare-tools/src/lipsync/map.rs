//! Which frame of the mouth symbol each shape shows. Any number of drawings will do: two shapes
//! may share one, and a symbol with fewer frames than shapes has the rest held at its last.

use thiserror::Error;

use super::Viseme;

/// The longest mapping text read: ten `name=frame` pairs fit many times over.
const MAX_MAP_TEXT: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MapError {
    #[error("the mapping is longer than {MAX_MAP_TEXT} characters")]
    TooLong,
    #[error(
        "`{0}` is not a mouth shape; the shapes are neutral, mbp, ai, e, o, u, fv, l, wq and etc"
    )]
    UnknownShape(String),
    #[error("`{0}` should be a shape, `=` and a frame counted from 0, like `mbp=3`")]
    BadPair(String),
}

/// A frame of the mouth symbol (counting from 0) for each mouth shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouthMap([u32; Viseme::ALL.len()]);

impl MouthMap {
    /// The shapes on the symbol's frames in [`Viseme::ALL`]'s order, which is how a mouth chart is
    /// usually drawn: Neutral on the first frame, MBP on the second and so on.
    pub fn in_order() -> Self {
        let mut frames = [0; Viseme::ALL.len()];
        for (frame, slot) in (0_u32..).zip(frames.iter_mut()) {
            *slot = frame;
        }
        Self(frames)
    }

    pub fn frame(&self, viseme: Viseme) -> u32 {
        self.0[viseme.index()]
    }

    #[must_use]
    pub fn with(mut self, viseme: Viseme, frame: u32) -> Self {
        self.0[viseme.index()] = frame;
        self
    }

    /// Each shape with its frame, in [`Viseme::ALL`]'s order.
    pub fn pairs(&self) -> impl Iterator<Item = (Viseme, u32)> + '_ {
        Viseme::ALL.into_iter().map(|v| (v, self.frame(v)))
    }

    /// [`MouthMap::in_order`] with the shapes `text` names moved: `mbp=3, o=5`, separated by
    /// commas, frames counted from 0. Empty text is the order unchanged. The command line and the
    /// MCP server both read a mapping with this.
    pub fn parse(text: &str) -> Result<Self, MapError> {
        if text.len() > MAX_MAP_TEXT {
            return Err(MapError::TooLong);
        }
        let mut map = Self::in_order();
        for pair in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let (name, frame) = pair
                .split_once('=')
                .ok_or_else(|| MapError::BadPair(pair.to_owned()))?;
            let viseme = Viseme::parse(name)
                .ok_or_else(|| MapError::UnknownShape(name.trim().to_owned()))?;
            let frame = frame
                .trim()
                .parse()
                .map_err(|_| MapError::BadPair(pair.to_owned()))?;
            map = map.with(viseme, frame);
        }
        Ok(map)
    }
}

impl Default for MouthMap {
    fn default() -> Self {
        Self::in_order()
    }
}
