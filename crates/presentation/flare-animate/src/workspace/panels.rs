//! The panels of the workspace, one module each: tools, properties, library, color, align and
//! transform, history and output, the timeline's toolbar, and the status bar.
//!
//! Controls that act on the document are real. Controls for features that do not exist yet are
//! drawn the same way but dimmed, with a hover note, using the mock helpers in `chrome`; each
//! is a place a later change fills in.

pub(super) const TOOL_ICON: f32 = 18.0;
pub(super) const BAR_ICON: f32 = 16.0;

mod align;
mod brush_options;
mod bucket_options;
mod color;
pub(crate) mod color_wells;
mod drawing_mode;
mod fps_field;
mod history;
mod layers_panel;
pub(crate) use motion_editor::MotionEditor;
mod info;
mod motion_editor;
mod motion_editor_ui;
mod onion_settings;
#[cfg(test)]
pub(super) use info::color_at as info_color_at;
mod character;
pub(crate) mod library;
pub(crate) mod mannequin;
pub(crate) mod mannequin_view;
mod properties;
mod properties_interp;
mod properties_object;
mod stage_bar;
mod swatches;
#[cfg(test)]
mod tests;
mod timeline_bar;
pub(crate) mod timeline_frame_buttons;
pub(super) mod tool_options_labels;
pub(crate) mod tool_pins;
pub(crate) mod tool_slots;
pub(crate) mod tools;
