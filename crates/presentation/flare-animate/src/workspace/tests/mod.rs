//! Shared test helpers; the tests themselves are in the modules below.
#![allow(clippy::expect_used)]

use super::*;
use egui::{Color32, Key, Modifiers, Pos2, Rect, Vec2};
use egui::{Event, RawInput};
use flare_commands::Command;
use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId, SymbolId};
use flare_dock::{self, Pane};
use flare_document::{Content, Document, FrameSpan, Layer, Scene, Shape, Timeline, Tween};
use flare_editor::timeline_view;
use flare_menubar::{MENU_LAYOUT, MenuAction, MenuEntry};
use flare_runtime::Mode;
use flare_tools::free_transform::handles::{self, Handle};
use flare_tools::ops::{self, SpanRef};
use glamx::{DAffine2, DVec2};
use std::num::NonZeroU32;
use std::path::PathBuf;

mod rotate_view;
mod sound_helpers;
use sound_helpers::{drain_audio, wav_file};

mod anchor_tools;
mod basics;
mod blend;
mod bone;
mod brush_nib;
mod brush_stroke;
mod camera;
mod chord_dispatch;
mod close_prompt;
mod color_section;
mod command_palette;
mod drawing_into_layers;
mod eraser;
mod frame_menu;
mod frame_rate;
mod frames_text_and_sound;
mod graphic_playback;
mod home;
mod home_extra;
mod home_recent;
mod language;
mod layer_drag;
mod layer_names;
mod layer_ops;
mod layer_transparency;
mod layers_and_symbols;
mod line_shift;
mod live_transform;
mod loop_range;
mod magic_wand;
mod menus_and_recovery;
mod menus_like_animate;
mod motion_editor;
mod multiple_frames;
mod narrow_window;
mod native_pen;
mod new_document_dialog;
mod nothing_left_in_english;
mod onion_settings;
mod open_any;
mod paint_areas;
mod paint_gap_size;
mod paint_tools;
mod panels_and_view;
mod paste_into_layer;
mod pasteboard;
mod pen_and_selection;
mod polygon_lasso;
mod preferences_dialog;
mod properties_context;
mod properties_tabs;
mod publish_popup;
mod publish_settings;
mod rail;
mod release_frame;
mod rendering_and_workers;
mod rulers_and_mirror;
mod save_flare;
mod save_formats;
mod scene_name;
mod selection_follows_playhead;
mod shortcut_editor;
mod sound_scrub;
mod span_drop;
mod spans_and_handles;
mod splash;
mod stage_menu;
mod stage_pointer;
mod status_bar;
mod swatches;
mod symbol_dialog;
mod tab_strip;
mod text_keys;
mod text_objects;
mod timeline_animating;
mod timeline_clicks;
mod timeline_frame_buttons;
mod timeline_frames;
mod timeline_names_column;
mod timeline_scrub;
mod tool_pins;
mod tool_slots;
mod tool_strip;
mod tools_and_menus;
mod tools_translated;
mod transform_pointer;
mod translated_chrome;
mod translated_menu_bar;
mod translated_side_panels;
mod translated_start_messages;
mod transport;
mod tween_inspector;
mod tweens;
mod undo_pictures;
mod window_controls;
mod window_menu;
mod workspaces;

fn document() -> Document {
    // A blank 24-frame keyframe, so a drawing joins a span of the length the old drawing default
    // gave and the frame tests keep their room.
    let blank = FrameSpan::new(0, NonZeroU32::new(24).expect("24"), Content::Empty);
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![blank]).expect("layer");
    let scene = Scene::new(SceneId::new(), "Scene 1", Timeline::new(vec![layer]));
    let rate = FrameRate::new(24, 1).expect("rate");
    Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("document")
}

/// A new document with nothing in it: no keyframe at all, as the blank and dirty checks need.
fn blank_workspace() -> Workspace {
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![]).expect("layer");
    let scene = Scene::new(SceneId::new(), "Scene 1", Timeline::new(vec![layer]));
    let rate = FrameRate::new(24, 1).expect("rate");
    let document = Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("document");
    Workspace::new(document, None)
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
        // As tall as a common laptop screen at 2x, so Properties shows its longer sections whole.
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 900.0))),
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

