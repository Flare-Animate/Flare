//! The stop-motion session a project keeps: the same behaviour as `flare stop-motion-project`,
//! from `flare_editor::stop_motion_library`, so each change is one undo step. Frames come from PNG
//! files in the export directory: no camera backend is built.

use flare_editor::stop_motion_library as library;
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::{Value, json};

use crate::color::safe_name;
use crate::ops::{apply, first_scene};
use crate::params_rigs::{
    StopMotionAddFrame, StopMotionFrame, StopMotionLay, StopMotionRevision, StopMotionStart,
};
use crate::server::{FlareServer, Outcome, State};

/// A frame's picture is refused past this size before it is read.
const MAX_PICTURE_BYTES: u64 = 64 << 20;

fn error(e: library::StopMotionLibraryError) -> String {
    e.to_string()
}

fn frames_now(state: &State) -> usize {
    state
        .session
        .document()
        .stop_motion()
        .map_or(0, |s| s.frames().len())
}

#[tool_router(router = stop_motion_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "Start the project's stop-motion session: captures a second and the hold each new frame gets; one undo step. No camera backend is built, so frames are added from PNG files with stop_motion_add_frame."
    )]
    fn stop_motion_start(&self, Parameters(a): Parameters<StopMotionStart>) -> Outcome {
        self.write(a.expected_revision, |state| {
            let command = library::start(
                state.session.document(),
                a.fps.unwrap_or(12),
                a.hold.unwrap_or(1),
            )
            .map_err(error)?;
            apply(state, vec![command])
        })
    }

    #[tool(
        description = "Add a PNG file from the export directory as the session's next frame, held `hold` timeline frames (the session's hold without it); one undo step. A picture already in the library is shared, not stored again."
    )]
    fn stop_motion_add_frame(&self, Parameters(a): Parameters<StopMotionAddFrame>) -> Outcome {
        self.write(a.expected_revision, |state| {
            // Said before the file is read, so a missing session is not blamed on the picture.
            if state.session.document().stop_motion().is_none() {
                return Err(library::StopMotionLibraryError::NotStarted.to_string());
            }
            let dir = super::export::export_dir(state)?
                .canonicalize()
                .map_err(|e| format!("cannot resolve the export directory: {e}"))?;
            let path = dir
                .join(safe_name(&a.source)?)
                .canonicalize()
                .map_err(|e| format!("cannot resolve {} in the export directory: {e}", a.source))?;
            if !path.starts_with(&dir) {
                return Err("the picture must stay inside the export directory".to_owned());
            }
            let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
            if !meta.is_file() || meta.len() > MAX_PICTURE_BYTES {
                return Err("a frame must be a PNG file of at most 64 MiB".to_owned());
            }
            let bytes =
                std::fs::read(&path).map_err(|e| format!("cannot read the picture: {e}"))?;
            let bitmap =
                flare_import::import_bitmap(&a.source, &bytes).map_err(|e| e.to_string())?;
            let commands =
                library::add_frame(state.session.document(), bitmap, a.hold).map_err(error)?;
            let mut out = apply(state, commands)?;
            out["frames"] = json!(frames_now(state));
            Ok(out)
        })
    }

    #[tool(
        description = "The stop-motion session: its rate, hold, duration in timeline frames and each frame's picture and hold."
    )]
    fn stop_motion_list(&self) -> Outcome {
        self.read(|state| {
            let session = state
                .session
                .document()
                .stop_motion()
                .ok_or_else(|| library::StopMotionLibraryError::NotStarted.to_string())?;
            let frames: Vec<Value> = session
                .frames()
                .iter()
                .enumerate()
                .map(|(i, f)| json!({ "index": i, "bitmap": f.bitmap().as_uuid(), "hold": f.hold() }))
                .collect();
            Ok(json!({
                "fps": session.fps(),
                "hold": session.default_hold(),
                "duration": session.duration(),
                "frames": frames,
            }))
        })
    }

    #[tool(
        description = "Hold a stop-motion frame (from 0) for `hold` timeline frames, 1 to 60; one undo step."
    )]
    fn stop_motion_set_hold(&self, Parameters(a): Parameters<StopMotionFrame>) -> Outcome {
        self.write(a.expected_revision, |state| {
            let hold = a.hold.ok_or("hold is needed")?;
            let command =
                library::set_hold(state.session.document(), a.index, hold).map_err(error)?;
            apply(state, vec![command])
        })
    }

    #[tool(
        description = "Take a stop-motion frame (from 0) out of the session; its picture stays in the library; one undo step."
    )]
    fn stop_motion_remove_frame(&self, Parameters(a): Parameters<StopMotionFrame>) -> Outcome {
        self.write(a.expected_revision, |state| {
            let command =
                library::remove_frame(state.session.document(), a.index).map_err(error)?;
            apply(state, vec![command])
        })
    }

    #[tool(
        description = "Lay the session's frames on a new layer of the first scene, one keyframe per picture for its hold, one after another from frame 0; one undo step."
    )]
    fn stop_motion_lay(&self, Parameters(a): Parameters<StopMotionLay>) -> Outcome {
        self.write(a.expected_revision, |state| {
            let scene = first_scene(state)?;
            let (layer, commands) = library::lay_on_timeline(
                state.session.document(),
                scene,
                a.layer_name.as_deref().unwrap_or("Stop motion"),
            )
            .map_err(error)?;
            let mut out = apply(state, commands)?;
            out["layer"] = json!(layer.as_uuid());
            Ok(out)
        })
    }

    #[tool(
        description = "End the stop-motion session; its pictures stay in the library; one undo step."
    )]
    fn stop_motion_end(&self, Parameters(a): Parameters<StopMotionRevision>) -> Outcome {
        self.write(a.expected_revision, |state| {
            let command = library::end(state.session.document()).map_err(error)?;
            apply(state, vec![command])
        })
    }
}
