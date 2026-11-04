//! The stop-motion commands. The rules live in `flare-stopmotion`; this only prints them.

use anyhow::Result;
use flare_stopmotion::{CameraSource, NoCamera, OnionSkin, StopMotionSession};
use serde_json::{Value, json};

pub fn new_session(
    fps: u32,
    hold: u32,
    onion_before: u8,
    onion_after: u8,
    onion_opacity: u8,
) -> Result<Value> {
    let mut session = StopMotionSession::new(fps, hold)?;
    session.set_onion(OnionSkin::new(onion_before, onion_after, onion_opacity)?);
    let onion = session.onion();
    Ok(json!({
        "fps": session.fps(),
        "default_hold": session.default_hold(),
        "duration": session.duration(),
        "frames": [],
        "onion_skin": {
            "before": onion.before(),
            "after": onion.after(),
            "opacity": onion.opacity(),
            "enabled": onion.is_enabled(),
        },
        "camera": NoCamera.name(),
    }))
}

pub fn capture() -> Result<Value> {
    let mut camera = NoCamera;
    camera.capture()?;
    Ok(json!({ "captured": true }))
}
