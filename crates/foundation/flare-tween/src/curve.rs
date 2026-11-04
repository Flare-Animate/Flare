//! A curve over keyframes: the value at any time, with handles solved for each key and the
//! extrapolation beyond the first and last key.

use crate::TweenError;
use crate::bezier::cubic_at;
use crate::easing::{Easing, Interpolation, Params, check_range, ease_valid};

/// Keys per curve. Evaluation is a binary search, so this bounds the work, not the time.
pub const MAX_KEYS: usize = 10_000;

/// How a key's handles are chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HandleKind {
    /// Tangent from the neighbouring keys, so the curve passes smoothly through the key.
    Auto,
    /// Auto, but flat at an end or where the curve turns, so it does not overshoot the keys.
    AutoClamped,
    /// Each handle points at its neighbour, a third of the way along.
    Vector,
    /// The two handles stay on one line; the left one follows the right one.
    Aligned,
    /// Handles as given.
    Free,
}

impl HandleKind {
    pub const ALL: [Self; 5] = [
        Self::Auto,
        Self::AutoClamped,
        Self::Vector,
        Self::Aligned,
        Self::Free,
    ];

    /// The name the command line, the MCP server and saved files use.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::AutoClamped => "auto-clamped",
            Self::Vector => "vector",
            Self::Aligned => "aligned",
            Self::Free => "free",
        }
    }

    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.name() == name)
    }
}

/// A handle, as an offset from its key: `dt` in time, `dv` in value. The left handle has `dt <= 0`
/// and the right one `dt >= 0`; `solve` clamps them so the time stays inside the segment.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Handle {
    pub dt: f64,
    pub dv: f64,
}

/// A keyframe: where it is, what the segment after it looks like, and its handles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Key {
    pub time: f64,
    pub value: f64,
    /// The shape of the segment that starts at this key.
    pub interpolation: Interpolation,
    pub easing: Easing,
    pub handle_kind: HandleKind,
    pub left: Handle,
    pub right: Handle,
}

impl Key {
    /// A key with the usual defaults: a Bezier segment, clamped handles, auto easing.
    #[must_use]
    pub fn new(time: f64, value: f64) -> Self {
        Self {
            time,
            value,
            interpolation: Interpolation::Bezier,
            easing: Easing::Auto,
            handle_kind: HandleKind::AutoClamped,
            left: Handle::default(),
            right: Handle::default(),
        }
    }
}

/// What the curve does before its first key and after its last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Extrapolation {
    /// Holds the end values.
    Constant,
    /// Continues the slope of the first or last segment.
    Linear,
    /// Repeats the keys from the first one, over and over.
    Cyclic,
    /// Repeats the keys, each repeat starting where the last one ended.
    CyclicOffset,
}

impl Extrapolation {
    pub const ALL: [Self; 4] = [
        Self::Constant,
        Self::Linear,
        Self::Cyclic,
        Self::CyclicOffset,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Constant => "constant",
            Self::Linear => "linear",
            Self::Cyclic => "cyclic",
            Self::CyclicOffset => "cyclic-offset",
        }
    }

    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.name() == name)
    }
}

/// Keys sorted by time, with solved handles. Built by `Curve::new`, which validates the input.
#[derive(Debug, Clone, PartialEq)]
pub struct Curve {
    keys: Vec<Key>,
    extrapolation: Extrapolation,
    params: Params,
}

impl Curve {
    /// Sorts the keys by time, solves their handles and checks everything.
    ///
    /// # Errors
    /// No keys, more than `MAX_KEYS`, a number that is not finite, parameters out of range, or
    /// two keys at the same time.
    pub fn new(
        mut keys: Vec<Key>,
        extrapolation: Extrapolation,
        params: Params,
    ) -> Result<Self, TweenError> {
        params.validate()?;
        if keys.is_empty() || keys.len() > MAX_KEYS {
            return Err(TweenError::KeyCount {
                count: keys.len(),
                max: MAX_KEYS,
            });
        }
        for key in &keys {
            check_finite("key time", key.time)?;
            check_finite("key value", key.value)?;
            check_finite(
                "handle",
                key.left.dt + key.left.dv + key.right.dt + key.right.dv,
            )?;
        }
        keys.sort_by(|a, b| a.time.total_cmp(&b.time));
        if let Some(pair) = keys.windows(2).find(|pair| pair[0].time == pair[1].time) {
            return Err(TweenError::DuplicateTime { time: pair[0].time });
        }
        solve_handles(&mut keys);
        Ok(Self {
            keys,
            extrapolation,
            params,
        })
    }

