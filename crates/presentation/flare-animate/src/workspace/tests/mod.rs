//! Shared test helpers; the tests themselves are in the modules below.
#![allow(clippy::expect_used)]

use super::*;
use crate::layout::{self, Pane};
use crate::timeline_view;
use crate::{MENU_LAYOUT, MenuAction, MenuEntry};
use egui::{Color32, Key, Modifiers, Pos2, Rect, Vec2};
use egui::{Event, RawInput};
use flare_commands::Command;
use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId, SymbolId};
use flare_document::{Content, Document, FrameSpan, Layer, Scene, Shape, Timeline, Tween};
use flare_runtime::Mode;
use flare_tools::handles::{self, Handle};
use flare_tools::ops::{self, SpanRef};
use glamx::{DAffine2, DVec2};
use std::num::NonZeroU32;
use std::path::PathBuf;

mod basics;
mod frames_text_and_sound;
mod layers_and_symbols;
mod menus_and_recovery;
mod panels_and_view;
mod pen_and_selection;
mod rendering_and_workers;
mod spans_and_handles;
mod tools_and_menus;
mod window_menu;

fn document() -> Document {
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![]).expect("layer");
    let scene = Scene::new(SceneId::new(), "Scene 1", Timeline::new(vec![layer]));
    let rate = FrameRate::new(24, 1).expect("rate");
    Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("document")
}

fn workspace() -> Workspace {
    Workspace::new(document(), None)
}

fn layer_count(ws: &Workspace) -> usize {
    ws.document().scenes()[0].timeline().layers().len()
}

fn input(time: f64, events: Vec<Event>) -> RawInput {
    RawInput {
        time: Some(time),
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
        events,
        ..RawInput::default()
    }
}

/// Runs one UI pass. A real renderer consumes the texture updates; tests discard them.
fn pass(ws: &mut Workspace, ctx: &egui::Context, input: RawInput) {
    let mut output = ctx.run_ui(input, |ui| ws.show(ui));
    output.textures_delta.clear();
}

fn run(ws: &mut Workspace, ctx: &egui::Context, time: f64) {
    pass(ws, ctx, input(time, vec![]));
}

