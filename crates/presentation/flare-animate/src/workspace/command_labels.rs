//! What each menu item is called right now.

use crate::MenuAction;
use flare_document::{Layer, Tween};

use super::*;

impl Workspace {
    /// The menu item's current label (some toggle between two meanings).
    pub fn label(&self, action: MenuAction) -> String {
        let layer = self.active_layer();
        let text = match action {
            MenuAction::Open => "Open...",
            MenuAction::ImportSound => "Import Sound...",
            MenuAction::NewLayerFolder => "New Layer Folder",
            MenuAction::ImportProject => "Import XFL or FLA...",
            MenuAction::New => "New",
            MenuAction::SaveAll => "Save All",
            MenuAction::OpenRecent => "Open Recent...",
            MenuAction::PublishSettings => "Publish Settings...",
            MenuAction::Close => "Close",
            MenuAction::Revert => "Revert",
            MenuAction::ExportImage => "Export Image...",
            MenuAction::ExportMovie => "Export Animated GIF...",
            MenuAction::Save => "Save",
            MenuAction::SaveAs => "Save As...",
            MenuAction::Quit => "Quit Flare",
            MenuAction::Undo => "Undo",
            MenuAction::Redo => "Redo",
            MenuAction::Delete => "Delete",
            MenuAction::Duplicate => "Duplicate",
            MenuAction::Copy => "Copy",
            MenuAction::Cut => "Cut",
            MenuAction::SelectAll => "Select All",
            MenuAction::DeselectAll => "Deselect All",
            MenuAction::Rewind => "Rewind",
            MenuAction::StepForward => "Step Forward",
            MenuAction::StepBackward => "Step Backward",
            MenuAction::ActualSize => "Actual Size",
            MenuAction::CopyFrames => "Copy Frames",
            MenuAction::PasteFrames => "Paste Frames",
            MenuAction::Paste => "Paste",
            MenuAction::PasteInPlace => "Paste in Place",
            MenuAction::FlipHorizontal => "Flip Horizontal",
            MenuAction::FlipVertical => "Flip Vertical",
            MenuAction::RotateClockwise => "Rotate 90\u{b0} Clockwise",
            MenuAction::RotateCounterclockwise => "Rotate 90\u{b0} Counterclockwise",
            MenuAction::ScaleUp => "Scale Up",
            MenuAction::ScaleDown => "Scale Down",
            MenuAction::ConvertToSymbol => "Convert to Symbol",
            MenuAction::Group => "Group",
            MenuAction::Ungroup => "Ungroup",
            MenuAction::BreakApart => "Break Apart",
            MenuAction::Publish => "Publish",
            MenuAction::DocumentSettings => "Document Settings...",
            MenuAction::Preferences => "Preferences...",
            MenuAction::ToolSubselection => "Subselection",
            MenuAction::ToolFreeTransform => "Free Transform",
            MenuAction::EditSymbol => "Edit Symbol",
            MenuAction::ExitSymbol => "Exit Symbol",
            MenuAction::ToolSelect => "Select / Move",
            MenuAction::ToolRectangle => "Rectangle",
            MenuAction::ToolEllipse => "Ellipse",
            MenuAction::ToolLine => "Line",
            MenuAction::ToolPen => "Pen",
            MenuAction::ToolText => "Text",
            MenuAction::ToolHand => "Hand",
            MenuAction::ToolZoom => "Zoom",
            MenuAction::ToolEyedropper => "Eyedropper",
            MenuAction::ToolBucket => "Paint Bucket",
            MenuAction::ToolInkBottle => "Ink Bottle",
            MenuAction::ToolEraser => "Eraser",
            MenuAction::ToolPolyStar => "PolyStar",
            MenuAction::ToolLasso => "Lasso",
            MenuAction::ToolPencil => "Pencil",
            MenuAction::ToolBrush => "Brush",
            MenuAction::ToolRuler => "Ruler",
            MenuAction::ToolMirror => "Mirror",
            MenuAction::MirrorCopyHorizontal => "Mirror Copy Left-Right",
            MenuAction::MirrorCopyVertical => "Mirror Copy Top-Bottom",
            MenuAction::ZoomIn => "Zoom In",
            MenuAction::ZoomOut => "Zoom Out",
            MenuAction::ZoomFit => "Fit Stage in Window",
            MenuAction::InsertFrame => "Insert Frame",
            MenuAction::RemoveFrame => "Remove Frame",
            MenuAction::InsertKeyframe => "Insert Keyframe",
            MenuAction::InsertBlankKeyframe => "Insert Blank Keyframe",
            MenuAction::ClearKeyframe => "Clear Keyframe",
            MenuAction::ConvertToKeyframes => "Convert to Keyframes",
            MenuAction::ToggleMotionTween => {
                if self.selection_tween() == Some(Tween::Linear) {
                    "Remove Motion Tween"
                } else {
                    "Create Motion Tween"
                }
            }
            MenuAction::PlayPause => {
                if self.playing {
                    "Pause"
                } else {
                    "Play"
                }
            }
            MenuAction::ToggleLayerVisible => {
                if layer.is_none_or(Layer::is_visible) {
                    "Hide Layer"
                } else {
                    "Show Layer"
                }
            }
            MenuAction::ToggleLayerLocked => {
                if layer.is_some_and(Layer::is_locked) {
                    "Unlock Layer"
                } else {
                    "Lock Layer"
                }
            }
            MenuAction::ToggleLayerMask => {
                if layer.is_some_and(Layer::is_mask) {
                    "Remove Mask"
                } else {
                    "Mask Layer Below"
                }
            }
            MenuAction::ToggleLayerGuide => {
                if layer.is_some_and(Layer::is_guide) {
                    "Make Normal Layer"
                } else {
                    "Motion Guide for Layer Below"
                }
            }
            MenuAction::MoveLayerUp => "Move Layer Up",
            MenuAction::MoveLayerDown => "Move Layer Down",
            MenuAction::DeleteLayer => "Delete Layer",
            MenuAction::ResetLayout => "Reset Workspace Layout",
            MenuAction::NewSymbol => "New Symbol",
            MenuAction::TextBold => {
                if self.text.bold {
                    "Not Bold"
                } else {
                    "Bold"
                }
            }
            MenuAction::TextItalic => {
                if self.text.italic {
                    "Not Italic"
                } else {
                    "Italic"
                }
            }
            MenuAction::TextLarger => "Larger Text",
            MenuAction::TextSmaller => "Smaller Text",
            MenuAction::ToggleGuides => {
                if self.display.show_guides {
                    "Hide Guides"
                } else {
                    "Show Guides"
                }
            }
            MenuAction::ClearGuides => "Clear Guides",
            MenuAction::About => "About Flare",
            MenuAction::KeyboardShortcuts => "Keyboard Shortcuts",
            MenuAction::ShowProperties => "Properties",
            MenuAction::ShowLibrary => "Library",
            MenuAction::ShowColor => "Color",
            MenuAction::ShowAlign => "Align",
            MenuAction::ShowTransform => "Transform",
            MenuAction::ShowHistory => "History",
            MenuAction::ShowTimeline => "Timeline",
            MenuAction::ShowOutput => "Output",
            MenuAction::ShowInfo => "Info",
            MenuAction::ShowHome => "Home",
            MenuAction::ToggleOnionSkin => {
                if self.onion.enabled {
                    "Hide Onion Skin"
                } else {
                    "Show Onion Skin"
                }
            }
            MenuAction::ToggleOutlineMode => {
                if self.display.outline_mode {
                    "Show Artwork"
                } else {
                    "Outline Mode"
                }
            }
            MenuAction::ToggleRulers => {
                if self.display.show_rulers {
                    "Hide Rulers"
                } else {
                    "Show Rulers"
                }
            }
            MenuAction::ToggleGrid => {
                if self.display.show_grid {
                    "Hide Grid"
                } else {
                    "Show Grid"
                }
            }
            MenuAction::ToggleSnap => {
                if self.display.snap_to_grid {
                    "Snapping: On"
                } else {
                    "Snapping: Off"
                }
            }
            MenuAction::ToggleLoop => {
                if self.loop_playback {
                    "Loop Playback: On"
                } else {
                    "Loop Playback: Off"
                }
            }
            MenuAction::ToggleMute => {
                if self.muted {
                    "Unmute Sounds"
                } else {
                    "Mute Sounds"
                }
            }
            MenuAction::TestMovie => "Test Movie",
        };
        text.to_owned()
    }
}
