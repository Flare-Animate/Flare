//! What a desktop needs from the operating system, and nothing about how it is drawn: where
//! per-user files live, the sound card, the native dialogs, recovery prompts, and the threads that publish,
//! export and play test movies. A front end calls these; none of them know about egui.
#![forbid(unsafe_code)]

pub mod audio;
pub mod dialogs;
pub mod dirs;
pub mod jobs;
pub mod recovery;
