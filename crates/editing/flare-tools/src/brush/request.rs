//! A brush stroke as a command line or an agent asks for it: points as numbers, a ready-made
//! profile by name, and the settings the desktop's tool panel has, checked and turned into the
//! brush and paint [`super::lay_strokes`] takes. Both surfaces read their requests here, so they
//! accept and refuse the same things.

use std::fmt;

use flare_core::Rgba8;
use flare_input::{InputSample, MAX_STABILIZER_LEVEL, PointerKind};

use super::{BrushStroke, BuiltinBrushProfile, MAX_TAPER, Paint, Taper};

/// The narrowest and widest brush, in stage units: the desktop's size slider.
pub const BRUSH_SIZE_RANGE: (f64, f64) = (1.0, 200.0);
/// The most the centre line may be simplified, in stage units.
pub const MAX_SMOOTHING: f64 = 100.0;
/// How far the desktop simplifies the centre line at 100% zoom.
pub const DEFAULT_SMOOTHING: f64 = 2.0;

/// What is wrong with a request.
#[derive(Debug, Clone, PartialEq)]
pub enum RequestError {
    /// No ready-made profile has this name.
    UnknownProfile(String),
    /// A setting outside its range.
    OutOfRange {
        setting: &'static str,
        range: String,
    },
    /// Point `point` of stroke `stroke` is not two or three numbers.
    PointShape { stroke: usize, point: usize },
    /// Point `point` of stroke `stroke` has a coordinate that is not a finite number.
    NotANumber { stroke: usize, point: usize },
    /// Point `point` of stroke `stroke` has a pressure outside 0 to 1.
    Pressure { stroke: usize, point: usize },
}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownProfile(name) => write!(
                f,
                "no brush profile is called {name:?}; the profiles are {}",
                BuiltinBrushProfile::keys().join(", ")
            ),
            Self::OutOfRange { setting, range } => write!(f, "{setting} must be {range}"),
            Self::PointShape { stroke, point } => write!(
                f,
                "point {point} of stroke {stroke} must be [x, y] or [x, y, pressure]"
            ),
            Self::NotANumber { stroke, point } => {
                write!(f, "point {point} of stroke {stroke} is not a finite number")
            }
            Self::Pressure { stroke, point } => write!(
                f,
                "the pressure of point {point} of stroke {stroke} must be 0 to 1"
            ),
        }
    }
}

impl std::error::Error for RequestError {}

/// The settings of a request; what is `None` comes from the profile (or the desktop's start).
#[derive(Clone, Debug, PartialEq)]
pub struct BrushRequest {
    /// A ready-made profile's key or name; `None` is the plain round brush.
    pub profile: Option<String>,
    /// Width at full pressure, in stage units.
    pub size: f64,
    pub color: Rgba8,
    /// How opaque the paint is, 0 to 100 (%).
    pub flow: Option<f32>,
    /// Samples averaged to steady the hand, 0 to [`MAX_STABILIZER_LEVEL`].
    pub stabilizer: Option<u8>,
    /// How long the start and the end taper, in stage units.
    pub taper: Option<(f64, f64)>,
    /// How far the centre line may be simplified, in stage units.
    pub smoothing: Option<f64>,
}

fn out_of_range(setting: &'static str, range: String) -> RequestError {
    RequestError::OutOfRange { setting, range }
}

impl BrushRequest {
    /// The brush and the paint the request asks for, or what is wrong with it.
    pub fn resolve(&self) -> Result<(BrushStroke, Paint), RequestError> {
        let profile = match self.profile.as_deref() {
            Some(name) => BuiltinBrushProfile::find(name)
                .ok_or_else(|| RequestError::UnknownProfile(name.to_owned()))?,
            None => BuiltinBrushProfile::Plain,
        };
        let preset = profile.preset();
        let (min, max) = BRUSH_SIZE_RANGE;
        if !(min..=max).contains(&self.size) {
            return Err(out_of_range("size", format!("{min} to {max}")));
        }
        let mut brush = preset.stroke(self.size);
        if let Some((start, end)) = self.taper {
            if !((0.0..=MAX_TAPER).contains(&start) && (0.0..=MAX_TAPER).contains(&end)) {
                return Err(out_of_range(
                    "taper",
                    format!("0 to {MAX_TAPER} at each end"),
                ));
            }
            brush.taper = Taper::new(start, end);
        }
        let flow = match self.flow {
            Some(flow) if (0.0..=100.0).contains(&flow) => flow / 100.0,
            Some(_) => return Err(out_of_range("flow", "0 to 100".to_owned())),
            None => preset.flow(),
        };
        let stabilizer = match self.stabilizer {
            Some(level) if level <= MAX_STABILIZER_LEVEL => level,
            Some(_) => {
                return Err(out_of_range(
                    "stabilizer",
                    format!("0 to {MAX_STABILIZER_LEVEL}"),
                ));
            }
            None => preset.stabilizer(),
        };
        let tolerance = self.smoothing.unwrap_or(DEFAULT_SMOOTHING);
        if !(0.0..=MAX_SMOOTHING).contains(&tolerance) {
            return Err(out_of_range("smoothing", format!("0 to {MAX_SMOOTHING}")));
        }
        let paint = Paint {
            color: self.color,
            flow,
            stabilizer,
            tolerance,
        };
        Ok((brush, paint))
    }
}

/// The samples of stroke number `stroke`, from points of two numbers (`x`, `y`, at full
/// pressure, as a mouse draws) or three (with the pen's pressure, 0 to 1).
pub fn stroke_samples(
    stroke: usize,
    points: &[Vec<f64>],
) -> Result<Vec<InputSample>, RequestError> {
    points
        .iter()
        .enumerate()
        .map(|(point, numbers)| {
            let (x, y, pressure) = match numbers.as_slice() {
                [x, y] => (*x, *y, 1.0),
                [x, y, p] => (*x, *y, *p),
                _ => return Err(RequestError::PointShape { stroke, point }),
            };
            if !(0.0..=1.0).contains(&pressure) {
                return Err(RequestError::Pressure { stroke, point });
            }
            InputSample::new(x, y, pressure as f32, PointerKind::Pen)
                .ok_or(RequestError::NotANumber { stroke, point })
        })
        .collect()
}
