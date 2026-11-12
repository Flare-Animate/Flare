//! Shared egui building blocks: the palette and type, the icon set, small widgets, the stage's
//! pan, zoom and turn math, and a modifier tapped on its own. Front ends (`flare-animate`)
//! assemble them into a workspace.
#![forbid(unsafe_code)]

pub mod chrome;
pub mod icons;
pub mod splash;
pub mod tap;
pub mod theme;
pub mod view;
