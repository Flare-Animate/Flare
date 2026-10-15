//! Motion presets (Window › Motion Presets): ready-made movements applied to
//! the selected object as a motion tween, and your own saved from a tween.
//!
//! Presets are our own designs, stored as JSON. Values are **relative to the
//! object they are applied to**, so one preset works anywhere on the stage:
//! `x`/`y` are offsets in pixels, `rotation`/`skew` are added degrees,
//! `scale_x`/`scale_y`/`alpha` are multipliers of the object's own values.
//! Key frames count from 0 over `frames` frames; when applied to a span that
//! already has a length the keys are stretched over it.
//!
//! ```json
//! { "name": "Fly In From Left", "category": "Entrance", "frames": 18,
//!   "ease": "Cubic Out", "tracks": { "x": [ {"f": 0, "v": -420}, {"f": 18, "v": 0} ] } }
//! ```
//! 3D, tint and filter keys are not part of a preset.

use crate::editor::Editor;
use crate::geom::{pt, Pt};
use crate::model::*;
use crate::motion::{Ease, Motion, PathKey, Pose, Prop, PropKey, Track};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PresetKey {
    /// Frame, counted from 0.
    pub f: u32,
    pub v: f32,
    /// A smooth (curved) point rather than a corner.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub smooth: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MotionPreset {
    pub name: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub description: String,
    /// Length of the preset in frames (24 fps feel).
    pub frames: u32,
    /// The span's ease by name ("Cubic Out").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ease: Option<String>,
    /// A custom ease curve (saved presets).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ease_curve: Option<Ease>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub orient_to_path: bool,
    /// Tracks by property id: x, y, rotation, skew, scale_x, scale_y, alpha.
    pub tracks: BTreeMap<String, Vec<PresetKey>>,
}

/// The presets that ship with Flare.
pub fn builtin() -> Vec<MotionPreset> {
    serde_json::from_str(include_str!("motion_presets.json")).expect("built-in motion presets are valid JSON")
}

pub fn to_json(presets: &[MotionPreset]) -> String {
    serde_json::to_string_pretty(presets).expect("presets serialise")
}

pub fn from_json(s: &str) -> Result<Vec<MotionPreset>, String> {
    serde_json::from_str(s).map_err(|e| format!("not a motion preset file: {e}"))
}

/// Value of a preset track at frame `f` (corner/linear between keys, held outside).
fn track_value(keys: &[PresetKey], f: f32, neutral: f32) -> f32 {
    let (Some(first), Some(last)) = (keys.first(), keys.last()) else { return neutral };
    if f <= first.f as f32 {
        return if first.f > 0 && f < first.f as f32 { neutral } else { first.v };
    }
    if f >= last.f as f32 {
        return last.v;
    }
    let i = keys.windows(2).position(|w| f >= w[0].f as f32 && f <= w[1].f as f32).unwrap_or(0);
    let (a, b) = (&keys[i], &keys[i + 1]);
    a.v + (b.v - a.v) * (f - a.f as f32) / (b.f - a.f).max(1) as f32
}

impl MotionPreset {
    /// The last key frame: the preset runs over `last_frame() + 1` frames.
    pub fn last_frame(&self) -> u32 {
        self.tracks.values().flatten().map(|k| k.f).max().unwrap_or(0).max(self.frames).max(1)
    }

    pub fn ease_for(&self) -> Option<Ease> {
        self.ease_curve.clone().or_else(|| self.ease.as_deref().and_then(Ease::from_name))
    }

    /// The motion this preset makes for `base` (pose about `pivot`) over `len` frames.
    pub fn motion_for(&self, base: &Pose, pivot: Pt, len: u32) -> Motion {
        let mut m = Motion::new(pivot);
        let src_last = self.last_frame() as f32;
        let len = len.max(2);
        let k = (len - 1) as f32 / src_last;
        let map_f = |f: u32| ((f as f32 * k).round() as u32).min(len - 1);
        // position: the path's anchors are the frames of the x and y keys
        let (xs, ys) = (self.tracks.get("x").map(Vec::as_slice).unwrap_or(&[]), self.tracks.get("y").map(Vec::as_slice).unwrap_or(&[]));
        if !xs.is_empty() || !ys.is_empty() {
            let mut frames: Vec<u32> = xs.iter().chain(ys).map(|k| k.f).collect();
            frames.sort_unstable();
            frames.dedup();
            if frames.first() != Some(&0) {
                frames.insert(0, 0);
            }
            let smooth_at = |f: u32| xs.iter().chain(ys).any(|k| k.f == f && k.smooth);
            let at: Vec<Pt> = frames.iter().map(|&f| pt(base.pos.x + track_value(xs, f as f32, 0.0), base.pos.y + track_value(ys, f as f32, 0.0))).collect();
            for (i, &f) in frames.iter().enumerate() {
                let (h_in, h_out) = if smooth_at(f) && i > 0 && i + 1 < frames.len() {
                    let d = pt((at[i + 1].x - at[i - 1].x) / 6.0, (at[i + 1].y - at[i - 1].y) / 6.0);
                    (pt(-d.x, -d.y), d)
                } else {
                    (Pt::default(), Pt::default())
                };
                m.path.push(PathKey { frame: map_f(f), at: at[i], h_in, h_out });
            }
            m.path.dedup_by_key(|k| k.frame);
        }
        let put = |id: &str, track: &mut Track, neutral: f32, f: &dyn Fn(f32) -> f32| {
            let Some(keys) = self.tracks.get(id).filter(|k| !k.is_empty()) else { return };
            let mut out: Vec<PropKey> = vec![];
            if keys[0].f > 0 {
                out.push(PropKey::new(0, f(neutral), false));
            }
            out.extend(keys.iter().map(|k| PropKey::new(map_f(k.f), f(k.v), k.smooth)));
            out.dedup_by_key(|k| k.frame);
            *track = Track { keys: out, ease: None };
        };
        put("rotation", &mut m.rotation, 0.0, &|v| base.rotation + v);
        put("skew", &mut m.skew, 0.0, &|v| base.skew + v);
        put("scale_x", &mut m.scale_x, 1.0, &|v| base.scale_x * v);
        put("scale_y", &mut m.scale_y, 1.0, &|v| base.scale_y * v);
        put("alpha", &mut m.alpha, 1.0, &|v| (base.alpha * v).clamp(0.0, 1.0));
        m
    }

