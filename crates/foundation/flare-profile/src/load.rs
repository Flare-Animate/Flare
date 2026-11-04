//! Reads a profile and checks it. A profile file is untrusted: it is size-capped, its trees are
//! bounded in depth, every number is in range, and it must place every panel. A profile that
//! fails any check is refused whole.

use std::collections::BTreeSet;

use crate::ProfileError;
use crate::model::{DockNode, PanelId, Profile};

/// The only file format version read.
pub const SCHEMA: u32 = 1;
/// Largest profile file accepted, in bytes.
pub const MAX_PROFILE_BYTES: usize = 256 * 1024;
pub const MAX_ROOMS: usize = 16;
pub const MAX_TOOLBARS: usize = 32;
/// Deepest a dock tree may nest; a room has at most 28 panels, so this is ample.
pub const MAX_DEPTH: usize = 16;
/// Longest id, title or name, in characters.
pub const MAX_TEXT: usize = 64;
const MIN_FRACTION: f32 = 0.05;
const MAX_FRACTION: f32 = 0.95;
/// The shortcut sets a profile may name (the `def*.ini` and `ot*.ini` presets in `original/`).
pub const SHORTCUT_SETS: [&str; 3] = ["flare", "opentoonz", "animate"];
/// The themes a profile may name, as `flare_ui::theme::Mode::code` writes them.
pub const THEMES: [&str; 3] = ["dark", "light", "liquidglass"];

/// Reads and checks one profile. Nothing is returned unless every check passes.
pub fn parse(json: &str) -> Result<Profile, ProfileError> {
    if json.len() > MAX_PROFILE_BYTES {
        return Err(ProfileError::TooLarge { bytes: json.len() });
    }
    let profile: Profile =
        serde_json::from_str(json).map_err(|e| ProfileError::Parse(e.to_string()))?;
    validate(&profile)?;
    Ok(profile)
}

/// Checks a profile that is already in memory, with the same rules `parse` applies.
pub fn validate(profile: &Profile) -> Result<(), ProfileError> {
    if profile.schema != SCHEMA {
        return Err(ProfileError::Schema(profile.schema));
    }
    check_name("profile id", &profile.id)?;
    check_title(&profile.title)?;
    check_one_of("shortcut set", &profile.shortcut_set, &SHORTCUT_SETS)?;
    check_one_of("theme", &profile.theme, &THEMES)?;
    if profile.toolbars.len() > MAX_TOOLBARS {
        return Err(ProfileError::TooMany {
            what: "toolbars",
            max: MAX_TOOLBARS,
        });
    }
    for toolbar in &profile.toolbars {
        check_name("toolbar id", &toolbar.id)?;
    }
    if profile.rooms.is_empty() {
        return Err(ProfileError::NoRooms);
    }
    if profile.rooms.len() > MAX_ROOMS {
        return Err(ProfileError::TooMany {
            what: "rooms",
            max: MAX_ROOMS,
        });
    }
    if profile.default_room >= profile.rooms.len() {
        return Err(ProfileError::DefaultRoom(profile.default_room));
    }

    let mut room_ids = BTreeSet::new();
    let mut covered = BTreeSet::new();
    for room in &profile.rooms {
        check_name("room id", &room.id)?;
        check_title(&room.title)?;
        if !room_ids.insert(room.id.as_str()) {
            return Err(ProfileError::Duplicate {
                what: "room id",
                value: room.id.clone(),
            });
        }
        let mut panels = BTreeSet::new();
        check_node(&room.layout, 1, &mut panels)?;
        covered.extend(panels);
    }
    let missing: Vec<PanelId> = PanelId::ALL
        .iter()
        .copied()
        .filter(|p| !covered.contains(p))
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(ProfileError::MissingPanels(missing))
    }
}

/// Checks one room's tree and collects its panels. A panel may be in one room once.
fn check_node(
    node: &DockNode,
    depth: usize,
    panels: &mut BTreeSet<PanelId>,
) -> Result<(), ProfileError> {
    if depth > MAX_DEPTH {
        return Err(ProfileError::TooDeep);
    }
    match node {
        DockNode::Split {
            first_fraction,
            first,
            second,
            ..
        } => {
            // NaN fails the range test too, so it is refused with the rest.
            if !(MIN_FRACTION..=MAX_FRACTION).contains(first_fraction) {
                return Err(ProfileError::Fraction(*first_fraction));
            }
            check_node(first, depth + 1, panels)?;
            check_node(second, depth + 1, panels)
        }
        DockNode::Stack {
            panels: tabs,
            active,
        } => {
            if tabs.is_empty() {
                return Err(ProfileError::EmptyStack);
            }
            if *active >= tabs.len() {
                return Err(ProfileError::ActiveTab {
                    active: *active,
                    len: tabs.len(),
                });
            }
            for tab in tabs {
                if !panels.insert(*tab) {
                    return Err(ProfileError::Duplicate {
                        what: "panel",
                        value: format!("{tab:?}"),
                    });
                }
            }
            Ok(())
        }
    }
}

/// Ids are short lowercase words with digits and hyphens, so they can be used in file names.
fn check_name(what: &'static str, value: &str) -> Result<(), ProfileError> {
    let ok = !value.is_empty()
        && value.len() <= MAX_TEXT
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if ok {
        Ok(())
    } else {
        Err(ProfileError::BadText {
            what,
            value: value.to_owned(),
        })
    }
}

/// Titles are shown to people: any text without control characters, not blank.
fn check_title(value: &str) -> Result<(), ProfileError> {
    let ok = !value.trim().is_empty()
        && value.chars().count() <= MAX_TEXT
        && !value.chars().any(char::is_control);
    if ok {
        Ok(())
    } else {
        Err(ProfileError::BadText {
            what: "title",
            value: value.to_owned(),
        })
    }
}

fn check_one_of(what: &'static str, value: &str, allowed: &[&str]) -> Result<(), ProfileError> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(ProfileError::Unknown {
            what,
            value: value.to_owned(),
        })
    }
}
