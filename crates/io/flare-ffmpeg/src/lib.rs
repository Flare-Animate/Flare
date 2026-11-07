//! Video through the system's ffmpeg, for the containers Flare does not write or read itself (MOV,
//! AVI, Matroska, WebM, MPEG, FLV, MP4 import). ffmpeg is never linked or bundled:
//! it is a program on the computer, found on PATH and run with its arguments as a list, so no shell
//! reads a file name. Every run has a time limit, and every byte it hands back is bounded.
#![forbid(unsafe_code)]

mod containers;
mod decode;
mod encode;
mod error;
mod run;
mod tool;

pub use containers::{CONTAINERS, Container, container};
pub use decode::{Clip, MAX_CLIP_BYTES, decode};
pub use encode::{Movie, encode};
pub use error::FfmpegError;
pub use tool::{Tools, locate};

#[cfg(test)]
mod tests;
