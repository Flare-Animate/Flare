//! Menu actions shared by every menu surface (the native macOS menu bar, the in-window
//! menu on other platforms). A surface asks the workspace whether an action is enabled,
//! what to call it right now, and to perform it, so behaviour cannot drift between them.

/// A user-invokable command. Variants map one-to-one onto menu items.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MenuAction {
    Open,
    Save,
    SaveAs,
    Quit,
    Undo,
    Redo,
    Delete,
    ConvertToSymbol,
    EditSymbol,
    ExitSymbol,
    ToolSelect,
    ToolRectangle,
    ToolEllipse,
    ToolLine,
    InsertFrame,
    RemoveFrame,
    InsertKeyframe,
    InsertBlankKeyframe,
    ClearKeyframe,
    ToggleMotionTween,
    PlayPause,
    ToggleLayerVisible,
    ToggleLayerLocked,
    MoveLayerUp,
    MoveLayerDown,
    DeleteLayer,
    ZoomIn,
    ZoomOut,
    ZoomFit,
}

impl MenuAction {
    pub const ALL: [MenuAction; 29] = [
        Self::Open,
        Self::Save,
        Self::SaveAs,
        Self::Quit,
        Self::Undo,
        Self::Redo,
        Self::Delete,
        Self::ConvertToSymbol,
        Self::EditSymbol,
        Self::ExitSymbol,
        Self::ToolSelect,
        Self::ToolRectangle,
        Self::ToolEllipse,
        Self::ToolLine,
        Self::InsertFrame,
        Self::RemoveFrame,
        Self::InsertKeyframe,
        Self::InsertBlankKeyframe,
        Self::ClearKeyframe,
        Self::ToggleMotionTween,
        Self::PlayPause,
        Self::ToggleLayerVisible,
        Self::ToggleLayerLocked,
        Self::MoveLayerUp,
        Self::MoveLayerDown,
        Self::DeleteLayer,
        Self::ZoomIn,
        Self::ZoomOut,
        Self::ZoomFit,
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
}

use MenuAction as A;
use MenuEntry::{Action, Separator};

/// The menu structure, shared by every menu surface: (title, entries).
pub const MENU_LAYOUT: &[(&str, &[MenuEntry])] = &[
    (
        "File",
        &[
            Action(A::Open),
            Action(A::Save),
            Action(A::SaveAs),
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
            Action(A::Delete),
            Separator,
            Action(A::ConvertToSymbol),
            Action(A::EditSymbol),
            Action(A::ExitSymbol),
        ],
    ),
    (
        "Tools",
        &[
            Action(A::ToolSelect),
            Action(A::ToolRectangle),
            Action(A::ToolEllipse),
            Action(A::ToolLine),
        ],
    ),
    (
        "Timeline",
        &[
            Action(A::InsertFrame),
            Action(A::RemoveFrame),
            Separator,
            Action(A::InsertKeyframe),
            Action(A::InsertBlankKeyframe),
            Action(A::ClearKeyframe),
            Separator,
            Action(A::ToggleMotionTween),
            Separator,
            Action(A::PlayPause),
        ],
    ),
    (
        "Layer",
        &[
            Action(A::ToggleLayerVisible),
            Action(A::ToggleLayerLocked),
            Separator,
            Action(A::MoveLayerUp),
            Action(A::MoveLayerDown),
            Separator,
            Action(A::DeleteLayer),
        ],
    ),
    (
        "View",
        &[Action(A::ZoomIn), Action(A::ZoomOut), Action(A::ZoomFit)],
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
