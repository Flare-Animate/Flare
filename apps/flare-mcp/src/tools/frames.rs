//! The frame picker for an agent: list frames and move the playhead. Both ask
//! `flare_editor::frame_picker`, as the window's picker and `flare goto` do.

use flare_editor::frame_picker::{self, Goto};
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

use crate::ops::first_scene;
use crate::server::{FlareServer, Outcome};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListFrames {
    /// First frame to list, counting from 0; defaults to 0.
    pub first: Option<u32>,
    /// How many frames to list, at most 2000; defaults to 200.
    pub count: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GotoFrame {
    /// A frame number counting from 0, or one of first, last, next, previous, next-keyframe,
    /// previous-keyframe (from the current playhead).
    pub target: String,
}

#[tool_router(router = frame_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "The frame picker: list the first scene's frames (from `first`, up to `count`), each with how many layers start a keyframe there and whether it shows art, and the playhead."
    )]
    fn list_frames(&self, Parameters(args): Parameters<ListFrames>) -> Outcome {
        self.read(|state| {
            let scene = first_scene(state)?;
            let timeline = state
                .session
                .document()
                .scenes()
                .iter()
                .find(|s| s.id() == scene)
                .ok_or("the document has no scene")?
                .timeline();
            let cells =
                frame_picker::cells(timeline, args.first.unwrap_or(0), args.count.unwrap_or(200));
            Ok(json!({
                "playhead": state.playhead,
                "frames": timeline.frame_count(),
                "cells": cells.iter().map(|c| json!({
                    "frame": c.frame, "keyframes": c.keyframes, "shows_art": c.shows_art,
                })).collect::<Vec<_>>(),
            }))
        })
    }

    #[tool(
        description = "Go to a frame: move the playhead to a frame number (from 0) or to first, last, next, previous, next-keyframe or previous-keyframe. draw_rect draws there when it is given no start. The playhead is not part of the project, so this works on a read-only server and is not undone. A frame outside the scene is refused with the scene's length."
    )]
    fn goto_frame(&self, Parameters(args): Parameters<GotoFrame>) -> Outcome {
        let goto: Goto = args
            .target
            .parse()
            .map_err(|e: frame_picker::GotoError| e.to_string())?;
        let mut state = self.lock();
        let scene = first_scene(&state)?;
        let timeline = state
            .session
            .document()
            .scenes()
            .iter()
            .find(|s| s.id() == scene)
            .ok_or("the document has no scene")?
            .timeline();
        let frame =
            frame_picker::resolve(timeline, state.playhead, goto).map_err(|e| e.to_string())?;
        let cell = frame_picker::cells(timeline, frame, 1).into_iter().next();
        let out = json!({
            "frame": frame,
            "frames": timeline.frame_count(),
            "keyframes": cell.map_or(0, |c| c.keyframes),
            "shows_art": cell.is_some_and(|c| c.shows_art),
        });
        state.playhead = frame;
        Ok(out.to_string())
    }
}
