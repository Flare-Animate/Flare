//! Edit Multiple Frames for an agent: everything in a range of frames, on the layers that can be
//! changed, moved, scaled and turned together as one undo step (`flare_tools::multi_frame`).

use flare_tools::multi_frame::{RangeTransform, transform_range};
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::ops::{apply, first_scene};
use crate::server::{FlareServer, Outcome};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TransformFrames {
    /// The first frame of the range, counting from 0.
    pub from: u32,
    /// The last frame of the range.
    pub to: u32,
    /// Moved by this much, in stage units.
    pub dx: Option<f64>,
    pub dy: Option<f64>,
    /// Scaled about the middle of everything in the range; 1 keeps the size.
    pub scale_x: Option<f64>,
    pub scale_y: Option<f64>,
    /// Turned clockwise about the middle, in degrees.
    pub rotate_degrees: Option<f64>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[tool_router(router = multi_frame_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "Edit Multiple Frames: move, scale and turn everything in frames from..=to on the shown, unlocked layers together, about the middle of all of it, as one undo step (needs write access)."
    )]
    fn transform_frames(&self, Parameters(args): Parameters<TransformFrames>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let scene = first_scene(state)?;
            let how = RangeTransform {
                dx: args.dx.unwrap_or(0.0),
                dy: args.dy.unwrap_or(0.0),
                scale_x: args.scale_x.unwrap_or(1.0),
                scale_y: args.scale_y.unwrap_or(1.0),
                rotate_degrees: args.rotate_degrees.unwrap_or(0.0),
            };
            let commands = transform_range(state.session.document(), scene, args.from, args.to, how)
                .ok_or("nothing to transform: the frames hold nothing on a shown, unlocked layer, or the transform changes nothing (or scales to zero)")?;
            apply(state, commands)
        })
    }
}
