//! Pure frame evaluation: document + scene + frame -> placed instances.
//!
//! No I/O, rendering or UI state. Instances are identified by their instance *path*, so
//! two occurrences of one symbol keep separate clocks and transforms. Graphic symbols
//! are parent-clocked: child frame = frames since the instance span began, looping over
//! the symbol's length. Movie Clip and Button runtime behaviour is out of scope.
#![forbid(unsafe_code)]

use flare_core::{InstanceId, SceneId, SymbolId};
use flare_document::{Content, Document, Instance, Layer, Shape, Timeline, Tween};
use glamx::{DAffine2, DVec2};
use thiserror::Error;

const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 100_000;

#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    /// Instances from the scene down to this one.
    pub path: Vec<InstanceId>,
    pub symbol: SymbolId,
    /// Composed transform into scene space.
    pub transform: DAffine2,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvalError {
    #[error("no such scene {0:?}")]
    NoSuchScene(SceneId),
    #[error("symbol nesting deeper than {MAX_DEPTH}")]
    TooDeep,
    #[error("more than {MAX_NODES} placed instances")]
    TooManyNodes,
    #[error("instance {0:?} cannot be tweened: its matrix has shear or reflection")]
    UnsupportedTween(InstanceId),
}

/// Everything visible at one frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frame<'a> {
    /// Instances, parents before children.
    pub placed: Vec<Placed>,
    /// Shapes in paint order: later entries draw on top.
    pub shapes: Vec<DrawnShape<'a>>,
    /// The mask regions that `DrawnShape::clips` refer to.
    pub clips: Vec<Vec<ClipShape<'a>>>,
}

/// One shape of a mask: the mask's region is the union of these.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipShape<'a> {
    pub shape: &'a Shape,
    /// Composed transform into scene space.
    pub transform: DAffine2,
}

impl Frame<'_> {
    /// A hash of what this frame draws: which shapes, where, in what order. Two frames of one
    /// document revision with the same signature render identically, so a held exposure (the
    /// same drawing for many frames) can reuse one image. Shapes are identified by id, which is
    /// stable within a revision; a different revision needs a different cache key.
    pub fn signature(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.shapes.len().hash(&mut hasher);
        for drawn in &self.shapes {
            drawn.shape.id().hash(&mut hasher);
            for value in drawn.transform.to_cols_array() {
                value.to_bits().hash(&mut hasher);
            }
            drawn.clips.hash(&mut hasher);
        }
        // Masks change what shows even when the masked shapes stay put.
        for clip in &self.clips {
            clip.len().hash(&mut hasher);
            for member in clip {
                member.shape.id().hash(&mut hasher);
                for value in member.transform.to_cols_array() {
                    value.to_bits().hash(&mut hasher);
                }
            }
        }
        hasher.finish()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DrawnShape<'a> {
    /// Instances from the scene down to the shape's owner (empty for scene-level shapes).
    pub path: Vec<InstanceId>,
    pub shape: &'a Shape,
    /// Composed transform into scene space.
    pub transform: DAffine2,
    /// Indices into `Frame::clips`, outermost mask first. The shape shows only where it lies
    /// inside every one of these regions.
    pub clips: Vec<usize>,
}

/// Whether a folder above `layers[index]` that holds it (directly or through other folders) is
/// hidden. Parents are always earlier layers, so one pass up the list finds them all.
fn hidden_by_folder(layers: &[flare_document::Layer], index: usize) -> bool {
    let mut parent = layers[index].parent();
    while let Some(id) = parent {
        let Some(folder) = layers[..index].iter().find(|l| l.id() == id) else {
            return false;
        };
        if !folder.is_visible() {
            return true;
        }
        parent = folder.parent();
    }
    false
}

fn scene_of(document: &Document, scene: SceneId) -> Result<&flare_document::Scene, EvalError> {
    document
        .scenes()
        .iter()
        .find(|candidate| candidate.id() == scene)
        .ok_or(EvalError::NoSuchScene(scene))
}

