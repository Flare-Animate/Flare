//! Menu actions shared by every menu surface (the native macOS menu bar, the in-window
//! menu on other platforms). A surface asks the workspace whether an action is enabled,
//! what to call it right now, and to perform it, so behaviour cannot drift between them.

/// A user-invokable command. Variants map one-to-one onto menu items.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MenuAction {
    Open,
    ImportSound,
    Save,
    SaveAs,
    Quit,
    Undo,
    Redo,
    Delete,
    Copy,
    Paste,
    Duplicate,
    ConvertToSymbol,
    EditSymbol,
    ExitSymbol,
    ToolSelect,
    ToolRectangle,
    ToolEllipse,
    ToolLine,
    ToolPen,
    ToolText,
    ToolHand,
    ToolZoom,
    ToolEyedropper,
    ToolBucket,
    ToolInkBottle,
    ToolEraser,
    ToolPolyStar,
    InsertFrame,
    CopyFrames,
    PasteFrames,
    RemoveFrame,
    InsertKeyframe,
    InsertBlankKeyframe,
    ClearKeyframe,
    ToggleMotionTween,
    PlayPause,
    ToggleLayerVisible,
    ToggleLayerLocked,
    ToggleLayerMask,
    MoveLayerUp,
    MoveLayerDown,
    DeleteLayer,
    ResetLayout,
    ToggleOnionSkin,
    TestMovie,
    ZoomIn,
    ZoomOut,
    ZoomFit,
    FlipHorizontal,
    FlipVertical,
    RotateClockwise,
    RotateCounterclockwise,
    ScaleUp,
    ScaleDown,
}

impl MenuAction {
    pub const ALL: [MenuAction; 54] = [
        Self::Open,
        Self::ImportSound,
        Self::Save,
        Self::SaveAs,
        Self::Quit,
        Self::Undo,
        Self::Redo,
        Self::Delete,
        Self::Copy,
        Self::Paste,
        Self::Duplicate,
        Self::ConvertToSymbol,
        Self::EditSymbol,
        Self::ExitSymbol,
        Self::ToolSelect,
        Self::ToolRectangle,
        Self::ToolEllipse,
        Self::ToolLine,
        Self::ToolPen,
        Self::ToolText,
        Self::ToolHand,
        Self::ToolZoom,
        Self::ToolEyedropper,
        Self::ToolBucket,
        Self::ToolInkBottle,
        Self::ToolEraser,
        Self::ToolPolyStar,
        Self::InsertFrame,
        Self::CopyFrames,
        Self::PasteFrames,
        Self::RemoveFrame,
        Self::InsertKeyframe,
        Self::InsertBlankKeyframe,
        Self::ClearKeyframe,
        Self::ToggleMotionTween,
        Self::PlayPause,
        Self::ToggleLayerVisible,
        Self::ToggleLayerLocked,
        Self::ToggleLayerMask,
        Self::MoveLayerUp,
        Self::MoveLayerDown,
        Self::DeleteLayer,
        Self::ResetLayout,
        Self::ToggleOnionSkin,
        Self::TestMovie,
        Self::ZoomIn,
        Self::ZoomOut,
        Self::ZoomFit,
        Self::FlipHorizontal,
        Self::FlipVertical,
        Self::RotateClockwise,
        Self::RotateCounterclockwise,
        Self::ScaleUp,
        Self::ScaleDown,
    ];

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
            Self::InsertFrame => "F5",
            Self::RemoveFrame => "Shift+F5",
            Self::InsertKeyframe => "F6",
            Self::ClearKeyframe => "Shift+F6",
            Self::InsertBlankKeyframe => "F7",
            Self::TestMovie => "CmdOrCtrl+Enter",
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
            Self::ToolPen => "P",
            Self::ToolText => "T",
            Self::ToolHand => "H",
            Self::ToolZoom => "Z",
            Self::ToolEyedropper => "I",
            Self::ToolBucket => "K",
            Self::ToolInkBottle => "S",
            Self::ToolEraser => "E",
            Self::ToolPolyStar => "G",
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
            Self::Save | Self::SaveAs => "file-save",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::Copy => "copy",
            Self::Paste => "paste",
            Self::Duplicate => "duplicate",
            Self::Delete => "delete",
            Self::ConvertToSymbol => "convert-symbol",
            Self::EditSymbol | Self::ExitSymbol => "symbol-graphic",
            Self::ToolSelect => "tool-selection",
            Self::ToolRectangle => "tool-rectangle",
            Self::ToolEllipse => "tool-oval",
            Self::ToolLine => "tool-line",
            Self::ToolPen => "tool-pen",
            Self::ToolText => "tool-text",
            Self::ToolHand => "tool-hand",
            Self::ToolZoom => "tool-zoom",
            Self::ToolEyedropper => "tool-eyedropper",
            Self::ToolBucket => "tool-bucket",
            Self::ToolInkBottle => "tool-ink-bottle",
            Self::ToolEraser => "tool-eraser",
            Self::ToolPolyStar => "tool-polystar",
            Self::InsertFrame => "frame-insert",
            Self::RemoveFrame => "frame-remove",
            Self::CopyFrames => "copy",
            Self::PasteFrames => "paste",
            Self::InsertKeyframe => "frame-keyframe",
            Self::InsertBlankKeyframe => "frame-blank-keyframe",
            Self::ClearKeyframe => "frame-blank-keyframe",
            Self::ToggleMotionTween => "frame-tween",
            Self::PlayPause => "transport-play",
            Self::TestMovie => "test-movie",
            Self::ToggleLayerVisible => "eye",
            Self::ToggleLayerLocked => "lock",
            Self::ToggleLayerMask => "layer-mask",
            Self::MoveLayerUp => "chevron-up",
            Self::MoveLayerDown => "chevron-down",
            Self::DeleteLayer => "layer-delete",
            Self::ZoomIn => "zoom-in",
            Self::ZoomOut => "zoom-out",
            Self::ZoomFit => "zoom-fit",
            Self::ToggleOnionSkin => "onion-skin",
            Self::FlipHorizontal => "flip-horizontal",
            Self::FlipVertical => "flip-vertical",
            Self::RotateClockwise => "rotate-cw",
            Self::RotateCounterclockwise => "rotate-ccw",
            Self::ScaleUp => "zoom-in",
            Self::ScaleDown => "zoom-out",
            Self::ResetLayout => "panel-tools",
            Self::Quit => return None,
        })
    }

    /// A stable identifier for native menu items (the variant name).
    pub fn id(self) -> String {
        format!("{self:?}")
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.id() == id)
    }
}

