//! The editing operations a UI invokes, by name, in one place. Each lives in the folder of the tool
//! it belongs to; this only gathers them, so a caller need not know which tool owns which.
//!
//! Every operation is a pure function from a document and numbers to commands. Nothing here
//! touches a window or a session, so each is tested without a UI. Selections are frame spans,
//! identified by their layer and start frame (spans on a layer never overlap, so this is unique).

pub use crate::add_anchor::add_anchor;
pub use crate::arrange::{Align, align_deltas, distribute_deltas};
pub use crate::clipboard::{
    ClipItem, clip_bounds, convert_to_symbol, convert_to_symbol_as, copy_span, duplicate,
    duplicate_many, duplicate_through, duplicate_through_many, paste, paste_offset, place_instance,
};
pub use crate::common::{
    SpanRef, each_element, each_element_with, find_element, find_span, shape_bounds, span_bounds,
    spans_bounds, symbol_bounds,
};
pub use crate::convert_anchor::convert_anchor;
pub use crate::delete_anchor::delete_anchor;
pub use crate::drawing::{DrawPlan, DrawRefusal, MIN_DRAWN_FRAMES, Target, draw_shapes};
pub use crate::eraser::{delete, delete_many};
pub use crate::free_transform::{transform, transform_many, translate, translate_many};
pub use crate::grouping::{break_apart, break_apart_many, group};
pub use crate::lasso::{polygon_contains, spans_in_polygon};
pub use crate::paint::restyle;
pub use crate::playback::set_playback;
pub use crate::selection::{locate_hit, spans_at_frame};
pub use crate::stacking::{Stack, arrange_elements};
pub use crate::subselection::{anchors, move_anchor};
pub use crate::text::{break_text_apart, replace_shape};
pub use crate::tween::{set_ease, set_tween};
