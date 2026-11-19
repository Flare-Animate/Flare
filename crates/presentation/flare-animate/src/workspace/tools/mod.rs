//! What the workspace does for each tool: wiring pointer events and settings to what the tool
//! computes in `flare-tools`. The folders there and the files here go by the same names.

pub(super) mod anchors;
pub(super) mod blend;
pub(super) mod bone;
pub(super) mod brush;
pub(super) mod camera;
pub(super) mod eraser;
pub(super) mod hand;
pub(super) mod lasso;
pub(super) mod magic_wand;
pub(super) mod magnet;
pub(super) mod paint;
pub(super) mod paint_pointer;
pub(super) mod pen;
pub(super) mod preview_mesh;
pub(super) mod rotate_view;
pub(super) mod stage_pointer;
pub(super) mod stroke_preview;
pub(super) mod text;
pub(super) mod text_fonts;
pub(super) mod text_preview;

/// The state of the tools that act on a drag and then let go: the Hand, which gives back the
/// tool before it, and the Magnet's pull.
#[derive(Default)]
pub(in crate::workspace) struct Springs {
    pub(in crate::workspace) hand: hand::HandSpring,
    pub(in crate::workspace) magnet: magnet::MagnetState,
}
