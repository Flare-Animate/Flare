//! Copy Motion / Paste Motion / Copy Motion as JS (P2 group 2).
//!
//! Copying a classic or motion tween keeps its per-frame transforms (what the
//! object does on every frame) and, for a motion tween, the span's
//! `Keyframe::motion` property keys with its ease. Pasting applies it to
//! another object as a new motion tween: from the keys directly, or - for a
//! classic tween - from the sampled frames, simplified to the few keys that
//! reproduce them. "Copy Motion as JS" writes the same tween as calls of the
//! scripting API (`docs/animatecraft/SCRIPTING.md`).

use crate::editor::{Editor, FrameSel};
use crate::geom::{pt, Matrix, Pt};
use crate::model::*;
use crate::motion::{Ease, Interp, Motion, PathKey, Pose, Prop, PropKey, Track};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// What the tweened object did on one frame.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameXform {
    pub matrix: Matrix,
    pub alpha: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipSource {
    Classic,
    Motion,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MotionClip {
    pub source: ClipSource,
    /// The object's transform on every copied frame (stage space), first frame first.
    pub frames: Vec<FrameXform>,
    /// `Keyframe::motion` of a copied motion tween.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<Motion>,
    /// The span's simple ease (−100…100).
    #[serde(default)]
    pub ease: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ease_curve: Option<Ease>,
    #[serde(default)]
    pub orient_to_path: bool,
    /// Local point the source object was transformed about.
    pub pivot: Pt,
    /// Length of the source span in frames (a classic tween: up to and including the end keyframe).
    pub len: u32,
}

const EPS_POS: f32 = 0.02;
const EPS_DEG: f32 = 0.02;
const EPS_SCALE: f32 = 0.0005;
const EPS_ALPHA: f32 = 0.002;

fn unwrap_deg(v: &mut [f32]) {
    for i in 1..v.len() {
        let mut d = v[i] - v[i - 1];
        while d > 180.0 {
            v[i] -= 360.0;
            d -= 360.0;
        }
        while d < -180.0 {
            v[i] += 360.0;
            d += 360.0;
        }
    }
}

/// Indices of the samples a piecewise-linear curve needs to stay within `tol`
/// of every sample (greedy Douglas-Peucker over (index, value)).
fn simplify_indices(n: usize, err: &dyn Fn(usize, usize, usize) -> f32, tol: f32) -> Vec<usize> {
    if n == 0 {
        return vec![];
    }
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    let mut stack = vec![(0usize, n - 1)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 {
            continue;
        }
        let (mut worst, mut wi) = (0.0f32, a);
        for i in a + 1..b {
            let e = err(a, b, i);
            if e > worst {
                worst = e;
                wi = i;
            }
        }
        if worst > tol {
            keep[wi] = true;
            stack.push((a, wi));
            stack.push((wi, b));
        }
    }
    (0..n).filter(|i| keep[*i]).collect()
}

fn track_of(values: &[f32], tol: f32) -> Track {
    let first = values.first().copied().unwrap_or(0.0);
    if values.iter().all(|v| (v - first).abs() <= tol) {
        return Track::default();
    }
    let lerp_err = |a: usize, b: usize, i: usize| {
        let u = (i - a) as f32 / (b - a) as f32;
        (values[i] - (values[a] + (values[b] - values[a]) * u)).abs()
    };
    let keys = simplify_indices(values.len(), &lerp_err, tol).into_iter().map(|i| PropKey::new(i as u32, values[i], false)).collect();
    Track { keys, ease: None }
}

/// A motion tween that reproduces `poses` (one per frame, first frame = the
/// object's own pose): straight path segments and linear property keys,
/// with only the keys the poses need.
pub fn fit_motion(pivot: Pt, poses: &[Pose]) -> Motion {
    let mut m = Motion::new(pivot);
    let n = poses.len();
    if n < 2 {
        return m;
    }
    let mut rot: Vec<f32> = poses.iter().map(|p| p.rotation).collect();
    unwrap_deg(&mut rot);
    let col = |f: &dyn Fn(&Pose) -> f32| poses.iter().map(f).collect::<Vec<f32>>();
    m.rotation = track_of(&rot, EPS_DEG);
    m.skew = track_of(&col(&|p| p.skew), EPS_DEG);
    m.scale_x = track_of(&col(&|p| p.scale_x), EPS_SCALE);
    m.scale_y = track_of(&col(&|p| p.scale_y), EPS_SCALE);
    m.alpha = track_of(&col(&|p| p.alpha), EPS_ALPHA);
    m.brightness = track_of(&col(&|p| p.brightness), EPS_ALPHA);
    let moves = poses.iter().any(|p| p.pos.dist(poses[0].pos) > EPS_POS);
    if moves {
        let pos: Vec<Pt> = poses.iter().map(|p| p.pos).collect();
        // distance of sample i from the chord a-b at the same parameter
        let err = |a: usize, b: usize, i: usize| {
            let u = (i - a) as f32 / (b - a) as f32;
            pos[i].dist(pos[a].lerp(pos[b], u))
        };
        m.path = simplify_indices(n, &err, EPS_POS).into_iter().map(|i| PathKey { frame: i as u32, at: pos[i], h_in: Pt::default(), h_out: Pt::default() }).collect();
    }
    m
}

fn pose_of_matrix(base: &Element, matrix: Matrix, alpha: f32, pivot: Pt) -> Pose {
    let mut e = base.clone();
    e.matrix = matrix;
    e.alpha = alpha;
    Pose::of(&e, pivot)
}

impl MotionClip {
    /// Poses of the copied frames, applied to `target` (a copy of the object it goes on).
    /// `relative`: the target does the same movement from where it is (the
    /// copied change in transform is applied to the target's own); otherwise it
    /// takes the copied transforms as they are.
    pub fn poses_for(&self, target: &Element, pivot: Pt, relative: bool) -> Vec<Pose> {
        let first = self.frames.first().copied().unwrap_or(FrameXform { matrix: Matrix::IDENTITY, alpha: 1.0 });
        let inv = first.matrix.invert().unwrap_or(Matrix::IDENTITY);
        self.frames
            .iter()
            .map(|f| {
                if relative {
                    let m = f.matrix.concat(&inv).concat(&target.matrix);
                    let a = if first.alpha > 1e-4 { target.alpha * f.alpha / first.alpha } else { f.alpha };
                    pose_of_matrix(target, m, a.clamp(0.0, 1.0), pivot)
                } else {
                    pose_of_matrix(target, f.matrix, f.alpha, pivot)
                }
            })
            .collect()
    }

    /// The tween this clip describes, as a motion for `target`: the copied keys
    /// (path moved to start at the target when `relative`), or one fitted to the frames.
    pub fn motion_for(&self, target: &Element, pivot: Pt, relative: bool) -> Motion {
        match &self.motion {
            Some(src) => {
                let mut m = src.clone();
                m.pivot = pivot;
                if relative {
                    if let Some(p0) = src.path.first().map(|k| k.at) {
                        let d = pt(target.matrix.apply(pivot).x - p0.x, target.matrix.apply(pivot).y - p0.y);
                        for k in &mut m.path {
                            k.at = pt(k.at.x + d.x, k.at.y + d.y);
                        }
                    }
                }
                m
            }
            None => fit_motion(pivot, &self.poses_for(target, pivot, relative)),
        }
    }

    /// The scripting-API calls that rebuild this tween on `layer` (1-based)
    /// starting at `start` (1-based frame): the tween, its ease, every property
    /// key (position as x/y pairs on the path's anchors) and non-default key types.
    pub fn to_commands(&self, layer: usize, start: u32) -> Vec<(String, Value)> {
        let motion = self.motion.clone().unwrap_or_else(|| {
            let base = self.frames.first().map(|f| (f.matrix, f.alpha)).unwrap_or((Matrix::IDENTITY, 1.0));
            let mut e = Element::group(vec![]);
            e.matrix = base.0;
            e.alpha = base.1;
            let poses: Vec<Pose> = self.frames.iter().map(|f| pose_of_matrix(&e, f.matrix, f.alpha, self.pivot)).collect();
            fit_motion(self.pivot, &poses)
        });
        let mut v: Vec<(String, Value)> = vec![];
        let mut tween = json!({ "kind": "motion", "layer": layer, "frame": start });
        if self.ease != 0.0 {
            tween["ease"] = num(self.ease);
        }
        if motion.rotate_turns != 0 {
            tween["rotate"] = json!(motion.rotate_turns);
        }
        if self.orient_to_path {
            tween["orient"] = json!(true);
        }
        v.push(("timeline.set_tween".into(), tween));
        // the span must be as long as the copied tween
        if self.len > 1 {
            v.push(("timeline.set_span_end".into(), json!({ "layer": layer, "frame": start, "end": start + self.len - 1 })));
        }
        if let Some(e) = &self.ease_curve {
            v.push(("timeline.set_ease".into(), json!({ "layer": layer, "frame": start, "ease": ease_json(e) })));
        }
        let at = |f: u32| start + f;
        for k in &motion.path {
            v.push(("motion.set_key".into(), json!({ "layer": layer, "prop": "x", "frame": at(k.frame), "value": num(k.at.x) })));
            v.push(("motion.set_key".into(), json!({ "layer": layer, "prop": "y", "frame": at(k.frame), "value": num(k.at.y) })));
        }
        let mut tracks: Vec<(Prop, &Track)> = [Prop::Rotation, Prop::Skew, Prop::ScaleX, Prop::ScaleY, Prop::Alpha, Prop::Brightness].into_iter().filter_map(|p| motion.track(p).map(|t| (p, t))).collect();
        tracks.extend(motion.extra.iter().map(|(p, t)| (*p, t)));
        for (p, t) in tracks {
            for k in &t.keys {
                v.push(("motion.set_key".into(), json!({ "layer": layer, "prop": p.id(), "frame": at(k.frame), "value": num(k.value) })));
            }
            for k in t.keys.iter().filter(|k| !k.interp.is_auto()) {
                let mut a = json!({ "layer": layer, "prop": p.id(), "key": at(k.frame), "type": k.interp.id() });
                match &k.interp {
                    Interp::EaseInOut { ease_in, ease_out } | Interp::EaseInOutPct { ease_in, ease_out } => {
                        a["ease_in"] = num(*ease_in);
                        a["ease_out"] = num(*ease_out);
                    }
                    Interp::Expression { text } => a["expression"] = json!(text),
                    Interp::SimilarShape { text, offset } => {
                        a["expression"] = json!(text);
                        a["offset"] = num(*offset);
                    }
                    _ => {}
                }
                v.push(("motion.set_interp".into(), a));
            }
            if let Some(e) = &t.ease {
                v.push(("motion.set_ease".into(), json!({ "layer": layer, "frame": start, "prop": p.id(), "ease": ease_json(e) })));
            }
        }
        if let Some(e) = &motion.path_ease {
            v.push(("motion.set_ease".into(), json!({ "layer": layer, "frame": start, "prop": "position", "ease": ease_json(e) })));
        }
        v
    }

    /// `to_commands` as a script: one `ac.<group>.<name>({...});` per call.
    pub fn to_js(&self, layer: usize, start: u32) -> String {
        let mut s = format!("// Flare motion: {} frame{}, from frame {start} on layer {layer}\n", self.len, if self.len == 1 { "" } else { "s" });
        for (id, args) in self.to_commands(layer, start) {
            s.push_str(&format!("ac.{id}({});\n", js_object(&args)));
        }
        s
    }
}

fn ease_json(e: &Ease) -> Value {
    match e {
        Ease::Preset { .. } => json!(e.name()),
        Ease::Custom { points } => Value::Array(points.iter().map(|p| json!([num(p.x), num(p.y), num(p.h_in.x), num(p.h_in.y), num(p.h_out.x), num(p.h_out.y)])).collect()),
    }
}

/// A number as JSON with the shortest f32 spelling (no 0.30000001192092896).
fn num(v: f32) -> Value {
    format!("{v}").parse::<f64>().ok().and_then(serde_json::Number::from_f64).map(Value::Number).unwrap_or(Value::Null)
}

/// A JSON object as a JavaScript object literal (bare keys, the usual ones first).
fn js_object(v: &Value) -> String {
    const FIRST: [&str; 14] = ["kind", "prop", "layer", "frame", "key", "value", "type", "ease", "ease_in", "ease_out", "expression", "offset", "end", "rotate"];
    match v {
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort_by_key(|k| (FIRST.iter().position(|f| f == k).unwrap_or(FIRST.len()), (*k).clone()));
            format!("{{ {} }}", keys.into_iter().map(|k| format!("{k}: {}", js_object(&m[k]))).collect::<Vec<_>>().join(", "))
        }
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", f as i64),
            _ => n.to_string(),
        },
        other => other.to_string(),
    }
}

