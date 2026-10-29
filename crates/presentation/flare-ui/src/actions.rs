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
    pub const ALL: [MenuAction; 47] = [
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
            Separator,
            Mock("Pencil"),
            Mock("Brush"),
            Mock("Paint Bucket"),
            Mock("Eraser"),
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
}
