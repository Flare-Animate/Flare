//! Features of the previous C++ Flare that are not yet re-implemented in Rust.
//! Every entry returns [`NotYetImplemented`]; none ever reports success.
//! The list is mirrored in PORTING_TRACKER.md ("Stubbed re-additions").

use std::fmt;

/// A legacy feature with the original source path it comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyFeature {
    pub name: &'static str,
    pub original: &'static str,
}

/// Error returned by every stubbed feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotYetImplemented(pub LegacyFeature);

impl fmt::Display for NotYetImplemented {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} is not yet re-implemented (see {})", self.0.name, self.0.original)
    }
}

impl std::error::Error for NotYetImplemented {}

macro_rules! stub {
    ($(#[$m:meta])* $fn:ident, $konst:ident, $name:literal, $path:literal) => {
        $(#[$m])*
        pub const $konst: LegacyFeature = LegacyFeature { name: $name, original: $path };
        $(#[$m])*
        pub fn $fn() -> Result<(), NotYetImplemented> {
            Err(NotYetImplemented($konst))
        }
    };
}

stub!(/** Tracker tool. Source: `original/flare/sources/tnztools/trackertool.cpp`. */
    tracker_tool, TRACKER, "Tracker tool", "original/flare/sources/tnztools/trackertool.cpp");
stub!(/** Plastic (mesh deformation) tool. Source: `original/flare/sources/tnztools/plastictool.cpp`. */
    plastic_tool, PLASTIC, "Plastic tool", "original/flare/sources/tnztools/plastictool.cpp");
stub!(/** Hook tool. Source: `original/flare/sources/tnztools/hooktool.cpp`. */
    hook_tool, HOOK, "Hook tool", "original/flare/sources/tnztools/hooktool.cpp");
stub!(/** Cutter tool. Source: `original/flare/sources/tnztools/cuttertool.cpp`. */
    cutter_tool, CUTTER, "Cutter tool", "original/flare/sources/tnztools/cuttertool.cpp");
stub!(/** Stop motion capture. Source: `original/flare/sources/stopmotion/stopmotion.cpp`. */
    stop_motion, STOP_MOTION, "Stop motion", "original/flare/sources/stopmotion/stopmotion.cpp");
stub!(/** Scan cleanup. Source: `original/flare/sources/tcleanupper/tcleanupper.cpp`. */
    cleanup, CLEANUP, "Scan cleanup", "original/flare/sources/tcleanupper/tcleanupper.cpp");
stub!(/** Render farm task. Source: `original/flare/sources/flarefarm/tfarm/tfarmtask.cpp`. */
    render_farm, RENDER_FARM, "Render farm", "original/flare/sources/flarefarm/tfarm/tfarmtask.cpp");

/// All stubbed features.
pub const ALL: [LegacyFeature; 7] = [TRACKER, PLASTIC, HOOK, CUTTER, STOP_MOTION, CLEANUP, RENDER_FARM];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_stubs_error_and_cite_existing_path() {
        assert!(tracker_tool().is_err() && render_farm().is_err() && cleanup().is_err());
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        for f in ALL {
            assert!(root.join(f.original).exists(), "{}", f.original);
            assert!(NotYetImplemented(f).to_string().contains("not yet re-implemented"));
        }
    }
}
