//! What each menu item is called right now. Most have one name; those that toggle something have
//! two, and which one shows depends on the state of the thing they toggle.

use crate::MenuAction;

/// The state the labels of the toggling items depend on, as the workspace sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LabelState {
    /// The selected frames have a motion tween, or a shape tween.
    pub motion_tween: bool,
    pub shape_tween: bool,
    pub playing: bool,
    /// The active layer is shown, locked, a mask, a motion guide.
    pub layer_visible: bool,
    pub layer_locked: bool,
    pub layer_mask: bool,
    pub layer_guide: bool,
    pub text_bold: bool,
    pub text_italic: bool,
    pub guides_shown: bool,
    pub onion_on: bool,
    pub outline_mode: bool,
    pub rulers_shown: bool,
    pub grid_shown: bool,
    pub snapping: bool,
    pub loop_playback: bool,
    pub muted: bool,
}

impl Default for LabelState {
    /// Nothing is on, except that a layer with nothing to ask is taken to be shown.
    fn default() -> Self {
        Self {
            motion_tween: false,
            shape_tween: false,
            playing: false,
            layer_visible: true,
            layer_locked: false,
            layer_mask: false,
            layer_guide: false,
            text_bold: false,
            text_italic: false,
            guides_shown: false,
            onion_on: false,
            outline_mode: false,
            rulers_shown: false,
            grid_shown: false,
            snapping: false,
            loop_playback: false,
            muted: false,
        }
    }
}