/// Placed instances at `frame`, parents before children, bottom layer drawn first.
pub fn evaluate(document: &Document, scene: SceneId, frame: u32) -> Result<Vec<Placed>, EvalError> {
    Ok(evaluate_frame(document, scene, frame)?.placed)
}

/// Placed instances and drawn shapes at `frame`.
pub fn evaluate_frame(
    document: &Document,
    scene: SceneId,
    frame: u32,
) -> Result<Frame<'_>, EvalError> {
    let scene = scene_of(document, scene)?;
    let mut out = Frame::default();
    walk(
        document,
        scene.timeline(),
        frame,
        DAffine2::IDENTITY,
        &mut Vec::new(),
        &[],
        &mut out,
    )?;
    Ok(out)
}

fn walk<'a>(
    document: &'a Document,
    timeline: &'a Timeline,
    frame: u32,
    parent: DAffine2,
    path: &mut Vec<InstanceId>,
    inherited: &[usize],
    out: &mut Frame<'a>,
) -> Result<(), EvalError> {
    if path.len() >= MAX_DEPTH {
        return Err(EvalError::TooDeep);
    }
    let layers = timeline.layers();
    // Layers are ordered top to bottom, so draw in reverse.
    for (index, layer) in layers.iter().enumerate().rev() {
        // A mask layer is never drawn itself; a hidden one stops masking.
        if !layer.is_visible()
            || layer.is_mask()
            || layer.is_guide()
            || layer.is_folder()
            || hidden_by_folder(layers, index)
        {
            continue;
        }
        let mut clips = inherited.to_vec();
        if let Some(mask) = index
            .checked_sub(1)
            .map(|above| &layers[above])
            .filter(|above| above.is_mask() && above.is_visible())
        {
            let mut scratch = Frame::default();
            walk_layer(
                document,
                mask,
                frame,
                parent,
                path,
                Steer::default(),
                &mut scratch,
            )?;
            if out.placed.len() + out.shapes.len() + scratch.shapes.len() >= MAX_NODES {
                return Err(EvalError::TooManyNodes);
            }
            clips.push(out.clips.len());
            out.clips.push(
                scratch
                    .shapes
                    .iter()
                    .map(|drawn| ClipShape {
                        shape: drawn.shape,
                        transform: drawn.transform,
                    })
                    .collect(),
            );
        }
        // A visible motion guide directly above steers this layer's motion tweens.
        let guide = index
            .checked_sub(1)
            .map(|above| &layers[above])
            .filter(|above| above.is_guide() && above.is_visible())
            .and_then(|above| above.span_at(frame))
            .and_then(|span| match span.content() {
                Content::Shape(shape) => Some(shape),
                _ => None,
            });
        walk_layer(
            document,
            layer,
            frame,
            parent,
            path,
            Steer {
                clips: &clips,
                guide,
            },
            out,
        )?;
    }
    Ok(())
}

/// What shapes how one layer is walked: the mask regions its shapes lie inside, and the motion
/// guide whose path its tweens follow.
#[derive(Clone, Copy, Default)]
struct Steer<'s> {
    clips: &'s [usize],
    guide: Option<&'s Shape>,
}

