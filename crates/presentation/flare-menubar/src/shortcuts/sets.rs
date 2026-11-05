//! The named shortcut sets, as data. `flare` is the built-in table (`MenuAction::shortcut` and
//! `tool_key`); each other set is a short list of what it moves, in the spirit of the tool's own
//! presets (OpenToonz `shortcuts/*.ini`, Animate's defaults, Tahoma2D's). The ids are the ones
//! `flare-profile` lets a profile name.

use crate::MenuAction as A;
use crate::MenuAction;

/// The set a new install starts with.
pub const DEFAULT_SET: &str = "flare";

/// Every set, `flare-profile`'s `SHORTCUT_SETS` among them.
pub const SHORTCUT_SET_IDS: [&str; 4] = ["flare", "opentoonz", "animate", "tahoma"];

const OPENTOONZ: &[(MenuAction, &str)] = &[
    (A::ToolSelect, "S"),
    (A::ToolSubselection, "A"),
    (A::ToolInkBottle, "U"),
    (A::ToolBucket, "F"),
    (A::ToolEyedropper, "K"),
    (A::ToolText, "Y"),
    (A::ToolPencil, "J"),
    (A::Redo, "CmdOrCtrl+Y"),
];

const ANIMATE: &[(MenuAction, &str)] = &[(A::Redo, "CmdOrCtrl+Y")];

const TAHOMA: &[(MenuAction, &str)] = &[
    (A::ToolSelect, "S"),
    (A::ToolSubselection, "A"),
    (A::ToolInkBottle, "U"),
    (A::ToolBucket, "G"),
    (A::ToolPolyStar, "W"),
    (A::ToolPencil, "J"),
    (A::Redo, "CmdOrCtrl+Y"),
];

/// The bindings of a set; an unknown id gives none.
pub fn set_bindings(id: &str) -> Vec<(MenuAction, &'static str)> {
    let moves: &[(MenuAction, &str)] = match id {
        "flare" => &[],
        "opentoonz" => OPENTOONZ,
        "animate" => ANIMATE,
        "tahoma" => TAHOMA,
        _ => return Vec::new(),
    };
    let mut out: Vec<(MenuAction, &'static str)> = MenuAction::ALL
        .into_iter()
        .filter_map(|a| a.shortcut().or_else(|| a.tool_key()).map(|k| (a, k)))
        .collect();
    for (action, key) in moves {
        match out.iter_mut().find(|(a, _)| a == action) {
            Some(slot) => slot.1 = key,
            None => out.push((*action, key)),
        }
    }
    out
}