// ------------------------------------------------------------------ editor

impl Editor {
    /// The object a motion goes on: the selected object (or the first one on
    /// the current layer) and the keyframe holding it.
    pub(crate) fn motion_target(&self) -> Result<(usize, u32, usize), String> {
        let (li, frame, ei) = match self.selection.as_slice() {
            [] => {
                let s = self.frame_target();
                (s.layer0, self.frame, 0)
            }
            [one] => (one.layer, self.frame, one.element),
            _ => return Err("Select one object to put the motion on".into()),
        };
        let layer = self.timeline().layers.get(li).ok_or("No layer")?;
        if layer.kind != LayerKind::Normal || !self.layer_editable(li) {
            return Err("That layer is locked, hidden or not a normal layer".into());
        }
        let (st, kf, _) = layer.span_at(frame).ok_or("No frame to put the motion on")?;
        if kf.elements.is_empty() {
            return Err("The frame is empty: select an instance first".into());
        }
        if kf.elements.len() > 1 {
            return Err("A motion tween animates one object per layer: put the instance on its own layer first".into());
        }
        Ok((li, st, ei.min(kf.elements.len() - 1)))
    }

    /// Copy Motion (Edit › Timeline › Copy Motion): the tween under the frame
    /// selection. Returns the number of frames copied.
    pub fn copy_motion(&mut self) -> Result<usize, String> {
        let s = self.frame_target();
        let layer = self.timeline().layers.get(s.layer0).ok_or("No layer")?;
        let (st, kf, next) = layer.span_at(s.f0).ok_or("Select frames of a tween")?;
        let end = next.unwrap_or(layer.end).min(layer.end.max(1));
        let (source, last) = match kf.tween {
            Tween::Classic { .. } => (ClipSource::Classic, next.unwrap_or(end.saturating_sub(1))),
            Tween::Motion { .. } if kf.motion.is_some() => (ClipSource::Motion, end - 1),
            _ => return Err("Select frames of a classic or motion tween".into()),
        };
        // a partial selection copies just those frames of a classic tween
        let (f0, f1) = if source == ClipSource::Classic && s.f1 > s.f0 { (s.f0.max(st), s.f1.min(last)) } else { (st, last) };
        let mut frames = vec![];
        for f in f0..=f1.max(f0) {
            let (els, _) = layer_elements_at(layer, f).ok_or("The tween has no content")?;
            let e = els.first().ok_or("The tween has no object")?;
            frames.push(FrameXform { matrix: e.matrix, alpha: e.alpha });
        }
        let first = kf.elements.first().ok_or("The tween has no object")?;
        let pivot = match &kf.motion {
            Some(m) => m.pivot,
            None => element_local_center(&EvalCtx::new(&self.doc, s.f0), first),
        };
        let len = frames.len() as u32;
        let n = frames.len();
        self.motion_clip = Some(MotionClip {
            source,
            frames,
            motion: if source == ClipSource::Motion && f0 == st { kf.motion.as_deref().cloned() } else { None },
            ease: kf.tween.ease(),
            ease_curve: kf.ease_curve.clone(),
            orient_to_path: kf.orient_to_path,
            pivot,
            len,
        });
        Ok(n)
    }