    #[must_use]
    pub fn keys(&self) -> &[Key] {
        &self.keys
    }

    #[must_use]
    pub fn extrapolation(&self) -> Extrapolation {
        self.extrapolation
    }

    /// The value at `t`.
    ///
    /// # Errors
    /// `t` is not finite. A finite `t` always gives a finite value, or `NonFiniteResult` when the
    /// keys are so large that the result would overflow.
    pub fn evaluate(&self, t: f64) -> Result<f64, TweenError> {
        check_finite("time", t)?;
        let value = evaluate_at(&self.keys, self.extrapolation, &self.params, t);
        if value.is_finite() {
            Ok(value)
        } else {
            Err(TweenError::NonFiniteResult)
        }
    }
}

fn check_finite(what: &'static str, value: f64) -> Result<(), TweenError> {
    check_range(what, value, f64::MIN, f64::MAX)
}

fn evaluate_at(keys: &[Key], extrapolation: Extrapolation, params: &Params, t: f64) -> f64 {
    let (Some(first), Some(last)) = (keys.first(), keys.last()) else {
        return 0.0;
    };
    if keys.len() == 1 || (first.time <= t && t <= last.time) {
        return inside(keys, params, t);
    }
    let span = last.time - first.time;
    match extrapolation {
        Extrapolation::Constant => {
            if t < first.time {
                first.value
            } else {
                last.value
            }
        }
        Extrapolation::Linear => {
            if t < first.time {
                let slope = segment_slope(&keys[0], &keys[1]);
                first.value + slope * (t - first.time)
            } else {
                let n = keys.len();
                let slope = segment_slope(&keys[n - 2], &keys[n - 1]);
                last.value + slope * (t - last.time)
            }
        }
        Extrapolation::Cyclic => {
            let local = first.time + (t - first.time).rem_euclid(span);
            inside(keys, params, local)
        }
        Extrapolation::CyclicOffset => {
            let cycles = ((t - first.time) / span).floor();
            let local = t - cycles * span;
            inside(keys, params, local) + cycles * (last.value - first.value)
        }
    }
}

fn segment_slope(a: &Key, b: &Key) -> f64 {
    (b.value - a.value) / (b.time - a.time)
}

/// The value inside the keys' range (`t` is clamped to it, which only matters for rounding).
fn inside(keys: &[Key], params: &Params, t: f64) -> f64 {
    let (Some(first), Some(last)) = (keys.first(), keys.last()) else {
        return 0.0;
    };
    let t = t.clamp(first.time, last.time);
    // The first key later than `t`: the segment is the one before it.
    let next = keys.partition_point(|key| key.time <= t);
    if next == 0 {
        return first.value;
    }
    match keys.get(next) {
        Some(b) => segment(&keys[next - 1], b, t, params),
        None => last.value,
    }
}

fn segment(a: &Key, b: &Key, t: f64, params: &Params) -> f64 {
    let width = b.time - a.time;
    let u = ((t - a.time) / width).clamp(0.0, 1.0);
    match a.interpolation {
        Interpolation::Constant => {
            if u >= 1.0 {
                b.value
            } else {
                a.value
            }
        }
        Interpolation::Bezier => {
            // The handles are in time (x) and value (y); `u` is the time as a fraction of the
            // segment, which is what the bezier is parameterised by in x.
            let x1 = (a.right.dt / width).clamp(0.0, 1.0);
            let x2 = (1.0 + b.left.dt / width).clamp(0.0, 1.0);
            let ys = [a.value, a.value + a.right.dv, b.value + b.left.dv, b.value];
            cubic_at([0.0, x1, x2, 1.0], ys, u)
        }
        other => {
            let e = ease_valid(other, a.easing, u, params);
            a.value + (b.value - a.value) * e
        }
    }
}

