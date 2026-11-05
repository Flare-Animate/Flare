//! Where the camera is at a frame, and the picture it takes.

use flare_document::{Camera, Content, Document, Layer, Timeline, Tween};
use glamx::DAffine2;

use super::morph;

/// The camera a camera layer holds at `frame`: its keyframe's, or between two keyframes when a
/// motion tween runs from one to the next. `None` before the layer's first keyframe, after its
/// last, and on a keyframe that is empty.
pub fn camera_at(layer: &Layer, frame: u32) -> Option<Camera> {
    let span = layer.span_at(frame)?;
    let Content::Camera(camera) = span.content() else {
        return None;
    };
    if span.tween() == Tween::Linear
        && let Some(Content::Camera(next)) = layer.span_at_start(span.end()).map(|n| n.content())
    {
        let t = morph::span_fraction(
            span,
            f64::from(frame - span.start()) / f64::from(span.duration().get()),
        );
        return Some(camera.lerp(next, t));
    }
    Some(*camera)
}

/// What to apply to a scene's timeline at `frame` to get the camera's picture: the identity
/// when the timeline has no camera layer, the layer is hidden, or it has nothing at the frame.
pub fn camera_view(document: &Document, timeline: &Timeline, frame: u32) -> DAffine2 {
    timeline
        .layers()
        .iter()
        .find(|layer| layer.is_camera() && layer.is_visible())
        .and_then(|layer| camera_at(layer, frame))
        .map_or(DAffine2::IDENTITY, |camera| camera.view(&document.stage()))
}
