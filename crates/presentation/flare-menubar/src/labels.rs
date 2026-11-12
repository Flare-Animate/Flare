//! What each menu item is called right now. Most have one name; those that toggle something have
//! two, and which one shows depends on the state of the thing they toggle.

use std::sync::OnceLock;

use flare_i18n::{Language, Translator};

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
    /// Artwork off the stage shows.
    pub pasteboard_shown: bool,
    pub rulers_shown: bool,
    pub grid_shown: bool,
    pub snapping: bool,
    pub loop_playback: bool,
    pub muted: bool,
}

impl Default for LabelState {
    /// Nothing is on, except that a layer with nothing to ask is taken to be shown and the
    /// pasteboard shows.
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
            // The workspace shows the pasteboard until it is hidden, as Animate does.
            pasteboard_shown: true,
            rulers_shown: false,
            grid_shown: false,
            snapping: false,
            loop_playback: false,
            muted: false,
        }
    }
}

impl MenuAction {
    /// The key of the menu item's name for `state`, in the interface's message files. A toggling
    /// item has two keys, `-on` and `-off`, named for the state that picks them.
    pub fn label_key(self, state: &LabelState) -> &'static str {
        match self {
            Self::ToggleMotionTween => {
                if state.motion_tween {
                    "menu-item-toggle-motion-tween-on"
                } else {
                    "menu-item-toggle-motion-tween-off"
                }
            }
            Self::ToggleShapeTween => {
                if state.shape_tween {
                    "menu-item-toggle-shape-tween-on"
                } else {
                    "menu-item-toggle-shape-tween-off"
                }
            }
            Self::PlayPause => {
                if state.playing {
                    "menu-item-play-pause-on"
                } else {
                    "menu-item-play-pause-off"
                }
            }
            Self::ToggleLayerVisible => {
                if state.layer_visible {
                    "menu-item-toggle-layer-visible-on"
                } else {
                    "menu-item-toggle-layer-visible-off"
                }
            }
            Self::ToggleLayerLocked => {
                if state.layer_locked {
                    "menu-item-toggle-layer-locked-on"
                } else {
                    "menu-item-toggle-layer-locked-off"
                }
            }
            Self::ToggleLayerMask => {
                if state.layer_mask {
                    "menu-item-toggle-layer-mask-on"
                } else {
                    "menu-item-toggle-layer-mask-off"
                }
            }
            Self::ToggleLayerGuide => {
                if state.layer_guide {
                    "menu-item-toggle-layer-guide-on"
                } else {
                    "menu-item-toggle-layer-guide-off"
                }
            }
            Self::TextBold => {
                if state.text_bold {
                    "menu-item-text-bold-on"
                } else {
                    "menu-item-text-bold-off"
                }
            }
            Self::TextItalic => {
                if state.text_italic {
                    "menu-item-text-italic-on"
                } else {
                    "menu-item-text-italic-off"
                }
            }
            Self::ToggleGuides => {
                if state.guides_shown {
                    "menu-item-toggle-guides-on"
                } else {
                    "menu-item-toggle-guides-off"
                }
            }
            Self::ToggleOnionSkin => {
                if state.onion_on {
                    "menu-item-toggle-onion-skin-on"
                } else {
                    "menu-item-toggle-onion-skin-off"
                }
            }
            Self::ToggleOutlineMode => {
                if state.outline_mode {
                    "menu-item-toggle-outline-mode-on"
                } else {
                    "menu-item-toggle-outline-mode-off"
                }
            }
            Self::TogglePasteboard => {
                if state.pasteboard_shown {
                    "menu-item-toggle-pasteboard-on"
                } else {
                    "menu-item-toggle-pasteboard-off"
                }
            }
            Self::ToggleRulers => {
                if state.rulers_shown {
                    "menu-item-toggle-rulers-on"
                } else {
                    "menu-item-toggle-rulers-off"
                }
            }
            Self::ToggleGrid => {
                if state.grid_shown {
                    "menu-item-toggle-grid-on"
                } else {
                    "menu-item-toggle-grid-off"
                }
            }
            Self::ToggleSnap => {
                if state.snapping {
                    "menu-item-toggle-snap-on"
                } else {
                    "menu-item-toggle-snap-off"
                }
            }
            Self::ToggleLoop => {
                if state.loop_playback {
                    "menu-item-toggle-loop-on"
                } else {
                    "menu-item-toggle-loop-off"
                }
            }
            Self::ToggleMute => {
                if state.muted {
                    "menu-item-toggle-mute-on"
                } else {
                    "menu-item-toggle-mute-off"
                }
            }
            Self::Open => "menu-item-open",
            Self::ImportSound => "menu-item-import-sound",
            Self::NewLayer => "menu-item-new-layer",
            Self::NewLayerFolder => "menu-item-new-layer-folder",
            Self::New => "menu-item-new",
            Self::SaveAll => "menu-item-save-all",
            Self::OpenRecent => "menu-item-open-recent",
            Self::PublishSettings => "menu-item-publish-settings",
            Self::Close => "menu-item-close",
            Self::Revert => "menu-item-revert",
            Self::ExportImage => "menu-item-export-image",
            Self::ExportMovie => "menu-item-export-movie",
            Self::ExportFla => "menu-item-export-fla",
            Self::Save => "menu-item-save",
            Self::SaveAs => "menu-item-save-as",
            Self::Quit => "menu-item-quit",
            Self::Undo => "menu-item-undo",
            Self::Redo => "menu-item-redo",
            Self::Delete => "menu-item-delete",
            Self::Duplicate => "menu-item-duplicate",
            Self::Copy => "menu-item-copy",
            Self::Cut => "menu-item-cut",
            Self::SelectAll => "menu-item-select-all",
            Self::DeselectAll => "menu-item-deselect-all",
            Self::Rewind => "menu-item-rewind",
            Self::GoToEnd => "menu-item-go-to-end",
            Self::StepForward => "menu-item-step-forward",
            Self::StepBackward => "menu-item-step-backward",
            Self::ActualSize => "menu-item-actual-size",
            Self::CopyFrames => "menu-item-copy-frames",
            Self::PasteFrames => "menu-item-paste-frames",
            Self::CutFrames => "menu-item-cut-frames",
            Self::SelectAllFrames => "menu-item-select-all-frames",
            Self::ReverseFrames => "menu-item-reverse-frames",
            Self::Paste => "menu-item-paste",
            Self::PasteInPlace => "menu-item-paste-in-place",
            Self::FlipHorizontal => "menu-item-flip-horizontal",
            Self::FlipVertical => "menu-item-flip-vertical",
            Self::RotateClockwise => "menu-item-rotate-clockwise",
            Self::RotateCounterclockwise => "menu-item-rotate-counterclockwise",
            Self::ScaleUp => "menu-item-scale-up",
            Self::ScaleDown => "menu-item-scale-down",
            Self::ConvertToSymbol => "menu-item-convert-to-symbol",
            Self::Group => "menu-item-group",
            Self::BringToFront => "menu-item-bring-to-front",
            Self::BringForward => "menu-item-bring-forward",
            Self::SendBackward => "menu-item-send-backward",
            Self::SendToBack => "menu-item-send-to-back",
            Self::Ungroup => "menu-item-ungroup",
            Self::BreakApart => "menu-item-break-apart",
            Self::Publish => "menu-item-publish",
            Self::DocumentSettings => "menu-item-document-settings",
            Self::Preferences => "menu-item-preferences",
            Self::ToolSubselection => "menu-item-tool-subselection",
            Self::ToolFreeTransform => "menu-item-tool-free-transform",
            Self::EditSymbol => "menu-item-edit-symbol",
            Self::ExitSymbol => "menu-item-exit-symbol",
            Self::ToolSelect => "menu-item-tool-select",
            Self::ToolRectangle => "menu-item-tool-rectangle",
            Self::ToolEllipse => "menu-item-tool-ellipse",
            Self::ToolLine => "menu-item-tool-line",
            Self::ToolPen => "menu-item-tool-pen",
            Self::ToolText => "menu-item-tool-text",
            Self::ToolHand => "menu-item-tool-hand",
            Self::ToolZoom => "menu-item-tool-zoom",
            Self::ToolEyedropper => "menu-item-tool-eyedropper",
            Self::ToolBucket => "menu-item-tool-bucket",
            Self::ToolInkBottle => "menu-item-tool-ink-bottle",
            Self::ToolEraser => "menu-item-tool-eraser",
            Self::ToolPolyStar => "menu-item-tool-poly-star",
            Self::ToolLasso => "menu-item-tool-lasso",
            Self::ToolPencil => "menu-item-tool-pencil",
            Self::ToolBrush => "menu-item-tool-brush",
            Self::ToolRuler => "menu-item-tool-ruler",
            Self::ToolMirror => "menu-item-tool-mirror",
            Self::MirrorCopyHorizontal => "menu-item-mirror-copy-horizontal",
            Self::MirrorCopyVertical => "menu-item-mirror-copy-vertical",
            Self::ZoomIn => "menu-item-zoom-in",
            Self::ZoomOut => "menu-item-zoom-out",
            Self::ZoomFit => "menu-item-zoom-fit",
            Self::InsertFrame => "menu-item-insert-frame",
            Self::RemoveFrame => "menu-item-remove-frame",
            Self::InsertKeyframe => "menu-item-insert-keyframe",
            Self::InsertBlankKeyframe => "menu-item-insert-blank-keyframe",
            Self::ClearKeyframe => "menu-item-clear-keyframe",
            Self::ConvertToKeyframes => "menu-item-convert-to-keyframes",
            Self::MoveLayerUp => "menu-item-move-layer-up",
            Self::MoveLayerDown => "menu-item-move-layer-down",
            Self::DeleteLayer => "menu-item-delete-layer",
            Self::ResetLayout => "menu-item-reset-layout",
            Self::NewSymbol => "menu-item-new-symbol",
            Self::TextLarger => "menu-item-text-larger",
            Self::TextSmaller => "menu-item-text-smaller",
            Self::IncrementInstances => "menu-item-increment-instances",
            Self::AddInstanceToSymbol => "menu-item-add-instance-to-symbol",
            Self::BakeTransform => "menu-item-bake-transform",
            Self::ClearGuides => "menu-item-clear-guides",
            Self::About => "menu-item-about",
            Self::KeyboardShortcuts => "menu-item-keyboard-shortcuts",
            Self::ShowProperties => "menu-item-show-properties",
            Self::ShowLibrary => "menu-item-show-library",
            Self::ShowColor => "menu-item-show-color",
            Self::ShowAlign => "menu-item-show-align",
            Self::ShowTransform => "menu-item-show-transform",
            Self::ShowHistory => "menu-item-show-history",
            Self::ShowTimeline => "menu-item-show-timeline",
            Self::ShowOutput => "menu-item-show-output",
            Self::ShowInfo => "menu-item-show-info",
            Self::ShowHome => "menu-item-show-home",
            Self::TestMovie => "menu-item-test-movie",
        }
    }

    /// The menu item's name in English, for a surface with no translator and for the tests.
    pub fn label(self, state: &LabelState) -> String {
        english().text(self.label_key(state))
    }
}

/// The key of a menu's title (`File`) in the message files.
pub fn title_key(title: &str) -> String {
    slug_key("menu-title-", title)
}

/// The key of a menu inside a menu (`Import`).
pub fn submenu_key(title: &str) -> String {
    slug_key("menu-submenu-", title)
}

/// The key of a planned command that does not exist yet (`Scene`).
pub fn mock_key(name: &str) -> String {
    slug_key("menu-mock-", name)
}

/// `prefix` and the English name lowercased with its words joined by hyphens, so the layout keeps
/// its English names (they read well in tests and in code) and the files still have keys.
fn slug_key(prefix: &str, name: &str) -> String {
    let words: Vec<String> = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect();
    format!("{prefix}{}", words.join("-"))
}

fn english() -> &'static Translator {
    static ENGLISH: OnceLock<Translator> = OnceLock::new();
    ENGLISH.get_or_init(|| Translator::new(Language::English))
}
