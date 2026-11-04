//! The four profiles that ship with Flare, as data. They are read by `load::parse` like any other
//! profile, so a built-in one is held to the same rules.

use crate::ProfileError;
use crate::load;
use crate::model::Profile;

/// Each built-in profile's id and its source file in `data/`.
pub const SOURCES: [(&str, &str); 4] = [
    ("flare-default", include_str!("../data/flare-default.json")),
    (
        "opentoonz-classic",
        include_str!("../data/opentoonz-classic.json"),
    ),
    ("tahoma-style", include_str!("../data/tahoma-style.json")),
    ("animate-style", include_str!("../data/animate-style.json")),
];

/// Reads every built-in profile. Fails only if a shipped file breaks the rules, which a test catches.
pub fn builtin() -> Result<Vec<Profile>, ProfileError> {
    SOURCES.iter().map(|(_, json)| load::parse(json)).collect()
}