    pub fn can_paste_motion(&self) -> bool {
        self.motion_clip.is_some()
    }

    /// Paste Motion: the copied tween becomes a new motion tween of the
    /// selected object (one undo step). `relative`: the object moves as the
    /// copied one did, starting from where it is.
    pub fn paste_motion(&mut self, relative: bool) -> Result<(), String> {
        let clip = self.motion_clip.clone().ok_or("Nothing copied: use Copy Motion on a tween first")?;
        let (li, st, ei) = self.motion_target()?;
        let kf = &self.timeline().layers[li].keyframes[&st];
        let mut base = kf.elements[ei].clone();
        let pivot = element_local_center(&EvalCtx::new(&self.doc, st), &base);
        let motion = clip.motion_for(&base, pivot, relative);
        if !relative {
            if let Some(f) = clip.frames.first() {
                base.matrix = f.matrix;
                base.alpha = f.alpha;
            }
        }
        let ease = clip.ease;
        self.install_motion("Paste Motion", li, st, motion, base, Tween::Motion { ease }, clip.ease_curve.clone(), clip.orient_to_path, clip.len);
        Ok(())
    }

    /// Replace the span at `st` on layer `li` by a motion tween of `base`
    /// (one undo step); the span grows to `len` frames if it is shorter.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn install_motion(&mut self, label: &str, li: usize, st: u32, motion: Motion, base: Element, tween: Tween, ease_curve: Option<Ease>, orient: bool, len: u32) {
        self.checkpoint(label);
        let l = &mut self.timeline_mut().layers[li];
        let kf = l.keyframes.get_mut(&st).unwrap();
        kf.elements = vec![base];
        kf.tween = tween;
        kf.motion = Some(Box::new(motion));
        kf.ease_curve = ease_curve;
        kf.orient_to_path = orient;
        kf.tween_boundary = None;
        kf.prop_eases = None;
        kf.shape_hints.clear();
        let have = l.span_end(st) - st;
        if have < len {
            crate::frames::insert_frames(l, st + have - 1, len - have);
        }
        self.selection.clear();
        self.frame_sel = Some(FrameSel { layer0: li, layer1: li, f0: st, f1: st });
        self.frame = st;
        self.layer = li;
    }

    /// Copy Motion as JS: the copied motion (copying the tween under the
    /// selection first when nothing is on the motion clipboard) as scripting
    /// API calls, ready to paste into the Script Console.
    pub fn copy_motion_as_js(&mut self) -> Result<String, String> {
        let s = self.frame_target();
        let start = self.timeline().layers.get(s.layer0).and_then(|l| l.span_at(s.f0)).map(|(st, _, _)| st);
        self.copy_motion()?;
        let clip = self.motion_clip.as_ref().ok_or("Nothing to copy")?;
        Ok(clip.to_js(s.layer0 + 1, start.unwrap_or(s.f0) + 1))
    }
}
