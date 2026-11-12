//! Named starting arrangements.

use serde::{Deserialize, Serialize};

/// A named layout profile: a starting arrangement of the panels, the way the
/// Animate/Toonz profiles give a different workspace for each task.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LayoutProfile {
    Animate,
    Cleanup,
    Compose,
    Draw,
    Library,
    Rigging,
    Sound,
    Timing,
    Default,
    OpenToonz,
    Flippaclip,
    Moho,
    StudioGhibli,
}

impl LayoutProfile {
    pub const ALL: [LayoutProfile; 13] = [
        Self::Animate,
        Self::Cleanup,
        Self::Compose,
        Self::Draw,
        Self::Library,
        Self::Rigging,
        Self::Sound,
        Self::Timing,
        Self::Default,
        Self::OpenToonz,
        Self::Flippaclip,
        Self::Moho,
        Self::StudioGhibli,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Animate => "Animate",
            Self::Cleanup => "Cleanup",
            Self::Compose => "Compose",
            Self::Draw => "Draw",
            Self::Library => "Library",
            Self::Rigging => "Rigging",
            Self::Sound => "Sound",
            Self::Timing => "Timing",
            Self::Default => "Default",
            Self::OpenToonz => "OpenToonz",
            Self::Flippaclip => "Flippaclip",
            Self::Moho => "Moho",
            Self::StudioGhibli => "StudioGhibli",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|p| p.name() == name)
    }
}

/// The panel arrangement for a profile. Profiles that are not yet specialised
/// fall back to the default tree; the specialist layouts land as they are ported.
pub fn profile_tree(profile: LayoutProfile) -> crate::Layout {
    match profile {
        LayoutProfile::Animate
        | LayoutProfile::Default
        | LayoutProfile::OpenToonz
        | LayoutProfile::Flippaclip
        | LayoutProfile::Moho
        | LayoutProfile::StudioGhibli => crate::default_layout(),
        // Stubbed: specialised arrangements to be ported from the Rust-Rewrite
        // profiles (see docs/LAYOUT_PROFILES.md). For now they use the default.
        LayoutProfile::Cleanup
        | LayoutProfile::Compose
        | LayoutProfile::Draw
        | LayoutProfile::Library
        | LayoutProfile::Rigging
        | LayoutProfile::Sound
        | LayoutProfile::Timing => crate::default_layout(),
    }
}