impl MenuAction {
    /// The menu item's label for `state`.
    pub fn label(self, state: &LabelState) -> &'static str {
        match self {
            Self::Open => "Open...",
            Self::ImportSound => "Import Sound...",
            Self::NewLayer => "Layer",
            Self::NewLayerFolder => "Layer Folder",
            Self::ImportProject => "Import XFL or FLA...",
            Self::New => "New",
            Self::SaveAll => "Save All",
            Self::OpenRecent => "Open Recent...",
            Self::PublishSettings => "Publish Settings...",
            Self::Close => "Close",
            Self::Revert => "Revert",
            Self::ExportImage => "Export Image...",
            Self::ExportMovie => "Export Animated GIF...",
            Self::ExportFla => "Export Flash Document (.fla)...",
            Self::Save => "Save",
            Self::SaveAs => "Save As...",
            Self::Quit => "Quit Flare",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::Delete => "Delete",
            Self::Duplicate => "Duplicate",
            Self::Copy => "Copy",
            Self::Cut => "Cut",
            Self::SelectAll => "Select All",
            Self::DeselectAll => "Deselect All",
            Self::Rewind => "Rewind",
            Self::GoToEnd => "Go to End",
            Self::StepForward => "Step Forward",
            Self::StepBackward => "Step Backward",
            Self::ActualSize => "Actual Size",
            Self::CopyFrames => "Copy Frames",
            Self::PasteFrames => "Paste Frames",
            Self::CutFrames => "Cut Frames",
            Self::SelectAllFrames => "Select All Frames",
            Self::ReverseFrames => "Reverse Frames",
            Self::Paste => "Paste",
            Self::PasteInPlace => "Paste in Place",
            Self::FlipHorizontal => "Flip Horizontal",
            Self::FlipVertical => "Flip Vertical",
            Self::RotateClockwise => "Rotate 90\u{b0} Clockwise",
            Self::RotateCounterclockwise => "Rotate 90\u{b0} Counterclockwise",
            Self::ScaleUp => "Scale Up",
            Self::ScaleDown => "Scale Down",
            Self::ConvertToSymbol => "Convert to Symbol",
            Self::Group => "Group",
            Self::Ungroup => "Ungroup",
            Self::BreakApart => "Break Apart",
            Self::Publish => "Publish",
            Self::DocumentSettings => "Document Settings...",
            Self::Preferences => "Preferences...",
            Self::ToolSubselection => "Subselection",
            Self::ToolFreeTransform => "Free Transform",
            Self::EditSymbol => "Edit Symbol",
            Self::ExitSymbol => "Exit Symbol",
            Self::ToolSelect => "Select / Move",
            Self::ToolRectangle => "Rectangle",
            Self::ToolEllipse => "Ellipse",
            Self::ToolLine => "Line",
            Self::ToolPen => "Pen",
            Self::ToolText => "Text",
            Self::ToolHand => "Hand",
            Self::ToolZoom => "Zoom",
            Self::ToolEyedropper => "Eyedropper",
            Self::ToolBucket => "Paint Bucket",
            Self::ToolInkBottle => "Ink Bottle",
            Self::ToolEraser => "Eraser",
            Self::ToolPolyStar => "PolyStar",
            Self::ToolLasso => "Lasso",
            Self::ToolPencil => "Pencil",
            Self::ToolBrush => "Brush",
            Self::ToolRuler => "Ruler",
            Self::ToolMirror => "Mirror",
            Self::MirrorCopyHorizontal => "Mirror Copy Left-Right",
            Self::MirrorCopyVertical => "Mirror Copy Top-Bottom",
            Self::ZoomIn => "Zoom In",
            Self::ZoomOut => "Zoom Out",
            Self::ZoomFit => "Fit Stage in Window",
            Self::InsertFrame => "Frame",
            Self::RemoveFrame => "Remove Frames",
            Self::InsertKeyframe => "Keyframe",
            Self::InsertBlankKeyframe => "Blank Keyframe",
            Self::ClearKeyframe => "Clear Keyframe",
            Self::ConvertToKeyframes => "Convert to Keyframes",
            Self::ToggleMotionTween => {
                if state.motion_tween {
                    "Remove Motion Tween"
                } else {
                    "Create Motion Tween"
                }
            }
            Self::ToggleShapeTween => {
                if state.shape_tween {
                    "Remove Shape Tween"
                } else {
                    "Create Shape Tween"
                }
            }
            Self::PlayPause => {
                if state.playing {
                    "Pause"
                } else {
                    "Play"
                }
            }
            Self::ToggleLayerVisible => {
                if state.layer_visible {
                    "Hide Layer"
                } else {
                    "Show Layer"
                }
            }
            Self::ToggleLayerLocked => {
                if state.layer_locked {
                    "Unlock Layer"
                } else {
                    "Lock Layer"
                }
            }
            Self::ToggleLayerMask => {
                if state.layer_mask {
                    "Remove Mask"
                } else {
                    "Mask Layer Below"
                }
            }
            Self::ToggleLayerGuide => {
                if state.layer_guide {
                    "Make Normal Layer"
                } else {
                    "Motion Guide for Layer Below"
                }
            }
            Self::MoveLayerUp => "Move Layer Up",
            Self::MoveLayerDown => "Move Layer Down",
            Self::DeleteLayer => "Delete Layer",
            Self::ResetLayout => "Reset Workspace Layout",
            Self::NewSymbol => "New Symbol",
            Self::TextBold => {
                if state.text_bold {
                    "Not Bold"
                } else {
                    "Bold"
                }
            }
            Self::TextItalic => {
                if state.text_italic {
                    "Not Italic"
                } else {
                    "Italic"
                }
            }
            Self::TextLarger => "Larger Text",
            Self::TextSmaller => "Smaller Text",
            Self::ToggleGuides => {
                if state.guides_shown {
                    "Hide Guides"
                } else {
                    "Show Guides"
                }
            }
            Self::ClearGuides => "Clear Guides",
            Self::About => "About Flare",
            Self::KeyboardShortcuts => "Keyboard Shortcuts",
            Self::ShowProperties => "Properties",
            Self::ShowLibrary => "Library",
            Self::ShowColor => "Color",
            Self::ShowAlign => "Align",
            Self::ShowTransform => "Transform",
            Self::ShowHistory => "History",
            Self::ShowTimeline => "Timeline",
            Self::ShowOutput => "Output",
            Self::ShowInfo => "Info",
            Self::ShowHome => "Home",
            Self::ToggleOnionSkin => {
                if state.onion_on {
                    "Hide Onion Skin"
                } else {
                    "Show Onion Skin"
                }
            }
            Self::ToggleOutlineMode => {
                if state.outline_mode {
                    "Show Artwork"
                } else {
                    "Outline Mode"
                }
            }
            Self::ToggleRulers => {
                if state.rulers_shown {
                    "Hide Rulers"
                } else {
                    "Show Rulers"
                }
            }
            Self::ToggleGrid => {
                if state.grid_shown {
                    "Hide Grid"
                } else {
                    "Show Grid"
                }
            }
            Self::ToggleSnap => {
                if state.snapping {
                    "Snapping: On"
                } else {
                    "Snapping: Off"
                }
            }
            Self::ToggleLoop => {
                if state.loop_playback {
                    "Loop Playback: On"
                } else {
                    "Loop Playback: Off"
                }
            }
            Self::ToggleMute => {
                if state.muted {
                    "Unmute Sounds"
                } else {
                    "Mute Sounds"
                }
            }
            Self::TestMovie => "Test Movie",
        }
    }
}
