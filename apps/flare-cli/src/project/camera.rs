//! Pointing the scene's camera.

use anyhow::{Context, Result};
use flare_core::DocumentPoint;
use flare_document::{Camera, Tween};
use serde_json::{Value, json};
use std::path::Path;

use super::*;

/// What a camera keyframe does after its frame.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum CameraMotion {
    /// Hold the camera until the next keyframe.
    None,
    /// Move the camera to the next keyframe's.
    Motion,
}

/// Makes the camera `camera` from `frame` on, creating the camera layer if the first scene has
/// none. What is not given stays what the camera has at that frame (the stage as it is, when
/// there is no camera yet). `rotation` is in degrees, clockwise.
pub fn camera(
    path: &Path,
    frame: u32,
    center: (Option<f64>, Option<f64>),
    zoom: Option<f64>,
    rotation: Option<f64>,
    motion: Option<CameraMotion>,
    ease: Option<i8>,
) -> Result<Value> {
    let mut session = open(path)?;
    let scene = session
        .document()
        .scenes()
        .first()
        .context("the document has no scene")?
        .id();
    let now = flare_editor::camera_now(session.document(), scene, frame)
        .context("the document has no scene")?;
    let point = DocumentPoint::new(
        center.0.unwrap_or(now.center().as_vec().x),
        center.1.unwrap_or(now.center().as_vec().y),
    )
    .context("the camera's position must be a finite number")?;
    let camera = Camera::new(
        point,
        zoom.unwrap_or(now.zoom()),
        rotation.map_or(now.rotation(), f64::to_radians),
    )?;
    let motion = match (motion, ease) {
        (None, None) => None,
        (kind, ease) => Some((
            match kind {
                Some(CameraMotion::Motion) => Tween::Linear,
                Some(CameraMotion::None) => Tween::None,
                // An ease alone changes the one the keyframe has; with none it is a motion.
                None => Tween::Linear,
            },
            ease.unwrap_or(0),
        )),
    };
    let commands = flare_editor::set_camera_key(session.document(), scene, frame, camera, motion)
        .with_context(|| {
        format!(
            "cannot key the camera at frame {frame}: keyframes go up to frame {}",
            flare_editor::MAX_CAMERA_FRAME
        )
    })?;
    session.apply(commands)?;
    commit(&session, path)?;
    Ok(json!({
        "frame": frame,
        "x": camera.center().as_vec().x,
        "y": camera.center().as_vec().y,
        "zoom": camera.zoom(),
        "rotation": camera.rotation().to_degrees(),
        "revision": session.revision(),
    }))
}