fn key(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn rect(ws: &mut Workspace) {
    ws.commit_rectangle(Pos2::new(10.0, 10.0), Pos2::new(110.0, 60.0));
}

fn spans_of(ws: &Workspace, layer: usize) -> usize {
    ws.document().scenes()[0].timeline().layers()[layer]
        .spans()
        .len()
}

/// Select the rectangle drawn by `rect()` (it covers 10..110 x 10..60).
fn draw_and_select(ws: &mut Workspace) {
    rect(ws);
    ws.set_tool(Tool::Select);
    ws.begin_drag(Pos2::new(50.0, 30.0));
    ws.end_drag(); // a plain press and release: selects, moves nothing
}

fn shape_bounds(ws: &Workspace) -> Rect {
    let scene = ws.scene_id().expect("scene");
    let selected = ws.selected.expect("selected");
    ops::span_bounds(ws.document(), scene, selected).expect("bounds")
}

fn active_spans(ws: &Workspace) -> Vec<(u32, u32)> {
    ws.active_layer()
        .expect("layer")
        .spans()
        .iter()
        .map(|s| (s.start(), s.duration().get()))
        .collect()
}

fn active_layer(ws: &Workspace) -> Layer {
    ws.active_layer().expect("layer").clone()
}

/// A workspace with symbol "Symbol 1" (a converted rectangle) placed in the scene, with
/// the instance selected.
fn with_symbol() -> (Workspace, SymbolId) {
    let mut ws = workspace();
    draw_and_select(&mut ws);
    ws.convert_selection_to_symbol();
    let symbol = ws.selected_symbol.expect("symbol");
    (ws, symbol)
}

fn symbol_layers(ws: &Workspace, symbol: SymbolId) -> usize {
    ws.session
        .document()
        .symbol(symbol)
        .expect("symbol")
        .timeline()
        .layers()
        .len()
}

fn pen_triangle(ws: &mut Workspace) {
    ws.set_tool(Tool::Pen);
    for at in [(10.0, 10.0), (110.0, 10.0), (110.0, 90.0)] {
        ws.pen_press(Pos2::new(at.0, at.1));
        ws.pen_release();
    }
}

fn layer_names(ws: &Workspace) -> Vec<String> {
    ws.document().scenes()[0]
        .timeline()
        .layers()
        .iter()
        .map(|l| l.name().to_string())
        .collect()
}

/// Two rectangles side by side on separate layers; returns the workspace.
fn two_rectangles() -> Workspace {
    let mut ws = workspace();
    for (a, b) in [((10.0, 10.0), (50.0, 50.0)), ((100.0, 10.0), (140.0, 50.0))] {
        ws.set_tool(Tool::Rectangle);
        ws.begin_drag(Pos2::new(a.0, a.1));
        ws.update_drag(Pos2::new(b.0, b.1));
        ws.end_drag();
    }
    ws.set_tool(Tool::Select);
    ws
}

fn rect_x(ws: &Workspace, layer: usize) -> f32 {
    let layers = ws.document().scenes()[0].timeline().layers();
    let Content::Shape(shape) = layers[layer].spans()[0].content() else {
        panic!("a shape expected")
    };
    ops::shape_bounds(shape).expect("bounds").min.x
}

fn selected_shape(ws: &Workspace) -> Shape {
    let scene = ws.scene_id().expect("scene");
    match ops::find_span(ws.document(), scene, ws.selected.expect("selected"))
        .expect("span")
        .content()
    {
        Content::Shape(shape) => shape.clone(),
        _ => panic!("a shape expected"),
    }
}

fn endpoints(shape: &Shape) -> Vec<(f64, f64)> {
    shape
        .path()
        .iter()
        .filter_map(|s| match s {
            flare_document::PathSegment::MoveTo(p) | flare_document::PathSegment::LineTo(p) => {
                Some((p.as_vec().x, p.as_vec().y))
            }
            _ => None,
        })
        .collect()
}

/// A selected line from (10, 10) to (50, 30), which is not symmetric under flips.
fn line_selected() -> Workspace {
    let mut ws = workspace();
    ws.set_tool(Tool::Line);
    ws.begin_drag(Pos2::new(10.0, 10.0));
    ws.update_drag(Pos2::new(50.0, 30.0));
    ws.end_drag();
    ws.set_tool(Tool::Select);
    // Strokes are not hit-testable on this branch, so select the new layer's span directly.
    let layer = ws.current_layer.expect("the line's layer");
    ws.selected = Some(SpanRef { layer, start: 0 });
    ws
}

/// One layer holding spans at 0..3 and 6..8, for timeline drags.
fn spans_workspace() -> (Workspace, LayerId) {
    let id = LayerId::new();
    let span = |start: u32, len: u32| {
        FrameSpan::new(
            start,
            NonZeroU32::new(len).expect("nonzero"),
            Content::Empty,
        )
    };
    let layer = Layer::new(id, "L", vec![span(0, 3), span(6, 2)]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    let rate = FrameRate::new(24, 1).expect("rate");
    let doc = Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("doc");
    (Workspace::new(doc, None), id)
}

fn active_spans_of(ws: &Workspace, id: LayerId) -> Vec<(u32, u32)> {
    ws.layer(id)
        .expect("layer")
        .spans()
        .iter()
        .map(|s| (s.start(), s.duration().get()))
        .collect()
}

/// A shape held for 24 frames, plus a second layer that appears only at frame 12.
fn held_then_changing() -> Workspace {
    let mut ws = workspace();
    ws.commit_rectangle(Pos2::new(10.0, 10.0), Pos2::new(60.0, 60.0));
    let layer = ws.current_layer.expect("layer");
    ws.set_frame(12);
    ws.current_layer = Some(layer);
    ws.frame_edit(FrameOp::InsertKeyframe);
    ws.set_frame(0);
    ws
}

/// Runs UI passes until `done` holds or five seconds pass, the way a running app would.
fn wait_until(ws: &mut Workspace, ctx: &egui::Context, done: impl Fn(&Workspace) -> bool) {
    let started = std::time::Instant::now();
    let mut time = 100.0;
    while !done(ws) {
        assert!(
            started.elapsed().as_secs() < 5,
            "timed out waiting for background work"
        );
        run(ws, ctx, time);
        time += 0.016;
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

/// A selected 100 x 50 rectangle at (10, 10), center (60, 35), for rotate and skew drags.
fn wide_rectangle_selected() -> Workspace {
    let mut ws = workspace();
    ws.commit_rectangle(Pos2::new(10.0, 10.0), Pos2::new(110.0, 60.0));
    ws.set_tool(Tool::Select);
    let layer = ws.current_layer.expect("layer");
    ws.selected = Some(SpanRef { layer, start: 0 });
    ws
}

/// A rectangle that lasts 12 frames, then a second exposure from frame 12.
fn two_exposures() -> (Workspace, LayerId) {
    let ws = held_then_changing();
    let layer = ws.current_layer.expect("layer");
    (ws, layer)
}

fn layer_spans(ws: &Workspace, id: LayerId) -> Vec<(u32, u32)> {
    ws.layer(id)
        .expect("layer")
        .spans()
        .iter()
        .map(|s| (s.start(), s.duration().get()))
        .collect()
}

/// A workspace whose text jobs use the Hack font by name, whatever the machine has.
fn text_workspace() -> Workspace {
    let mut ws = workspace();
    ws.text_family = "Hack".into();
    ws.text_size = 40.0;
    ws.set_tool(Tool::Text);
    ws
}

fn type_text(ws: &mut Workspace, at: Pos2, text: &str) {
    ws.text_edit = Some(PendingText {
        at,
        buffer: text.into(),
        focused: true,
    });
    ws.place_text();
}

/// A one-second mono 16-bit WAV file at 8 kHz, written to `dir`.
fn wav_file(dir: &std::path::Path, name: &str) -> PathBuf {
    let samples: Vec<i16> = (0..8000).map(|i| ((i % 100) as i16 - 50) * 300).collect();
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&8000_u32.to_le_bytes());
    out.extend_from_slice(&16_000_u32.to_le_bytes());
    out.extend_from_slice(&2_u16.to_le_bytes());
    out.extend_from_slice(&16_u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    let path = dir.join(name);
    std::fs::write(&path, out).expect("write wav");
    path
}

fn drain_audio(ws: &mut Workspace) -> Vec<String> {
    ws.take_audio_events()
        .into_iter()
        .map(|e| match e {
            AudioEvent::Stop => "stop".to_owned(),
            AudioEvent::Play { delay_seconds, .. } => format!("play@{:.3}", delay_seconds),
        })
        .collect()
}

fn drawn_shape(ws: &Workspace, layer: usize) -> Shape {
    match ws.document().scenes()[0].timeline().layers()[layer].spans()[0].content() {
        Content::Shape(shape) => shape.clone(),
        _ => panic!("shape expected"),
    }
}
