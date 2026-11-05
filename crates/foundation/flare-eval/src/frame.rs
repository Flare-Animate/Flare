//! What evaluating a frame produces: placed instances, drawn shapes and the masks that clip them.

use std::borrow::Cow;

use flare_core::{InstanceId, Rgba8, SceneId, SymbolId};
use flare_document::{Bitmap, Shape};
use glamx::DAffine2;
use thiserror::Error;

use crate::{MAX_DEPTH, MAX_NODES};

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
    #[error("instance {0:?} cannot be tweened: its matrix is not finite")]
    UnsupportedTween(InstanceId),
}

/// Everything visible at one frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frame<'a> {
    /// Instances, parents before children.
    pub placed: Vec<Placed>,
    /// Shapes in paint order: later entries draw on top.
    pub shapes: Vec<DrawnShape<'a>>,
    /// Raster images, each knowing how many shapes were drawn before it.
    pub bitmaps: Vec<DrawnBitmap<'a>>,
    /// The mask regions that `DrawnShape::clips` refer to.
    pub clips: Vec<Vec<ClipShape<'a>>>,
    /// Things that went wrong but did not stop the frame, such as a tween that fell back to the
    /// span's own keyframe.
    pub warnings: Vec<String>,
}

/// One shape of a mask: the mask's region is the union of these.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipShape<'a> {
    pub shape: Cow<'a, Shape>,
    /// See [`DrawnShape::phase`].
    pub phase: u64,
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
        for bitmap in &self.bitmaps {
            bitmap.bitmap.id().hash(&mut hasher);
            bitmap.after_shapes.hash(&mut hasher);
            bitmap.opacity.to_bits().hash(&mut hasher);
            bitmap.clips.hash(&mut hasher);
            for value in bitmap.transform.to_cols_array() {
                value.to_bits().hash(&mut hasher);
            }
        }
        for drawn in &self.shapes {
            drawn.shape.id().hash(&mut hasher);
            drawn.phase.hash(&mut hasher);
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
                member.phase.hash(&mut hasher);
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
    /// The shape: borrowed from the document, or owned when it is a step of a shape tween (it
    /// keeps the identity of the shape it started from, so it can be picked on the stage).
    pub shape: Cow<'a, Shape>,
    /// Which step of a shape tween this is, as the bits of how far along it is; 0 when the shape is
    /// the document's own. Two drawn shapes with one identity and one phase are one picture.
    pub phase: u64,
    /// Composed transform into scene space.
    pub transform: DAffine2,
    /// Indices into `Frame::clips`, outermost mask first. The shape shows only where it lies
    /// inside every one of these regions.
    pub clips: Vec<usize>,
    /// The color of the layer the shape is on, which outline mode draws it in.
    pub outline: Rgba8,
}

/// A bitmap placed on the stage.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawnBitmap<'a> {
    pub path: Vec<InstanceId>,
    pub bitmap: &'a Bitmap,
    /// Maps the bitmap's pixel space (top-left origin) into scene space.
    pub transform: DAffine2,
    pub opacity: f32,
    /// How many of `Frame::shapes` come before this bitmap in draw order.
    pub after_shapes: usize,
    pub clips: Vec<usize>,
}