/// Sets every key's handles from its kind and its neighbours, then clamps them so each stays
/// inside its own segment. Neighbours are read from the keys' times and values, not handles, so
/// the result does not depend on the order keys are solved in.
fn solve_handles(keys: &mut [Key]) {
    let times: Vec<f64> = keys.iter().map(|k| k.time).collect();
    let values: Vec<f64> = keys.iter().map(|k| k.value).collect();
    let n = keys.len();
    for i in 0..n {
        let prev = i.checked_sub(1);
        let next = (i + 1 < n).then_some(i + 1);
        let kind = keys[i].handle_kind;
        match kind {
            HandleKind::Auto | HandleKind::AutoClamped => {
                let flat = kind == HandleKind::AutoClamped && is_turning_or_end(&values, i);
                let slope = if flat {
                    0.0
                } else {
                    tangent(&times, &values, i)
                };
                keys[i].right = match next {
                    Some(j) => {
                        let w = (times[j] - times[i]) / 3.0;
                        Handle {
                            dt: w,
                            dv: slope * w,
                        }
                    }
                    None => Handle::default(),
                };
                keys[i].left = match prev {
                    Some(j) => {
                        let w = (times[i] - times[j]) / 3.0;
                        Handle {
                            dt: -w,
                            dv: -slope * w,
                        }
                    }
                    None => Handle::default(),
                };
            }
            HandleKind::Vector => {
                keys[i].right = match next {
                    Some(j) => Handle {
                        dt: (times[j] - times[i]) / 3.0,
                        dv: (values[j] - values[i]) / 3.0,
                    },
                    None => Handle::default(),
                };
                keys[i].left = match prev {
                    Some(j) => Handle {
                        dt: -(times[i] - times[j]) / 3.0,
                        dv: (values[j] - values[i]) / 3.0,
                    },
                    None => Handle::default(),
                };
            }
            HandleKind::Aligned => {
                // The right handle is the one that was set; the left keeps its length, on the
                // opposite side of the key.
                let r = keys[i].right;
                let l = keys[i].left;
                let (rl, ll) = (r.dt.hypot(r.dv), l.dt.hypot(l.dv));
                if rl > 0.0 {
                    keys[i].left = Handle {
                        dt: -r.dt * ll / rl,
                        dv: -r.dv * ll / rl,
                    };
                }
            }
            HandleKind::Free => {}
        }
        clamp_handles(keys, i, prev, next, &times);
    }
}

fn clamp_handles(
    keys: &mut [Key],
    i: usize,
    prev: Option<usize>,
    next: Option<usize>,
    times: &[f64],
) {
    let key = &mut keys[i];
    key.right.dt = match next {
        Some(j) => key.right.dt.clamp(0.0, times[j] - times[i]),
        None => 0.0,
    };
    key.left.dt = match prev {
        Some(j) => key.left.dt.clamp(-(times[i] - times[j]), 0.0),
        None => 0.0,
    };
}

/// The slope an Auto handle follows: a one-sided difference at an end, central elsewhere.
fn tangent(times: &[f64], values: &[f64], i: usize) -> f64 {
    let n = times.len();
    if n < 2 {
        return 0.0;
    }
    let (a, b) = if i == 0 {
        (0, 1)
    } else if i + 1 == n {
        (n - 2, n - 1)
    } else {
        (i - 1, i + 1)
    };
    (values[b] - values[a]) / (times[b] - times[a])
}

/// True at either end, and where the values change direction or sit on a plateau. A clamped handle
/// is flat there, so the curve does not overshoot the keys.
fn is_turning_or_end(values: &[f64], i: usize) -> bool {
    if i == 0 || i + 1 == values.len() {
        return true;
    }
    (values[i] - values[i - 1]) * (values[i + 1] - values[i]) <= 0.0
}
