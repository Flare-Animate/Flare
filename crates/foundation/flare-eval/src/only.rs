//! One scene layer's picture on its own, as onion skin shows the frames around the layer that
//! is being drawn on rather than the whole scene.

use flare_core::{LayerId, SceneId};
use flare_document::Document;
use glamx::DAffine2;

use crate::{EvalError, Frame, camera_view, evaluate_frame_seen_by, scene_of};

/// [`crate::evaluate_frame`] drawing only `layer`, and what it holds when it is a folder. Masks
/// and motion guides above it still shape it, and the camera still looks at it. A layer the
/// scene does not have draws nothing.
pub fn evaluate_frame_of_layer(
    document: &Document,
    scene: SceneId,
    frame: u32,
    layer: LayerId,
) -> Result<Frame<'_>, EvalError> {
    let view = camera_view(document, scene_of(document, scene)?.timeline(), frame);
    evaluate_frame_seen_by(document, scene, frame, view, false, Some(layer))
}

/// [`evaluate_frame_of_layer`] with the camera ignored, as the stage is edited.
pub fn evaluate_frame_of_layer_without_camera(
    document: &Document,
    scene: SceneId,
    frame: u32,
    layer: LayerId,
) -> Result<Frame<'_>, EvalError> {
    evaluate_frame_seen_by(
        document,
        scene,
        frame,
        DAffine2::IDENTITY,
        true,
        Some(layer),
    )
}

/// What only the scene's own timeline is drawn with: a symbol's layers are always all drawn and
/// never show a reference layer.
#[derive(Clone, Copy, Default)]
pub(crate) struct Top {
    /// Whether reference layers show, as on the stage.
    pub(crate) show_reference: bool,
    /// The one scene layer to draw (with what it holds, for a folder); `None` draws them all.
    pub(crate) only: Option<LayerId>,
}