    /// A preset from a motion tween of `base`, `len` frames long: every key
    /// becomes relative to the object's own values.
    pub fn from_motion(name: &str, motion: &Motion, base: &Element, len: u32, ease: Option<Ease>, orient: bool) -> MotionPreset {
        let b = Pose::of(base, motion.pivot);
        let mut tracks: BTreeMap<String, Vec<PresetKey>> = BTreeMap::new();
        if !motion.path.is_empty() {
            let curved = |k: &PathKey| k.h_in != Pt::default() || k.h_out != Pt::default();
            tracks.insert("x".into(), motion.path.iter().map(|k| PresetKey { f: k.frame, v: k.at.x - b.pos.x, smooth: curved(k) }).collect());
            tracks.insert("y".into(), motion.path.iter().map(|k| PresetKey { f: k.frame, v: k.at.y - b.pos.y, smooth: curved(k) }).collect());
        }
        let mut rel = |p: Prop, id: &str, f: &dyn Fn(f32) -> f32| {
            if let Some(t) = motion.track(p).filter(|t| !t.keys.is_empty()) {
                tracks.insert(id.into(), t.keys.iter().map(|k| PresetKey { f: k.frame, v: f(k.value), smooth: k.smooth }).collect());
            }
        };
        rel(Prop::Rotation, "rotation", &|v| v - b.rotation);
        rel(Prop::Skew, "skew", &|v| v - b.skew);
        let ratio = |base: f32| move |v: f32| if base.abs() > 1e-4 { v / base } else { v };
        rel(Prop::ScaleX, "scale_x", &ratio(b.scale_x));
        rel(Prop::ScaleY, "scale_y", &ratio(b.scale_y));
        rel(Prop::Alpha, "alpha", &ratio(b.alpha));
        let (named, curve) = match ease {
            Some(e) if Ease::from_name(&e.name()).as_ref() == Some(&e) => (Some(e.name()), None),
            e => (None, e),
        };
        MotionPreset { name: name.into(), category: "My presets".into(), description: String::new(), frames: len.max(1), ease: named, ease_curve: curve, orient_to_path: orient, tracks }
    }
}

impl Editor {
    /// Apply a motion preset to the selected object (or the object on the
    /// current layer): a new motion tween, one undo step. A one-frame span
    /// grows to the preset's length; a longer span keeps its length and the
    /// preset is stretched over it.
    pub fn apply_motion_preset(&mut self, preset: &MotionPreset) -> Result<(), String> {
        let (li, st, ei) = self.motion_target()?;
        let l = &self.timeline().layers[li];
        let base = l.keyframes[&st].elements[ei].clone();
        let span = l.span_end(st) - st;
        let len = if span > 1 { span } else { preset.last_frame() + 1 };
        let pivot = element_local_center(&EvalCtx::new(&self.doc, st), &base);
        let motion = preset.motion_for(&Pose::of(&base, pivot), pivot, len);
        let ease_curve = preset.ease_for();
        self.install_motion(&format!("Motion Preset: {}", preset.name), li, st, motion, base, Tween::Motion { ease: 0.0 }, ease_curve, preset.orient_to_path, len);
        Ok(())
    }

    /// Save the motion tween under the playhead as a preset named `name`.
    pub fn motion_preset_from_tween(&self, name: &str) -> Result<MotionPreset, String> {
        let (st, len, m, kf) = self.current_motion().ok_or("Put the playhead on a motion tween first")?;
        let base = kf.elements.first().ok_or("The tween has no object")?;
        let _ = st;
        Ok(MotionPreset::from_motion(name, m, base, len, kf.ease_curve.clone(), kf.orient_to_path))
    }
}
