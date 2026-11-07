//! Reading the curves of an `AnimationCore`: one `Property` per animated value, each a list of
//! keys in ticks, with Bezier handles where the file gives them. The curves are flare-tween
//! curves, so the value at any tick is the one flare-tween's own evaluation gives.

use crate::xml::Node;
use flare_tween::{Curve, Extrapolation, Handle, HandleKind, Interpolation, Key, Params};

/// Gaps listed for one motion object. A long list says the same thing again, so it is capped.
const MAX_GAPS: usize = 16;

/// The values a motion object animates that Flare carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Channel {
    /// Offsets of the transformation point, in pixels.
    MoveX,
    MoveY,
    /// Degrees.
    Rotation,
    /// Degrees.
    SkewX,
    /// Percent; 100 is the instance's own size.
    ScaleX,
    ScaleY,
}

impl Channel {
    fn named(id: &str) -> Option<Self> {
        Some(match id {
            "Motion_X" => Self::MoveX,
            "Motion_Y" => Self::MoveY,
            "Rotation_Z" => Self::Rotation,
            "Skew_X" => Self::SkewX,
            "Scale_X" => Self::ScaleX,
            "Scale_Y" => Self::ScaleY,
            _ => return None,
        })
    }
}

/// A whole-curve ease of a property's timing: one of the power families Animate's Motion Editor
/// names Quadratic to Quintic. A positive strength ends slowly and a negative one starts slowly;
/// the strength is how much of the power curve is blended in (100 is all of it). The direction
/// and blend follow the file's sign and value. They have not been compared with Animate's own
/// playback.
#[derive(Debug, Clone, Copy, PartialEq)]
struct TimeMap {
    power: f64,
    strength: f64,
}

impl TimeMap {
    fn named(kind: &str, strength: f64) -> Option<Self> {
        let power = match kind {
            "Quadratic" => 2.0,
            "Cubic" => 3.0,
            "Quartic" => 4.0,
            "Quintic" => 5.0,
            _ => return None,
        };
        Some(Self {
            power,
            strength: (strength / 100.0).clamp(-1.0, 1.0),
        })
    }

    /// The eased fraction of `r`, the position through the whole motion (0 to 1).
    fn map(self, r: f64) -> f64 {
        let r = r.clamp(0.0, 1.0);
        let a = self.strength.abs();
        let curve = if self.strength > 0.0 {
            1.0 - (1.0 - r).powf(self.power)
        } else {
            r.powf(self.power)
        };
        a * curve + (1.0 - a) * r
    }
}

/// One motion object's curves, with what it asks for that is not imported.
pub(super) struct Motion {
    /// Ticks in one second: the file's `TimeScale`, normally frame rate times 1000.
    pub(super) ticks_per_second: f64,
    /// Ticks in the whole motion (`AnimationCore`'s `duration`), when the file gives it.
    duration_ticks: Option<f64>,
    curves: Vec<(Channel, Curve, Option<TimeMap>)>,
    /// What the file animates that is not imported, in the file's own names.
    pub(super) gaps: Vec<String>,
}

impl Motion {
    /// The value of `channel` at `ticks`, or `None` when the motion does not animate it.
    pub(super) fn value(&self, channel: Channel, ticks: f64) -> Option<f64> {
        let (_, curve, time_map) = self.curves.iter().find(|(c, _, _)| *c == channel)?;
        let ticks = match (time_map, self.duration_ticks) {
            (Some(map), Some(total)) => map.map(ticks / total) * total,
            _ => ticks,
        };
        curve.evaluate(ticks).ok()
    }

    /// Whether any curve was read.
    pub(super) fn animates_anything(&self) -> bool {
        !self.curves.is_empty()
    }

    fn gap(&mut self, text: String) {
        if self.gaps.len() < MAX_GAPS && !self.gaps.contains(&text) {
            self.gaps.push(text);
        }
    }
}

