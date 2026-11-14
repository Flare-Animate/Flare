//! A posable mannequin for reference, as drawing programs give an artist a wooden figure to pose:
//! a man's or a woman's build with proportions that can be changed, seventeen joints held to
//! the way a body bends, poses that come with Flare, and the figure seen from any side: shaded
//! as a solid body into a picture (`render`), or as flat outlines, back to front (`draw`). Any
//! part is posed by dragging it, with the hands and feet that are held staying where they are,
//! or by turning it about one axis on a ring, or copied from marks put on a picture of a person
//! (`copy_pose`). A view moves round it as a 3D program's does (`Camera`). It is a reference to
//! draw from, not a 3D editor: the body is built in code, with one fixed light and no file
//! beyond a small JSON form.
//!
//! Pure data and pure functions, with no window, GPU or document: the workspace shows the shaded
//! picture, `flare-tools` turns the outlines into shapes on a layer, and the command line and
//! the MCP server pose, picture and place the figure the same way.

mod body;
mod camera;
mod copy;
mod figure;
mod grab;
mod kinematics;
mod manipulate;
mod math;
mod pose;
mod raster;
mod reference;
mod render;
mod request;
mod rings;
mod shape;
mod skeleton;
mod skin;
mod view;
mod wire;

#[cfg(test)]
mod tests;

pub use body::{Body, BodyError, Proportion, Proportions};
pub use camera::{Camera, ZOOM_RANGE};
pub use copy::LEAN_SLACK;
pub use figure::{JointMark, Mannequin};
pub use grab::Grab;
pub use kinematics::Posed;
pub use manipulate::MAX_REACH_ITERATIONS;
pub use math::{Angles, Rot3, Vec3};
pub use pose::{MAX_LIFT, Pose, PoseError};
pub use reference::{
    Board, Depth, Fit, MARK_LINES, MARKED, MAX_MARK, MAX_MARKS_BYTES, Marks, MarksError,
};
pub use render::{Look, MAX_PICTURE_SIDE, Picture};
pub use request::{
    MAX_JOINT_SETTINGS, MAX_REACH, Request, catalog, parse_joint_setting, parse_reach_setting,
};
pub use rings::{Axis, RING_POINTS, Ring, RingGrab};
pub use skeleton::{JOINT_COUNT, Joint, Limits, Side};
pub use view::{
    Facing, MAX_FOCUS, MAX_FOV, MAX_PITCH, MAX_PLACEMENT_HEIGHT, Placement, Projected, Projector,
    View,
};
pub use wire::{FIGURE_VERSION, MAX_FIGURE_BYTES, Preset, WireError, preset, presets};
