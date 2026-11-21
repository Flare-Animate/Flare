//! The frame picker on the command line: list the frames of the first scene, and find where a
//! request such as `next-keyframe` lands. Both ask `flare_editor::frame_picker`, as the window's
//! picker does.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use flare_editor::frame_picker::{self, Goto};
use serde_json::{Value, json};

use crate::project::open_session as open;

#[derive(Args)]
pub struct FramesArgs {
    pub path: PathBuf,
    #[arg(long, default_value_t = 0)]
    pub first: u32,
    #[arg(long, default_value_t = 200)]
    pub count: u32,
}

#[derive(Args)]
pub struct GotoArgs {
    pub path: PathBuf,
    pub target: String,
    #[arg(long, default_value_t = 0)]
    pub from: u32,
}

/// The frames of the first scene from `first`, up to `count` of them, each with how many layers
/// start a keyframe there and whether it shows any art.
pub fn list(args: FramesArgs) -> Result<Value> {
    let FramesArgs { path, first, count } = args;
    let session = open(&path)?;
    let scene = session
        .document()
        .scenes()
        .first()
        .context("document has no scene")?;
    let timeline = scene.timeline();
    let cells = frame_picker::cells(timeline, first, count);
    Ok(json!({
        "frames": timeline.frame_count(),
        "cells": cells.iter().map(|c| json!({
            "frame": c.frame,
            "keyframes": c.keyframes,
            "shows_art": c.shows_art,
        })).collect::<Vec<_>>(),
    }))
}

/// Where `target` (a zero-based frame, or first, last, next, previous, next-keyframe or
/// previous-keyframe) leads from frame `from`; an error says why not.
pub fn goto(args: GotoArgs) -> Result<Value> {
    let GotoArgs { path, target, from } = args;
    let goto: Goto = target.parse()?;
    let session = open(&path)?;
    let scene = session
        .document()
        .scenes()
        .first()
        .context("document has no scene")?;
    let timeline = scene.timeline();
    let frame = frame_picker::resolve(timeline, from, goto)?;
    let cell = frame_picker::cells(timeline, frame, 1)
        .into_iter()
        .next()
        .context("the frame is not in the scene")?;
    Ok(json!({
        "frame": frame,
        "frames": timeline.frame_count(),
        "keyframes": cell.keyframes,
        "shows_art": cell.shows_art,
    }))
}
