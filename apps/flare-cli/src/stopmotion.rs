//! The stop-motion commands. The rules live in `flare-stopmotion`; this only prints them.

use anyhow::Result;
use clap::Subcommand;
use flare_stopmotion::{
    CameraSource, FrameStore, NoCamera, OnionSkin, PATTERN_HEIGHT, PATTERN_WIDTH, SimulatedCamera,
    StopMotionSession, capture_next, ghost_frames, timeline_slots,
};
use serde_json::{Value, json};

/// The stop-motion subcommands. Sessions are not saved in a project yet: every command
/// works on a session it builds and then drops, and its reply says so.
// The StopMotion prefix is what names the subcommands `stop-motion-*` when this enum is
// flattened into the command list, so it is kept.
#[allow(clippy::enum_variant_names)]
#[derive(Subcommand)]
pub enum Commands {
    /// Validate a stop-motion session's rate, hold and onion skin and print its settings. Sessions
    /// are not saved in a project yet, so this writes no file.
    StopMotionNew {
        #[arg(long, default_value_t = 12)]
        fps: u32,
        /// Timeline frames each captured frame is held for.
        #[arg(long, default_value_t = 1)]
        hold: u32,
        /// Earlier frames shown as ghosts, 0 to 5.
        #[arg(long, default_value_t = 0)]
        onion_before: u8,
        /// Later frames shown as ghosts, 0 to 5.
        #[arg(long, default_value_t = 0)]
        onion_after: u8,
        /// Ghost opacity in percent, 0 to 100.
        #[arg(long, default_value_t = 50)]
        onion_opacity: u8,
    },
    /// Capture a frame. No camera backend is built, so without --simulated this fails with
    /// the named error; with --simulated a deterministic pattern camera stands in and the
    /// reply says so.
    StopMotionCapture {
        #[arg(long)]
        simulated: bool,
    },
    /// Lay captured frames out on a timeline: each frame shows for its hold count of
    /// timeline frames. Sessions are not saved in a project yet, so this prints the layout
    /// for a session of the given size.
    StopMotionTimeline {
        #[arg(long, default_value_t = 12)]
        fps: u32,
        /// Timeline frames each captured frame is held for.
        #[arg(long, default_value_t = 1)]
        hold: u32,
        /// How many frames the session has captured.
        #[arg(long)]
        frames: usize,
    },
    /// Which frames the onion skin draws as ghosts while a frame is live.
    StopMotionOnion {
        /// Earlier frames shown as ghosts, 0 to 5.
        #[arg(long, default_value_t = 1)]
        before: u8,
        /// Later frames shown as ghosts, 0 to 5.
        #[arg(long, default_value_t = 0)]
        after: u8,
        /// Ghost opacity in percent.
        #[arg(long, default_value_t = 50)]
        opacity: u8,
        /// How many frames the session has captured.
        #[arg(long)]
        frames: usize,
        /// The frame the person is on, counting captured frames from 1.
        #[arg(long, default_value_t = 1)]
        frame: usize,
    },
}

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

pub fn capture(simulated: bool) -> Result<Value> {
    if !simulated {
        let mut camera = NoCamera;
        camera.capture()?;
    }
    // The pattern camera stands in for a device, and the reply says so. The session is
    // thrown away with the command: sessions are not saved in a project yet.
    let mut session = StopMotionSession::new(12, 1)?;
    let mut store = FrameStore::default();
    let mut camera = SimulatedCamera::default();
    let id = capture_next(&mut camera, &mut session, &mut store)?;
    Ok(json!({
        "captured": true,
        "camera": camera.name(),
        "pattern_width": PATTERN_WIDTH,
        "pattern_height": PATTERN_HEIGHT,
        "captures_before_this_one": camera.captures() - 1,
        "frame_id": id.get(),
        "hold": session.default_hold(),
        "session_saved": false,
    }))
}

pub fn timeline(fps: u32, hold: u32, frames: usize) -> Result<Value> {
    let mut session = StopMotionSession::new(fps, hold)?;
    for _ in 0..frames {
        session.add_frame()?;
    }
    let slots = timeline_slots(&session)?;
    let laid: Vec<Value> = slots
        .iter()
        .map(|slot| {
            json!({
                "frame": slot.frame().get(),
                "start": slot.start(),
                "length": slot.length(),
            })
        })
        .collect();
    Ok(json!({
        "fps": session.fps(),
        "captured_frames": session.frames().len(),
        "duration": session.duration(),
        "slots": laid,
        "session_saved": false,
    }))
}

pub fn onion(before: u8, after: u8, opacity: u8, frames: usize, frame: usize) -> Result<Value> {
    let skin = OnionSkin::new(before, after, opacity)?;
    // Positions count captured frames from 1 on the command line; the rules count from 0.
    let ghosts = ghost_frames(&skin, frames, frame.saturating_sub(1))?;
    Ok(json!({
        "onion_skin": {
            "before": skin.before(),
            "after": skin.after(),
            "opacity": skin.opacity(),
            "enabled": skin.is_enabled(),
        },
        "captured_frames": frames,
        "frame": frame,
        "ghosts": ghosts.iter().map(|at| at + 1).collect::<Vec<usize>>(),
    }))
}
