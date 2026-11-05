//! What the Properties panel is about, and the edits it holds while they are made.

use super::types::{FrameRange, Tool};

/// What the Properties panel is about. It follows the selection, as Animate's does: an object
/// selected on the stage, else frames selected in the timeline, else the tool in hand if it has
/// settings, else the document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PropsContext {
    Object,
    Frame,
    Tool,
    Document,
}

/// What was selected and in hand when a context was asked for by a command, so the request lasts
/// until the selection or the tool changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PropsKey {
    pub(super) tool: Tool,
    pub(super) selected: Option<flare_tools::ops::SpanRef>,
    pub(super) others: usize,
    pub(super) frames: Option<FrameRange>,
}

/// A change to the paint of the selected shape, kept while the color picker is open or a number is
/// typed, and applied as one undo step when it is let go.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ShapePaintEdit {
    pub(super) at: flare_tools::ops::SpanRef,
    pub(super) fill: egui::Color32,
    pub(super) fill_changed: bool,
    pub(super) outline: bool,
    pub(super) color: egui::Color32,
    pub(super) width: f32,
    pub(super) cap: flare_document::Cap,
    pub(super) join: flare_document::Join,
    pub(super) style: flare_document::StrokeStyle,
    pub(super) stroke_changed: bool,
}
