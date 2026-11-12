//! What a desktop needs from the operating system, and nothing about how it is drawn: where
//! per-user files live, the sound card, the native dialogs, recovery prompts, and the threads that publish,
//! export and play test movies. A front end calls these; none of them know about egui.
// Denied rather than forbidden so the one module that reaches AppKit can allow it for itself.
#![deny(unsafe_code)]

pub mod app_icon;
#[cfg(target_os = "macos")]
mod app_icon_macos;
#[cfg(test)]
mod app_icon_tests;

pub mod audio;
mod audio_prepared;
pub mod browser;
pub mod dialogs;
#[cfg(test)]
mod dialogs_tests;
pub mod dirs;
pub mod jobs;
pub mod locale;
pub mod recovery;
