//! What a menu action shows next to its name: shortcut, tool key and icon.

use super::*;

impl MenuAction {
    /// The keyboard shortcut in muda's syntax (`CmdOrCtrl+O`), or `None`. This is the one table:
    /// the native menu registers these, and the in-window menu shows them.
    ///
    /// Single letters are never shortcuts here: a native menu would take them before a text field
    /// could. Tool keys are handled by the workspace itself, only when nothing is being typed.
    pub fn shortcut(self) -> Option<&'static str> {
        Some(match self {
            Self::Open => "CmdOrCtrl+O",
            Self::Save => "CmdOrCtrl+S",
            Self::SaveAs => "CmdOrCtrl+Shift+S",
            Self::Quit => "CmdOrCtrl+Q",
            Self::Undo => "CmdOrCtrl+Z",
            Self::Redo => "CmdOrCtrl+Shift+Z",
            Self::Duplicate => "CmdOrCtrl+D",
            Self::ConvertToSymbol => "F8",
            Self::EditSymbol => "CmdOrCtrl+E",
            Self::ExitSymbol => "CmdOrCtrl+Shift+E",
            Self::CopyFrames => "CmdOrCtrl+Alt+C",
            Self::PasteFrames => "CmdOrCtrl+Alt+V",
            Self::CutFrames => "CmdOrCtrl+Alt+X",
            Self::SelectAllFrames => "CmdOrCtrl+Alt+A",
            Self::InsertFrame => "F5",
            Self::RemoveFrame => "Shift+F5",
            Self::InsertKeyframe => "F6",
            Self::ClearKeyframe => "Shift+F6",
            Self::InsertBlankKeyframe => "F7",
            Self::TestMovie => "CmdOrCtrl+Enter",
            // The stage's zoom, as in Animate. The window's own zoom is off in the workspace, so
            // these keys never scale the whole interface.
            Self::ZoomIn => "CmdOrCtrl+=",
            Self::ZoomOut => "CmdOrCtrl+-",
            // Animate's key for View > Pasteboard.
            Self::TogglePasteboard => "CmdOrCtrl+Shift+W",
            Self::Preferences => "CmdOrCtrl+,",
            _ => return None,
        })
    }

    /// The shortcut as a person reads it on this platform: `Ctrl+O`, or `\u{2318}O` on macOS.
    pub fn shortcut_text(self) -> Option<String> {
        let shortcut = self.shortcut()?;
        Some(
            shortcut
                .replace(
                    "CmdOrCtrl+",
                    if cfg!(target_os = "macos") {
                        "\u{2318}"
                    } else {
                        "Ctrl+"
                    },
                )
                .replace(
                    "Shift+",
                    if cfg!(target_os = "macos") {
                        "\u{21e7}"
                    } else {
                        "Shift+"
                    },
                )
                .replace(
                    "Alt+",
                    if cfg!(target_os = "macos") {
                        "\u{2325}"
                    } else {
                        "Alt+"
                    },
                ),
        )
    }

    /// The key that picks this tool when nothing is being typed.
    pub fn tool_key(self) -> Option<&'static str> {
        Some(match self {
            Self::ToolSelect => "V",
            Self::ToolSubselection => "A",
            Self::ToolFreeTransform => "Q",
            Self::ToolPen => "P",
            Self::ToolText => "T",
            Self::ToolHand => "H",
            Self::ToolRotateView => "D",
            Self::ToolZoom => "Z",
            Self::ToolEyedropper => "I",
            Self::ToolBucket => "K",
            Self::ToolInkBottle => "S",
            Self::ToolEraser => "E",
            Self::ToolPolyStar => "G",
            Self::ToolLasso => "L",
            Self::ToolPencil => "Y",
            Self::ToolBrush => "B",
            Self::ToolRuler => "M",
            Self::ToolMirror => "X",
            Self::ToolLine => "N",
            Self::ToolRectangle => "R",
            Self::ToolEllipse => "O",
            _ => return None,
        })
    }

    /// The icon drawn beside the menu item, if it has one.
    pub fn icon(self) -> Option<&'static str> {
        Some(match self {
            Self::Open => "file-open",
            Self::ImportSound => "sound",
            Self::New => "file-new",
            Self::Close => "close",
            Self::Revert => "rotate-ccw",
            Self::SaveAll => "file-save",
            Self::OpenRecent => "file-open",
            Self::PublishSettings => "settings",
            Self::ExportImage => "file-export",
            Self::ExportMovie => "test-movie",
            Self::ExportFla => "file-export",
            Self::Save | Self::SaveAs => "file-save",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::Copy => "copy",
            Self::Paste | Self::PasteInPlace => "paste",
            Self::Duplicate => "duplicate",
            Self::Delete => "delete",
            Self::ConvertToSymbol | Self::NewSymbol => "convert-symbol",
            Self::AddInstanceToSymbol => "duplicate",
            Self::IncrementInstances => "frame-insert",
            Self::BakeTransform => "tool-transform",
            Self::EditSymbol | Self::ExitSymbol => "symbol-graphic",
            Self::ToolSelect => "tool-selection",
            Self::ToolSubselection => "tool-subselection",
            Self::ToolFreeTransform => "tool-transform",
            Self::Group => "group",
            Self::BringToFront | Self::BringForward => "bring-front",
            Self::SendBackward | Self::SendToBack => "send-back",
            Self::Ungroup => "ungroup",
            Self::BreakApart => "break-apart",
            Self::Publish => "publish",
            Self::DocumentSettings => "settings",
            Self::Preferences => "gear",
            Self::ToolRectangle => "tool-rectangle",
            Self::ToolEllipse => "tool-oval",
            Self::ToolLine => "tool-line",
            Self::ToolPen => "tool-pen",
            Self::ToolText
            | Self::TextBold
            | Self::TextItalic
            | Self::TextLarger
            | Self::TextSmaller => "tool-text",
            Self::ToolHand => "tool-hand",
            Self::ToolRotateView => "tool-rotate-view",
            Self::ToolZoom => "tool-zoom",
            Self::ToolEyedropper => "tool-eyedropper",
            Self::ToolBucket => "tool-bucket",
            Self::ToolInkBottle => "tool-ink-bottle",
            Self::ToolEraser => "tool-eraser",
            Self::ToolPolyStar => "tool-polystar",
            Self::ToolLasso => "tool-lasso",
            Self::ToolPencil => "tool-pencil",
            Self::ToolBrush => "tool-brush",
            Self::ToolRuler => "tool-ruler",
            Self::ToolMirror | Self::MirrorCopyHorizontal | Self::MirrorCopyVertical => {
                "tool-mirror"
            }
            Self::InsertFrame => "frame-insert",
            Self::RemoveFrame => "frame-remove",
            Self::CopyFrames => "copy",
            Self::PasteFrames => "paste",
            Self::CutFrames => "cut",
            Self::SelectAllFrames => "select-all",
            Self::ReverseFrames => "frame-reverse",
            Self::InsertKeyframe | Self::ConvertToKeyframes => "frame-keyframe",
            Self::InsertBlankKeyframe => "frame-blank-keyframe",
            Self::ClearKeyframe => "frame-blank-keyframe",
            Self::ToggleMotionTween | Self::ToggleShapeTween => "frame-tween",
            Self::PlayPause => "transport-play",
            Self::TestMovie => "test-movie",
            Self::ToggleLayerVisible => "eye",
            Self::ToggleLayerLocked => "lock",
            Self::ToggleLayerMask => "layer-mask",
            Self::ToggleLayerGuide => "layer-motion-guide",
            Self::NewLayer => "layer-new",
            Self::NewLayerFolder => "layer-folder",
            Self::MoveLayerUp => "chevron-up",
            Self::MoveLayerDown => "chevron-down",
            Self::DeleteLayer => "layer-delete",
            Self::ZoomIn => "zoom-in",
            Self::ZoomOut => "zoom-out",
            Self::ZoomFit => "zoom-fit",
            Self::ToggleOnionSkin => "onion-skin",
            Self::ToggleOutlineMode => "outline-square",
            Self::TogglePasteboard => "pasteboard",
            Self::ToggleRulers => "ruler",
            Self::ToggleGuides | Self::ClearGuides => "ruler",
            Self::ToggleGrid => "grid",
            Self::ToggleSnap => "snap",
            Self::ToggleLoop => "transport-loop",
            Self::ToggleMute => "sound-mute",
            Self::FlipHorizontal => "flip-horizontal",
            Self::FlipVertical => "flip-vertical",
            Self::RotateClockwise => "rotate-cw",
            Self::RotateCounterclockwise => "rotate-ccw",
            Self::ScaleUp => "zoom-in",
            Self::ScaleDown => "zoom-out",
            Self::ResetLayout => "panel-tools",
            Self::ShowProperties => "panel-properties",
            Self::ShowLibrary => "panel-library",
            Self::ShowColor => "panel-color",
            Self::ShowAlign => "panel-align",
            Self::ShowTransform => "panel-transform",
            Self::ShowHistory => "panel-history",
            Self::ShowTimeline => "panel-timeline",
            Self::ShowOutput => "panel-tools",
            Self::ShowInfo => "panel-properties",
            Self::ShowHome => "home",
            Self::Cut => "cut",
            Self::SelectAll => "select-all",
            Self::Rewind => "transport-first",
            Self::GoToEnd => "transport-last",
            Self::StepForward => "transport-next",
            Self::StepBackward => "transport-prev",
            Self::ActualSize => "zoom-actual",
            Self::DeselectAll => return None,
            Self::About | Self::KeyboardShortcuts => return None,
            Self::Quit => return None,
        })
    }
}