/// Reads the `AnimationCore` of a motion object. `frame_rate` is the document's, in frames per
/// second, and is the `TimeScale` when the file leaves it out.
pub(super) fn read(core: &Node, frame_rate: f64) -> Motion {
    let ticks_per_second = core
        .attr("TimeScale")
        .and_then(|t| t.trim().parse::<f64>().ok())
        .filter(|t| t.is_finite() && *t > 0.0)
        .unwrap_or(frame_rate * 1000.0);
    let duration_ticks = core
        .attr("duration")
        .and_then(|t| t.trim().parse::<f64>().ok())
        .filter(|t| t.is_finite() && *t > 0.0);
    let mut motion = Motion {
        ticks_per_second,
        duration_ticks,
        curves: Vec::new(),
        gaps: Vec::new(),
    };
    let time_maps: Vec<(String, f64)> = core
        .children_named("TimeMap")
        .map(|t| {
            let strength = t
                .attr("strength")
                .and_then(|s| s.trim().parse::<f64>().ok())
                .filter(|s| s.is_finite())
                .unwrap_or(0.0);
            (t.attr("type").unwrap_or("Linear").to_owned(), strength)
        })
        .collect();

    // Walk the property containers without recursion. Each property knows the container it is
    // in, which tells colour and filter curves apart from motion.
    let mut stack: Vec<(&Node, String)> =
        core.children.iter().map(|c| (c, String::new())).collect();
    while let Some((node, container)) = stack.pop() {
        match node.name.as_str() {
            "PropertyContainer" => {
                let id = node.attr("id").unwrap_or_default().to_owned();
                stack.extend(node.children.iter().map(|c| (c, id.clone())));
            }
            "Property" => read_property(node, &container, &time_maps, &mut motion),
            _ => stack.extend(node.children.iter().map(|c| (c, container.clone()))),
        }
    }
    motion
}

fn read_property(node: &Node, container: &str, time_maps: &[(String, f64)], out: &mut Motion) {
    let id = node.attr("id").unwrap_or_default();
    if node.attr("enabled") == Some("0") {
        return;
    }
    let Some(channel) = Channel::named(id) else {
        let what = match container {
            "Colors" => "colour curves are not imported".to_owned(),
            "Filters" => "filter curves are not imported".to_owned(),
            _ => format!("the motion curve {id} is not imported"),
        };
        out.gap(what);
        return;
    };
    let mut time_map = None;
    if node.attr("ignoreTimeMap") != Some("1") {
        let index = node
            .attr("TimeMapIndex")
            .and_then(|i| i.parse::<usize>().ok())
            .unwrap_or(0);
        if let Some((kind, strength)) = time_maps.get(index).filter(|(_, s)| *s != 0.0) {
            time_map = TimeMap::named(kind, *strength);
            if time_map.is_none() {
                out.gap(format!(
                    "the time map {kind} is not imported; the curves keep their own timing"
                ));
            }
        }
    }
    let keys: Vec<Key> = node
        .children_named("Keyframe")
        .filter_map(read_key)
        .collect();
    // A property with no keys but its own value is a constant.
    let keys = if keys.is_empty() {
        match node
            .attr("value")
            .and_then(|v| v.trim().parse::<f64>().ok())
        {
            Some(value) if value.is_finite() => vec![Key::new(0.0, value)],
            _ => return,
        }
    } else {
        keys
    };
    match Curve::new(keys, Extrapolation::Constant, Params::default()) {
        Ok(curve) => out.curves.push((channel, curve, time_map)),
        Err(error) => out.gap(format!("the curve of {id} could not be read ({error})")),
    }
}

/// One key: its anchor value at `timevalue`, and its handles as `dt,value` pairs in ticks and
/// absolute values. A key with no usable anchor is skipped.
fn read_key(node: &Node) -> Option<Key> {
    let time = node
        .attr("timevalue")
        .and_then(|t| t.trim().parse::<f64>().ok())
        .filter(|t| t.is_finite())?;
    let (_, value) = pair(node.attr("anchor")?)?;
    let mut key = Key::new(time, value);
    let right = node
        .attr("next")
        .and_then(pair)
        .map(|(dt, v)| Handle { dt, dv: v - value });
    let left = node
        .attr("previous")
        .and_then(pair)
        .map(|(dt, v)| Handle { dt, dv: v - value });
    if let Some(h) = right {
        key.right = h;
    }
    if let Some(h) = left {
        key.left = h;
    }
    // A segment with a handle away from its key is a curve; without one it is a straight line.
    if right.is_some_and(|h| h.dt != 0.0 || h.dv != 0.0) {
        key.interpolation = Interpolation::Bezier;
        key.handle_kind = HandleKind::Free;
    } else {
        key.interpolation = Interpolation::Linear;
    }
    Some(key)
}

/// `a,b` as two finite numbers.
fn pair(text: &str) -> Option<(f64, f64)> {
    let (a, b) = text.split_once(',')?;
    let a = a.trim().parse::<f64>().ok().filter(|n| n.is_finite())?;
    let b = b.trim().parse::<f64>().ok().filter(|n| n.is_finite())?;
    Some((a, b))
}
