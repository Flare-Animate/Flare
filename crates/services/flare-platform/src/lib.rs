//! What a desktop needs from the operating system, and nothing about how it is drawn: where
//! per-user files live, the sound card, the native dialogs, recovery prompts, and the threads that publish,
//! export and play test movies. A front end calls these; none of them know about egui.
#![forbid(unsafe_code)]

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
#[cfg(unix)]
pub mod tablet;
#[cfg(unix)]
mod tablet_events;
#[cfg(all(test, unix))]
mod tablet_events_tests;
