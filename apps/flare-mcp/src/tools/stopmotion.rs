//! Stop-motion for agents: a session's settings are checked and returned, and camera capture
//! answers with its named error. Sessions are not saved in a project yet, so nothing here
//! reads or changes the open document.

use flare_stopmotion::{CameraSource, NoCamera, OnionSkin, StopMotionSession};
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::server::{FlareServer, Outcome};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StartStopMotion {
    /// Captures per second, 1 to 120.
    pub fps: u32,
    /// Timeline frames each captured frame is held for, 1 to 60.
    pub hold: u32,
    /// Earlier frames shown as ghosts, 0 to 5.
    #[serde(default)]
    pub onion_before: u8,
    /// Later frames shown as ghosts, 0 to 5.
    #[serde(default)]
    pub onion_after: u8,
    /// Ghost opacity in percent, 0 to 100.
    #[serde(default = "default_opacity")]
    pub onion_opacity: u8,
}

fn default_opacity() -> u8 {
    50
}

/// The settings of a new session, checked by the library, as JSON.
pub(super) fn session_settings(args: StartStopMotion) -> Result<Value, String> {
    let mut session = StopMotionSession::new(args.fps, args.hold).map_err(|e| e.to_string())?;
    let onion = OnionSkin::new(args.onion_before, args.onion_after, args.onion_opacity)
        .map_err(|e| e.to_string())?;
    session.set_onion(onion);
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
    }))
}

/// The named refusal of camera capture, as the tool reports it.
pub(super) fn capture_refusal() -> Result<Value, String> {
    let mut camera = NoCamera;
    camera.capture().map_err(|e| e.to_string())?;
    Ok(json!({ "captured": true }))
}

#[tool_router(router = stopmotion_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "Start a stop-motion session: checks the frame rate, hold and onion skin and returns them. Sessions are not saved in a project yet."
    )]
    fn start_stop_motion(&self, Parameters(args): Parameters<StartStopMotion>) -> Outcome {
        session_settings(args).map(|value| value.to_string())
    }

    #[tool(
        description = "Capture a frame from a camera. Not built yet: returns the error that names the missing feature."
    )]
    fn capture_stop_motion_frame(&self) -> Outcome {
        capture_refusal().map(|value| value.to_string())
    }
}