/// The elements the first layer's keyframe at frame 0 holds, bottom first. Shapes drawn one after
/// another land there, so "the nth drawing" is the element `n - 1` (index 0 of the old tests'
/// layers was the empty first layer).
fn drawings(ws: &Workspace) -> Vec<Content> {
    ws.document().scenes()[0].timeline().layers()[0]
        .span_at(0)
        .map(|s| s.elements().cloned().collect())
        .unwrap_or_default()
}

/// How many things are drawn in the document, over every layer and keyframe.
fn drawing_count(ws: &Workspace) -> usize {
    ws.document().scenes()[0]
        .timeline()
        .layers()
        .iter()
        .flat_map(|l| l.spans())
        .flat_map(|s| s.elements())
        .filter(|c| !matches!(c, Content::Empty))
        .count()
}

/// 1 when the `nth` drawing (counting from 1) is still on the first layer, else 0.
fn spans_of(ws: &Workspace, nth: usize) -> usize {
    usize::from(
        drawings(ws)
            .get(nth - 1)
            .is_some_and(|c| !matches!(c, Content::Empty)),
    )
}

/// The reference to the `nth` drawing (counting from 1) of the first layer's keyframe.
fn drawing_ref(ws: &Workspace, nth: usize) -> SpanRef {
    SpanRef {
        layer: ws.document().scenes()[0].timeline().layers()[0].id(),
        start: 0,
        element: nth - 1,
    }
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
    let symbol = ws.library.selected.expect("symbol");
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

/// How many elements the symbol's first frame holds over all its layers.
fn symbol_elements(ws: &Workspace, symbol: SymbolId) -> usize {
    ws.session
        .document()
        .symbol(symbol)
        .expect("symbol")
        .timeline()
        .layers()
        .iter()
        .filter_map(|l| l.span_at(0))
        .map(|s| s.element_count())
        .sum()
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

/// Like [`two_rectangles`], but the second goes into a layer of its own below the first.
fn two_rectangles_on_two_layers() -> Workspace {
    let mut ws = workspace();
    for (index, (a, b)) in [((10.0, 10.0), (50.0, 50.0)), ((100.0, 10.0), (140.0, 50.0))]
        .into_iter()
        .enumerate()
    {
        if index == 1 {
            ws.add_layer();
        }
        ws.set_tool(Tool::Rectangle);
        ws.begin_drag(Pos2::new(a.0, a.1));
        ws.update_drag(Pos2::new(b.0, b.1));
        ws.end_drag();
    }
    ws.set_tool(Tool::Select);
    ws
}

fn rect_x(ws: &Workspace, layer: usize) -> f32 {
    let Some(Content::Shape(shape)) = drawings(ws).get(layer - 1).cloned() else {
        panic!("a shape expected")
    };
    let shape = &shape;
    ops::shape_bounds(shape).expect("bounds").min.x
}

fn selected_shape(ws: &Workspace) -> Shape {
    let scene = ws.scene_id().expect("scene");
    match ops::find_element(ws.document(), scene, ws.selected.expect("selected")).expect("element")
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
    ws.selected = Some(SpanRef {
        layer,
        start: 0,
        element: 0,
    });
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

/// Runs UI passes until `done` holds or fifteen seconds pass (a font job takes about three in a debug build, more
/// on a busy machine), the way a running app would.
fn wait_until(ws: &mut Workspace, ctx: &egui::Context, done: impl Fn(&Workspace) -> bool) {
    let started = std::time::Instant::now();
    let mut time = 100.0;
    while !done(ws) {
        assert!(
            started.elapsed().as_secs() < 15,
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
    ws.selected = Some(SpanRef {
        layer,
        start: 0,
        element: 0,
    });
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
    ws.text.family = "Hack".into();
    ws.text.size = 40.0;
    ws.set_tool(Tool::Text);
    ws
}

fn type_text(ws: &mut Workspace, at: Pos2, text: &str) {
    ws.text.edit = Some(PendingText {
        at,
        buffer: text.into(),
        focused: true,
        editing: None,
    });
    ws.place_text();
}

fn drawn_shape(ws: &Workspace, layer: usize) -> Shape {
    match drawings(ws).get(layer - 1) {
        Some(Content::Shape(shape)) => shape.clone(),
        _ => panic!("shape expected"),
    }
}
