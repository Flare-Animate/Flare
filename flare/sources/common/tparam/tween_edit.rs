// Ported from Flare (github.com/Flare-Animate/Flare, flare/sources/flare/rust/core,
// commit a27136a), whose core is AnimationCraft's ac-core (MIT OR Apache-2.0).
//! Sparse corrections over a retained authored curve. No sampled geometry or
//! asset references: source paths, hints, easing and motion remain authoritative.
use crate::{frames, model::*, Matrix};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TweenPoseEdit {
    pub element: usize,
    pub time: f32,
    /// Exclusive endpoint in source time; may precede zero for a reversed span.
    pub end: f32,
    pub matrix: [f32; 6],
    pub alpha: f32,
    pub fill: Option<[f32; 4]>,
}

fn components(m: Matrix) -> [f32; 6] {
    [m.a, m.b, m.c, m.d, m.tx, m.ty]
}
fn rgba(c: Option<Color>) -> [f32; 4] {
    c.map_or([0.0; 4], |c| {
        [c.r as f32, c.g as f32, c.b as f32, c.a as f32]
    })
}

impl TweenPoseEdit {
    fn between(
        element: usize,
        time: f32,
        end: f32,
        before: &Element,
        after: &Element,
    ) -> Option<Self> {
        let mut comparable = after.clone();
        comparable.matrix = before.matrix;
        comparable.alpha = before.alpha;
        let fill = match (&before.kind, &after.kind, &mut comparable.kind) {
            (ElementKind::Shape(a), ElementKind::Shape(b), ElementKind::Shape(c)) => {
                c.fill = a.fill;
                if a.fill != b.fill {
                    // Removal is a topology/paint-mode edit, not a color adjustment.
                    b.fill?;
                    let (a, b) = (rgba(a.fill), rgba(b.fill));
                    Some(std::array::from_fn(|i| b[i] - a[i]))
                } else {
                    None
                }
            }
            _ => None,
        };
        if comparable != *before {
            return None;
        }
        let (a, b) = (components(before.matrix), components(after.matrix));
        let edit = Self {
            element,
            time,
            end,
            matrix: std::array::from_fn(|i| b[i] - a[i]),
            alpha: after.alpha - before.alpha,
            fill,
        };
        edit.finite().then_some(edit)
    }

    pub fn finite(&self) -> bool {
        self.time.is_finite()
            && self.end.is_finite()
            && self.time != self.end
            && self.matrix.iter().all(|x| x.is_finite())
            && self.alpha.is_finite()
            && self.fill.iter().flatten().all(|x| x.is_finite())
    }

    pub fn transform_space(&mut self, m: Matrix) {
        let [a, b, c, d, x, y] = self.matrix;
        self.matrix = [
            m.a * a + m.c * b,
            m.b * a + m.d * b,
            m.a * c + m.c * d,
            m.b * c + m.d * d,
            m.a * x + m.c * y,
            m.b * x + m.d * y,
        ];
    }
}

pub fn apply(edits: &[TweenPoseEdit], time: f32, elements: &mut [Element]) {
    for (index, el) in elements.iter_mut().enumerate() {
        let mut matrix = components(el.matrix);
        let mut alpha = el.alpha;
        let mut fill = match &el.kind {
            ElementKind::Shape(s) => s.fill,
            _ => None,
        };
        let mut color = rgba(fill);
        let mut changed = false;
        for edit in edits.iter().filter(|e| e.element == index) {
            let weight = (1.0 - (time - edit.time) / (edit.end - edit.time)).clamp(0.0, 1.0);
            if weight == 0.0 {
                continue;
            }
            changed = true;
            for (value, delta) in matrix.iter_mut().zip(edit.matrix) {
                *value += delta * weight;
            }
            alpha += edit.alpha * weight;
            if let Some(delta) = edit.fill {
                for (value, delta) in color.iter_mut().zip(delta) {
                    *value += delta * weight;
                }
                fill = Some(Color::rgb(0, 0, 0));
            }
        }
        if !changed {
            continue;
        }
        let [a, b, c, d, tx, ty] = matrix;
        el.matrix = Matrix { a, b, c, d, tx, ty };
        el.alpha = alpha.clamp(0.0, 1.0);
        if let ElementKind::Shape(s) = &mut el.kind {
            if fill.is_some() {
                let [r, g, b, a] = color.map(|v| v.round().clamp(0.0, 255.0) as u8);
                s.fill = Some(Color { r, g, b, a });
            }
        }
    }
}

/// Preflight all selected objects before splitting. A supported edit creates at
/// most one timeline boundary and a sparse correction per object. The prefix
/// and retained endpoint stay fixed; corrections fade in source-time coordinates.
pub fn edit_pose(
    layer: &mut Layer,
    frame: u32,
    selected: &[usize],
    edit: &impl Fn(&mut Element),
) -> bool {
    if !layer.bones.is_empty() {
        return false;
    }
    let Some((start, key, _)) = layer.span_at(frame) else {
        return false;
    };
    if key.tween_boundary.is_none() {
        return false;
    }
    let len = layer.span_end(start) - start;
    let boundary = key.tween_boundary.as_ref().unwrap();
    let time = boundary.source_time(frame - start, len);
    let step = (boundary.time_end - boundary.time_start) / len as f32
        * if boundary.reversed { -1.0 } else { 1.0 };
    let end = boundary.source_time(0, len) + len as f32 * step;
    let Some((elements, _)) = layer_elements_at(layer, frame) else {
        return false;
    };
    let mut changes = vec![];
    for &index in selected {
        let Some(before) = elements.get(index) else {
            return false;
        };
        let mut after = before.clone();
        edit(&mut after);
        if after == *before {
            continue;
        }
        let Some(change) = TweenPoseEdit::between(index, time, end, before, &after) else {
            return false;
        };
        changes.push(change);
    }
    if changes.is_empty() {
        return true;
    }
    frames::convert_to_keyframes(layer, frame, frame, false);
    let boundary = layer
        .keyframes
        .get_mut(&frame)
        .unwrap()
        .tween_boundary
        .as_mut()
        .unwrap();
    for change in changes {
        if let Some(old) = boundary
            .pose_edits
            .iter_mut()
            .find(|e| e.element == change.element && e.time == change.time && e.end == change.end)
        {
            for (value, delta) in old.matrix.iter_mut().zip(change.matrix) {
                *value += delta;
            }
            old.alpha += change.alpha;
            if let Some(delta) = change.fill {
                let color = old.fill.get_or_insert([0.0; 4]);
                for (value, delta) in color.iter_mut().zip(delta) {
                    *value += delta;
                }
            }
        } else {
            boundary.pose_edits.push(change);
        }
    }
    true
}

/// Keep object-index corrections attached to surviving source elements.
pub fn remap_elements(key: &mut Keyframe, retained: &[usize]) {
    if let Some(boundary) = &mut key.tween_boundary {
        boundary.pose_edits.retain_mut(|e| {
            if let Some(index) = retained.iter().position(|&old| old == e.element) {
                e.element = index;
                true
            } else {
                false
            }
        });
    }
}