/// One layer's content at `frame`, added to `out` with the given clip regions.
fn walk_layer<'a>(
    document: &'a Document,
    layer: &'a Layer,
    frame: u32,
    parent: DAffine2,
    path: &mut Vec<InstanceId>,
    steer: Steer<'_>,
    out: &mut Frame<'a>,
) -> Result<(), EvalError> {
    let Steer { clips, guide } = steer;
    let Some(span) = layer.span_at(frame) else {
        return Ok(());
    };
    if out.placed.len() + out.shapes.len() >= MAX_NODES {
        return Err(EvalError::TooManyNodes);
    }
    let instance = match span.content() {
        Content::Empty => return Ok(()),
        Content::Shape(shape) => {
            out.shapes.push(DrawnShape {
                path: path.clone(),
                shape,
                transform: parent,
                clips: clips.to_vec(),
            });
            return Ok(());
        }
        Content::Instance(instance) => instance,
    };
    let local = tweened(layer, frame, instance, guide)?;
    let transform = parent * local;
    path.push(instance.id());
    out.placed.push(Placed {
        path: path.clone(),
        symbol: instance.symbol(),
        transform,
    });
    if let Some(symbol) = document.symbol(instance.symbol()) {
        let length = symbol.timeline().frame_count();
        if length > 0 {
            let elapsed = u64::from(frame - span.start());
            // `elapsed % length` < length <= u32::MAX * 2; the remainder fits u32 only
            // when length fits, so fall back to saturation for absurd lengths.
            let local = u32::try_from(elapsed % length).unwrap_or(u32::MAX);
            walk(
                document,
                symbol.timeline(),
                local,
                transform,
                path,
                clips,
                out,
            )?;
        }
    }
    path.pop();
    Ok(())
}

/// Applies a linear motion tween toward the next adjacent span on the same layer.
///
/// Interpolates scale, rotation (shortest arc) and translation. Matrices that are not a
/// pure scale-rotate-translate (shear, reflection) are reported, not approximated.
fn tweened(
    layer: &Layer,
    frame: u32,
    instance: &Instance,
    guide: Option<&Shape>,
) -> Result<DAffine2, EvalError> {
    let (id, from) = (instance.id(), instance.transform().as_affine());
    let Some(span) = layer.span_at(frame) else {
        return Ok(from);
    };
    if span.tween() != Tween::Linear {
        return Ok(from);
    }
    let Some(next) = layer.span_at_start(span.end()) else {
        return Ok(from);
    };
    let Content::Instance(target) = next.content() else {
        return Ok(from);
    };
    if target.symbol() != instance.symbol() {
        return Ok(from);
    }
    let to = target.transform().as_affine();
    let (Some((s0, r0, t0)), Some((s1, r1, t1))) = (decompose(from), decompose(to)) else {
        return Err(EvalError::UnsupportedTween(id));
    };
    let t = f64::from(frame - span.start()) / f64::from(span.duration().get());
    let mut delta = (r1 - r0) % std::f64::consts::TAU;
    if delta > std::f64::consts::PI {
        delta -= std::f64::consts::TAU;
    } else if delta < -std::f64::consts::PI {
        delta += std::f64::consts::TAU;
    }
    // With a guide the registration point rides the guide's path, from its start to its end,
    // instead of going straight from one position to the other.
    let position = guide
        .and_then(|shape| guide::point_along(shape.path(), t))
        .unwrap_or_else(|| t0.lerp(t1, t));
    Ok(DAffine2::from_scale_angle_translation(
        s0.lerp(s1, t),
        r0 + delta * t,
        position,
    ))
}

/// Whether a motion tween can start or end at `matrix`: only a scale, rotation and translation
/// can be interpolated, so a matrix with shear or reflection cannot.
pub fn can_tween(matrix: DAffine2) -> bool {
    matrix.is_finite() && decompose(matrix).is_some()
}

/// Scale, angle, translation, or `None` when the matrix is not exactly representable.
fn decompose(matrix: DAffine2) -> Option<(DVec2, f64, DVec2)> {
    let (scale, angle, translation) = matrix.to_scale_angle_translation();
    let rebuilt = DAffine2::from_scale_angle_translation(scale, angle, translation);
    let close = rebuilt
        .to_cols_array()
        .iter()
        .zip(matrix.to_cols_array())
        .all(|(a, b)| (a - b).abs() <= 1e-9 * (1.0 + b.abs()));
    close.then_some((scale, angle, translation))
}

mod guide;

pub use guide::{flatten, point_along};

#[cfg(test)]
mod tests;