/// One entry in a menu: an action or a separator line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuEntry {
    Action(MenuAction),
    Separator,
    /// A command that is planned but does not exist yet: shown dimmed, does nothing.
    Mock(&'static str),
}

use MenuAction as A;
use MenuEntry::{Action, Mock, Separator};

/// The menu structure, shared by every menu surface: (title, entries).
///
/// `Mock` entries are commands that are planned but do not exist yet. They are shown dimmed so the
/// menus already have their final shape; each becomes an `Action` when its feature lands.
pub const MENU_LAYOUT: &[(&str, &[MenuEntry])] = &[
    (
        "File",
        &[
            Mock("New..."),
            Action(A::Open),
            Mock("Open Recent"),
            Mock("Close"),
            Separator,
            Action(A::Save),
            Action(A::SaveAs),
            Mock("Save All"),
            Mock("Revert"),
            Separator,
            Action(A::ImportSound),
            Mock("Import to Stage..."),
            Mock("Import to Library..."),
            Separator,
            Mock("Export Image..."),
            Mock("Export Movie..."),
            Mock("Publish Settings..."),
            Mock("Publish"),
            Separator,
            Action(A::Quit),
        ],
    ),
    (
        "Edit",
        &[
            Action(A::Undo),
            Action(A::Redo),
            Separator,
            Mock("Cut"),
            Action(A::Copy),
            Action(A::Paste),
            Mock("Paste in Place"),
            Action(A::Duplicate),
            Action(A::Delete),
            Separator,
            Mock("Select All"),
            Mock("Deselect All"),
            Separator,
            Action(A::EditSymbol),
            Action(A::ExitSymbol),
            Separator,
            Mock("Preferences..."),
            Mock("Keyboard Shortcuts..."),
        ],
    ),
    (
        "View",
        &[
            Action(A::ZoomIn),
            Action(A::ZoomOut),
            Action(A::ZoomFit),
            Mock("Actual Size"),
            Separator,
            Mock("Rulers"),
            Mock("Grid"),
            Mock("Guides"),
            Mock("Snapping"),
            Separator,
            Action(A::ToggleOnionSkin),
            Mock("Outline Mode"),
            Separator,
            Action(A::ResetLayout),
        ],
    ),
    (
        "Insert",
        &[
            Mock("New Symbol..."),
            Separator,
            Action(A::InsertFrame),
            Action(A::InsertKeyframe),
            Action(A::InsertBlankKeyframe),
            Separator,
            Mock("Scene"),
            Mock("Layer Folder"),
        ],
    ),
    (
        "Modify",
        &[
            Action(A::ConvertToSymbol),
            Mock("Break Apart"),
            Separator,
            Action(A::FlipHorizontal),
            Action(A::FlipVertical),
            Action(A::RotateClockwise),
            Action(A::RotateCounterclockwise),
            Action(A::ScaleUp),
            Action(A::ScaleDown),
            Separator,
            Mock("Arrange"),
            Mock("Align"),
            Mock("Group"),
            Mock("Ungroup"),
            Separator,
            Mock("Document Settings..."),
        ],
    ),
    (
        "Text",
        &[
            Mock("Font"),
            Mock("Size"),
            Mock("Style"),
            Mock("Align"),
            Separator,
            Mock("Check Spelling"),
        ],
    ),
    (
        "Layer",
        &[
            Action(A::ToggleLayerVisible),
            Action(A::ToggleLayerLocked),
            Action(A::ToggleLayerMask),
            Mock("Guide Layer"),
            Separator,
            Action(A::MoveLayerUp),
            Action(A::MoveLayerDown),
            Separator,
            Action(A::DeleteLayer),
        ],
    ),
    (
        "Timeline",
        &[
            Action(A::RemoveFrame),
            Action(A::CopyFrames),
            Action(A::PasteFrames),
            Separator,
            Action(A::ClearKeyframe),
            Mock("Convert to Keyframes"),
            Separator,
            Action(A::ToggleMotionTween),
            Mock("Create Shape Tween"),
        ],
    ),
    (
        "Control",
        &[
            Action(A::PlayPause),
            Mock("Rewind"),
            Mock("Step Forward"),
            Mock("Step Backward"),
            Separator,
            Action(A::TestMovie),
            Mock("Loop Playback"),
            Mock("Mute Sounds"),
        ],
    ),
    (
        "Tools",
        &[
            Action(A::ToolSelect),
            Action(A::ToolPen),
            Action(A::ToolText),
            Action(A::ToolLine),
            Action(A::ToolRectangle),
            Action(A::ToolEllipse),
            Action(A::ToolPolyStar),
            Separator,
            Mock("Pencil"),
            Mock("Brush"),
            Separator,
            Action(A::ToolBucket),
            Action(A::ToolInkBottle),
            Action(A::ToolEyedropper),
            Action(A::ToolEraser),
            Separator,
            Action(A::ToolHand),
            Action(A::ToolZoom),
        ],
    ),
    (
        "Commands",
        &[
            Mock("Run Command..."),
            Mock("Manage Saved Commands..."),
            Mock("Get More Commands..."),
        ],
    ),
    (
        "Window",
        &[
            Mock("Properties"),
            Mock("Library"),
            Mock("Color"),
            Mock("Align"),
            Mock("Transform"),
            Mock("History"),
            Mock("Timeline"),
            Separator,
            Mock("Workspace Presets"),
        ],
    ),
    (
        "Help",
        &[
            Mock("Flare Help"),
            Mock("Keyboard Shortcuts"),
            Separator,
            Mock("About Flare"),
        ],
    ),
];

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn every_action_appears_in_the_layout_exactly_once() {
        let mut seen = Vec::new();
        for (_, entries) in MENU_LAYOUT {
            for entry in *entries {
                if let Action(action) = entry {
                    seen.push(*action);
                }
            }
        }
        for action in MenuAction::ALL {
            assert_eq!(
                seen.iter().filter(|a| **a == action).count(),
                1,
                "{action:?}"
            );
        }
        assert_eq!(seen.len(), MenuAction::ALL.len());
    }

    #[test]
    fn ids_round_trip_and_all_is_complete() {
        for action in MenuAction::ALL {
            assert_eq!(MenuAction::from_id(&action.id()), Some(action));
        }
        assert_eq!(MenuAction::from_id("NoSuchAction"), None);
    }

    #[test]
    fn shortcuts_are_unique_never_a_bare_letter_and_read_naturally() {
        let mut seen = std::collections::HashSet::new();
        for action in MenuAction::ALL {
            if let Some(shortcut) = action.shortcut() {
                assert!(seen.insert(shortcut), "{shortcut} is used twice");
                let key = shortcut.rsplit('+').next().unwrap_or_default();
                let bare = !shortcut.contains('+');
                assert!(
                    !(bare && key.len() == 1),
                    "{action:?}: a bare letter {shortcut}"
                );
                let text = action.shortcut_text().expect("text");
                assert!(!text.contains("CmdOrCtrl"), "{text}");
            }
        }
        assert!(MenuAction::Open.shortcut_text().is_some());
        assert!(MenuAction::Delete.shortcut().is_none());
    }

    #[test]
    fn tool_keys_are_unique_letters_and_every_menu_icon_exists() {
        let mut keys = std::collections::HashSet::new();
        for action in MenuAction::ALL {
            if let Some(key) = action.tool_key() {
                assert!(keys.insert(key), "{key} is used twice");
                assert_eq!(key.len(), 1);
            }
            if let Some(icon) = action.icon() {
                assert!(
                    crate::icons::exists(icon),
                    "{action:?} names a missing icon {icon}"
                );
            }
        }
        assert!(keys.len() >= 6);
    }
}
