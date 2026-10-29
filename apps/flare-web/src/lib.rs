//! `flare-web`: the Animate workspace in a browser tab.
//!
//! It is the same `flare-animate` workspace the desktop app shows, drawn by eframe's web
//! backend onto a canvas. What differs is only what a page can do: there is no file system and
//! no threads, so work runs inline (see `flare-runtime`), and opening and saving go through the
//! browser rather than `flare-platform`.
#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
mod web;
