//! `flare-web`: Flare in a browser tab.
//!
//! It is the same `flare-animate` workspace the desktop app shows, drawn by eframe's web backend
//! onto a canvas. This crate is only what a page does differently: it has no file system and no
//! threads, so work runs inline (see `flare-runtime`), preferences live in the browser's local
//! storage, and opening and saving go through the file picker and a download. Nothing in
//! `flare-platform` applies here.
#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
mod app;
#[cfg(target_arch = "wasm32")]
mod canvas;
#[cfg(target_arch = "wasm32")]
mod files;
#[cfg(target_arch = "wasm32")]
mod preferences;
