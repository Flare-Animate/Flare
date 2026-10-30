//! The menu bar's structure, shared by the in-window menu and the native one.

use super::MenuAction;

/// One entry in a menu: an action or a separator line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuEntry {
    Action(MenuAction),
    Separator,
    /// A command that is planned but does not exist yet: shown dimmed, does nothing.
    Mock(&'static str),
    /// A menu inside the menu, as Animate's Import, Export, Transform and Timeline are.
    Submenu(&'static str, &'static [MenuEntry]),
}

use MenuAction as A;
use MenuEntry::{Action, Mock, Separator, Submenu};

/// The menu structure, shared by every menu surface: (title, entries).
///
/// The titles and where commands sit follow Animate's menu bar (File, Edit, View, Insert, Modify,
/// Text, Commands, Control, Window, Help; Debug is for ActionScript and is left out), so a person
/// who knows Animate finds things where they expect them. `Mock` entries are commands that are
/// planned but do not exist yet. They are shown dimmed so the menus already have their final shape;
/// each becomes an `Action` when its feature lands.
pub const MENU_LAYOUT: &[(&str, &[MenuEntry])] = &[
    (
        "File",
        &[
            Action(A::New),
            Action(A::Open),
            Action(A::OpenRecent),
            Action(A::Close),
            Separator,
            Action(A::Save),
            Action(A::SaveAs),
            Action(A::SaveAll),
            Action(A::Revert),
            Separator,
            Submenu(
                "Import",
                &[Action(A::ImportSound), Action(A::ImportProject)],
            ),
            Submenu("Export", &[Action(A::ExportImage), Action(A::ExportMovie)]),
            Separator,
            Action(A::PublishSettings),
            Action(A::Publish),
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
            Action(A::Cut),
            Action(A::Copy),
            Action(A::Paste),
            Action(A::PasteInPlace),
            Action(A::Duplicate),
            Action(A::Delete),
            Separator,
            Action(A::SelectAll),
            Action(A::DeselectAll),
            Separator,
            Submenu(
                "Timeline",
                &[
                    Action(A::RemoveFrame),
                    Action(A::CopyFrames),
                    Action(A::PasteFrames),
                ],
            ),
            Separator,
            Action(A::EditSymbol),
            Action(A::ExitSymbol),
            Separator,
            Action(A::Preferences),
            Mock("Keyboard Shortcuts..."),
        ],
    ),
    (
        "View",
        &[
            Action(A::ZoomIn),
            Action(A::ZoomOut),
            Action(A::ZoomFit),
            Action(A::ActualSize),
            Separator,
            Action(A::ToggleRulers),
            Action(A::ToggleGrid),
            Action(A::ToggleGuides),
            Action(A::ClearGuides),
            Action(A::ToggleSnap),
            Separator,
            Action(A::ToggleOnionSkin),
            Action(A::ToggleOutlineMode),
            Separator,
            Action(A::ResetLayout),
        ],
    ),
    (
        "Insert",
        &[
            Action(A::NewSymbol),
            Separator,
            Submenu(
                "Timeline",
                &[
                    Action(A::NewLayer),
                    Action(A::NewLayerFolder),
                    Action(A::InsertFrame),
                    Action(A::InsertKeyframe),
                    Action(A::InsertBlankKeyframe),
                ],
            ),
            Separator,
            Action(A::ToggleMotionTween),
            Action(A::ToggleShapeTween),
            Separator,
            Mock("Scene"),
        ],
    ),
    (
        "Modify",
        &[
            Action(A::DocumentSettings),
            Separator,
            Action(A::ConvertToSymbol),
            Action(A::BreakApart),
            Separator,
            Submenu(
                "Timeline",
                &[
                    Action(A::ToggleLayerVisible),
                    Action(A::ToggleLayerLocked),
                    Action(A::ToggleLayerMask),
                    Action(A::ToggleLayerGuide),
                    Separator,
                    Action(A::MoveLayerUp),
                    Action(A::MoveLayerDown),
                    Action(A::DeleteLayer),
                    Separator,
                    Action(A::ClearKeyframe),
                    Action(A::ConvertToKeyframes),
                ],
            ),
            Submenu(
                "Transform",
                &[
                    Action(A::FlipHorizontal),
                    Action(A::FlipVertical),
                    Separator,
                    Action(A::RotateClockwise),
                    Action(A::RotateCounterclockwise),
                    Action(A::ScaleUp),
                    Action(A::ScaleDown),
                    Separator,
                    Action(A::MirrorCopyHorizontal),
                    Action(A::MirrorCopyVertical),
                ],
            ),
            Mock("Arrange"),
            Mock("Align"),
            Separator,
            Action(A::Group),
            Action(A::Ungroup),
        ],
    ),
    (
        "Text",
        &[
            Action(A::TextBold),
            Action(A::TextItalic),
            Separator,
            Action(A::TextLarger),
            Action(A::TextSmaller),
            Separator,
            Mock("Font..."),
            Mock("Align"),
            Mock("Check Spelling"),
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
        "Control",
        &[
            Action(A::PlayPause),
            Action(A::Rewind),
            Action(A::StepForward),
            Action(A::StepBackward),
            Separator,
            Action(A::TestMovie),
            Separator,
            Action(A::ToggleLoop),
            Action(A::ToggleMute),
        ],
    ),
    (
        "Window",
        &[
            Action(A::ShowTimeline),
            Action(A::ShowProperties),
            Action(A::ShowLibrary),
            Action(A::ShowColor),
            Action(A::ShowAlign),
            Action(A::ShowTransform),
            Action(A::ShowHistory),
            Action(A::ShowInfo),
            Action(A::ShowOutput),
            Separator,
            Mock("Workspace Presets"),
        ],
    ),
    (
        "Help",
        &[
            Mock("Flare Help"),
            Action(A::KeyboardShortcuts),
            Separator,
            Action(A::About),
        ],
    ),
];

/// The commands that have no menu item, because Animate has none for them either: the tools are
/// picked from the tool strip or by their keys, and Home is the house in the top bar.
pub const OFF_MENU: &[MenuAction] = &[
    A::ToolSelect,
    A::ToolSubselection,
    A::ToolFreeTransform,
    A::ToolRectangle,
    A::ToolEllipse,
    A::ToolLine,
    A::ToolPen,
    A::ToolText,
    A::ToolHand,
    A::ToolZoom,
    A::ToolEyedropper,
    A::ToolBucket,
    A::ToolInkBottle,
    A::ToolEraser,
    A::ToolPolyStar,
    A::ToolLasso,
    A::ToolPencil,
    A::ToolBrush,
    A::ToolRuler,
    A::ToolMirror,
    A::ShowHome,
];

/// Every entry of every menu, submenus opened, in order.
pub fn all_entries() -> Vec<MenuEntry> {
    fn walk(entries: &[MenuEntry], out: &mut Vec<MenuEntry>) {
        for entry in entries {
            out.push(*entry);
            if let MenuEntry::Submenu(_, inner) = entry {
                walk(inner, out);
            }
        }
    }
    let mut out = Vec::new();
    for (_, entries) in MENU_LAYOUT {
        walk(entries, &mut out);
    }
    out
}
