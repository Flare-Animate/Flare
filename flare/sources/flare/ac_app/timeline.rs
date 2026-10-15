//! Timeline panel: layer column + frame grid, styled after Animate's timeline
//! (keyframe dots, purple classic-tween spans, green shape-tween spans, red
//! playhead, blue frame selection).

use crate::{c32, App, ACCENT};
use ac_core::{FrameSel, LayerFlag, LayerKind, Tween};
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, Sense, Stroke};

/// Onion skin markers and modes (timeline toolbar).
pub struct OnionOpts {
    pub before: u32,
    pub after: u32,
    /// Anchored markers: a fixed frame range instead of one around the playhead.
    pub anchor: Option<(u32, u32)>,
    pub outline: bool,
    /// Edit Multiple Frames: show and edit every keyframe between the markers.
    pub multi: bool,
    /// Shift and Trace (from OpenToonz): the previous and next keyframes show as
    /// ghosts you can slide and turn under the current drawing. View only.
    pub shift_trace: bool,
    /// Dragging on the stage moves a ghost (Shift: the next one; Alt: rotate).
    pub shift_edit: bool,
    /// Ghost offsets (previous, next) in stage space.
    pub shift: [ac_core::Matrix; 2],
    /// Pivot of the current Alt-drag turn.
    pub shift_pivot: Option<ac_core::Pt>,
    /// Onion skin shows only keyframes (frame-by-frame drawing).
    pub keys_only: bool,
}

impl Default for OnionOpts {
    fn default() -> Self {
        OnionOpts { before: 2, after: 2, anchor: None, outline: false, multi: false, shift_trace: false, shift_edit: false, shift: [ac_core::Matrix::IDENTITY; 2], shift_pivot: None, keys_only: false }
    }
}

#[derive(Default)]
pub struct TimelineState {
    /// Onion marker being dragged (true = start marker).
    marker_drag: Option<bool>,
    /// First visible frame column.
    pub scroll: f32,
    /// First visible layer row.
    pub vscroll: usize,
    pub renaming: Option<(usize, String)>,
    /// Cell where the current mouse gesture started.
    anchor: Option<(usize, u32)>,
    gesture: Gesture,
    /// Layer being dragged in the layer column.
    layer_drag: Option<usize>,
    /// Show Parenting View: a link column where you drag a layer onto its parent.
    pub parent_view: bool,
    /// Parent link being dragged from this layer.
    link_drag: Option<usize>,
    /// Frame View: thumbnails of each keyframe in its span.
    pub frame_view: FrameView,
    /// Keyframe thumbnails by (layer, frame, in context, width, height); cleared on any edit.
    thumbs: std::collections::HashMap<(usize, u32, bool, u32, u32), Option<egui::TextureHandle>>,
    thumbs_key: (u64, Option<ac_core::SymbolId>, usize),
    /// Frame cell width in pixels (Frames menu sizes, Ctrl+wheel); 0 = Normal.
    pub cell_w: f32,
    /// Header and counter show seconds instead of frame numbers.
    pub seconds: bool,
    /// Control › Loop Range: playback loops over these frames (inclusive).
    pub loop_range: Option<(u32, u32)>,
    /// Loop marker being dragged (true = start marker).
    loop_drag: Option<bool>,
}

impl TimelineState {
    /// Frame cell width in pixels.
    pub fn cw(&self) -> f32 {
        if self.cell_w <= 0.0 { CW } else { self.cell_w.clamp(CW_MIN, CW_MAX) }
    }
}

/// Frame-size presets (Frames menu), as Animate's Tiny … Large.
pub const FRAME_SIZES: [(&str, f32); 5] = [("Tiny", 5.0), ("Small", 8.0), ("Normal", 11.0), ("Medium", 16.0), ("Large", 24.0)];
const CW_MIN: f32 = 3.0;
const CW_MAX: f32 = 40.0;

/// The frame after `frame` while playing `n` frames, and whether playback
/// wrapped (the sound restarts); None when playback ends. A loop range keeps
/// playback inside it once the playhead reaches its end.
pub(crate) fn play_next(frame: u32, n: u32, looping: bool, range: Option<(u32, u32)>) -> Option<(u32, bool)> {
    let last = n.saturating_sub(1);
    if let Some((a, b)) = range.filter(|_| looping) {
        let (a, b) = (a.min(last), b.min(last));
        if frame == b || frame >= last {
            return Some((a, true));
        }
        return Some((frame + 1, false));
    }
    if frame + 1 >= n {
        return looping.then_some((0, true));
    }
    Some((frame + 1, false))
}

/// Header label spacing: (frames between labels).
fn label_step(cw: f32, seconds: bool, fps: f32) -> u32 {
    if seconds {
        let fpl = fps.round().max(1.0) as u32;
        for k in [1u32, 2, 5, 10, 15, 30, 60, 120, 300] {
            if (fpl * k) as f32 * cw >= 34.0 {
                return fpl * k;
            }
        }
        fpl * 600
    } else {
        for k in [1u32, 2, 5, 10, 20, 50, 100, 200, 500] {
            if k as f32 * cw >= 26.0 {
                return k;
            }
        }
        1000
    }
}

/// "1:05" / "12s" / "1.5s" for the seconds display.
pub(crate) fn fmt_time(frame: u32, fps: f32) -> String {
    let t = frame as f32 / fps.max(0.01);
    if t >= 60.0 {
        format!("{}:{:04.1}", (t / 60.0).floor() as u32, t % 60.0)
    } else if (t - t.round()).abs() < 1e-3 {
        format!("{}s", t.round() as u32)
    } else {
        format!("{t:.2}s")
    }
}

/// Timeline Frame View options (as in Animate's frame-view menu).
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum FrameView {
    #[default]
    Normal,
    /// Each keyframe's content, scaled to fit the span.
    Preview,
    /// Each keyframe as it sits on the stage.
    PreviewInContext,
}

#[derive(Default, Clone, Copy, PartialEq, Debug)]
enum Gesture {
    #[default]
    None,
    Select,
    /// Dragging selected frames; value = current frame offset.
    Move(i64),
    /// Dragging the end of a layer's last span (Ctrl+drag, or any motion tween).
    SpanEnd { layer: usize, start: u32, end: u32 },
}

const LW: f32 = 230.0; // layer column width
const HH: f32 = 22.0; // header height
const RH: f32 = 24.0; // row height
const CW: f32 = 11.0; // default frame cell width

const CLASSIC: Color32 = Color32::from_rgb(0x6E, 0x5B, 0xB0);
const SHAPE: Color32 = Color32::from_rgb(0x4E, 0x8F, 0x57);
const MOTION: Color32 = Color32::from_rgb(0x3F, 0x78, 0xB5);
const PLAYHEAD: Color32 = Color32::from_rgb(0xE5, 0x3B, 0x3B);

/// Frame commands (Insert menu, Edit › Timeline, and the right-click menu).
pub fn frame_menu(app: &mut App, ui: &mut egui::Ui) {
    let item = |ui: &mut egui::Ui, label: &str, sc: &str| ui.add(egui::Button::new(label).shortcut_text(sc)).clicked();
    if item(ui, "Insert Frame", "F5") { app.ed.insert_frame(); ui.close_menu(); }
    if item(ui, "Remove Frames", "Shift+F5") { app.ed.remove_frame(); ui.close_menu(); }
    if item(ui, "Insert Keyframe", "F6") { app.ed.insert_keyframe(); ui.close_menu(); }
    if item(ui, "Insert Blank Keyframe", "F7") { app.ed.insert_blank_keyframe(); ui.close_menu(); }
    if item(ui, "Clear Keyframe", "Shift+F6") { app.ed.clear_keyframe(); ui.close_menu(); }
    if item(ui, "Clear Frames", "Del") { app.ed.clear_frames(); ui.close_menu(); }
    ui.separator();
    if item(ui, "Create Classic Tween", "") { app.ed.set_tween(Tween::Classic { ease: 0.0, rotate_turns: 0 }); ui.close_menu(); }
    if item(ui, "Create Motion Tween", "") { app.ed.set_tween(Tween::Motion { ease: 0.0 }); app.status = "Motion tween: move to a frame and change the object to add property keyframes".into(); ui.close_menu(); }
    if item(ui, "Create Shape Tween", "") { app.ed.set_tween(Tween::Shape { ease: 0.0 }); ui.close_menu(); }
    if item(ui, "Remove Tween", "") { app.ed.set_tween(Tween::None); ui.close_menu(); }
    if app.ed.motion_span(app.ed.layer, app.ed.frame).is_some() && item(ui, "Motion Editor…", "") { app.motion.open = true; ui.close_menu(); }
    if item(ui, "Convert to Frame-by-Frame", "") { app.ed.convert_to_frame_by_frame(); ui.close_menu(); }
    if item(ui, "Reverse Frames", "") { app.ed.reverse_frames(); ui.close_menu(); }
    if ui.button("Synchronize Symbols").on_hover_text("Graphic symbols keep playing across the keyframes in the selection (or the whole layer) instead of restarting: classic tweens get Sync, plain keyframes get their First Frame advanced").clicked() { crate::palette::sync_symbols(app); ui.close_menu(); }
    let mut keep = app.settings.retain_tween_curves;
    if ui
        .checkbox(&mut keep, "Keep Tween Curves When Splitting")
        .on_hover_text("On: a keyframe added inside a tween (or a partial copy/reverse) keeps the original ease curve exactly — Flare's behaviour. Off: Animate's behaviour (each piece becomes its own tween). Pieces export to .fla frame by frame.")
        .changed()
    {
        app.settings.retain_tween_curves = keep;
        app.settings.save();
        ac_core::frames::set_retain_tween_curves(keep);
    }
    ui.separator();
    if item(ui, "Cut Frames", "") { app.ed.cut_frames(); ui.close_menu(); }
    if item(ui, "Copy Frames", "") { app.ed.copy_frames(); ui.close_menu(); }
    if ui.add_enabled(app.ed.can_paste_frames(), egui::Button::new("Paste Frames")).clicked() { app.ed.paste_frames(); ui.close_menu(); }
    if ui.add_enabled(app.ed.can_paste_frames(), egui::Button::new("Paste and Overwrite Frames")).clicked() { app.ed.paste_overwrite_frames(); ui.close_menu(); }
    if item(ui, "Select All Frames", "") {
        let n = app.ed.timeline().layers.len();
        let f = app.ed.frame_count();
        app.ed.select_frames(FrameSel { layer0: 0, layer1: n.saturating_sub(1), f0: 0, f1: f.saturating_sub(1) });
        ui.close_menu();
    }
}

/// Does a drag starting at (layer, frame) grab the end of the layer's last
/// span? Ctrl+drag on any last frame, plain drag on a motion tween's end.
/// Is `frame` the last frame of a span that can be dragged (Ctrl, or any
/// motion tween)? Returns the span's start.
fn span_end_at(app: &App, layer: usize, frame: u32, ctrl: bool) -> Option<u32> {
    let l = app.ed.timeline().layers.get(layer)?;
    if l.end == 0 || l.kind == LayerKind::Folder || frame >= l.end {
        return None;
    }
    let (start, kf, next) = l.span_at(frame)?;
    if frame + 1 != next.unwrap_or(l.end).min(l.end) {
        return None;
    }
    (ctrl || kf.tween.is_motion()).then_some(start)
}

/// Centre of a row's parent-link dot (Show Parenting View).
fn link_dot(rect: Rect, y: f32) -> egui::Pos2 {
    pos2(rect.left() + LW - 90.0, y + RH / 2.0)
}

/// Layer commands (right-click a layer name, Insert › Timeline).
pub fn layer_menu(app: &mut App, ui: &mut egui::Ui, i: usize) {
    let kind = app.ed.timeline().layers.get(i).map(|l| l.kind).unwrap_or(LayerKind::Normal);
    if ui.button("Insert Layer").clicked() { app.ed.layer = i; app.ed.add_layer(); ui.close_menu(); }
    if ui.button("Delete Layer").clicked() { app.ed.layer = i; app.ed.delete_layer(); ui.close_menu(); }
    if ui.button("Duplicate Layer").clicked() { app.ed.layer = i; app.ed.duplicate_layer(); ui.close_menu(); }
    // the clicked layer, or the selected rows when it's one of them
    let pick = |app: &mut App| {
        let in_sel = app.ed.frame_sel.is_some_and(|s| (s.layer0.min(s.layer1)..=s.layer0.max(s.layer1)).contains(&i));
        if !in_sel {
            app.ed.frame_sel = None;
        }
        app.ed.layer = i;
    };
    if ui.button("Cut Layers").clicked() { pick(app); app.ed.cut_layers(); app.status = "Cut layers".into(); ui.close_menu(); }
    if ui.button("Copy Layers").clicked() { pick(app); app.ed.copy_layers(); app.status = "Copied layers (with their folders' contents)".into(); ui.close_menu(); }
    if ui.add_enabled(app.ed.can_paste_layers(), egui::Button::new("Paste Layers")).clicked() { app.ed.layer = i; app.ed.paste_layers(); ui.close_menu(); }
    ui.separator();
    if ui.button("New Folder").on_hover_text("Insert a folder above and put this layer in it").clicked() { app.ed.layer = i; app.ed.wrap_layer(LayerKind::Folder, "Folder"); ui.close_menu(); }
    if ui.selectable_label(kind == LayerKind::Guide, "Guide").on_hover_text("Guide layers don't export. A guide with layers under it is a motion guide").clicked() {
        app.ed.set_layer_kind(i, if kind == LayerKind::Guide { LayerKind::Normal } else { LayerKind::Guide });
        ui.close_menu();
    }
    if ui.button("Add Classic Motion Guide").on_hover_text("Classic tweens on this layer follow the first stroke on the guide").clicked() {
        app.ed.layer = i;
        app.ed.wrap_layer(LayerKind::Guide, "Motion Guide");
        app.status = "Draw a path on the Motion Guide layer (Pencil or Pen), then make a classic tween on the layer below.".into();
        ui.close_menu();
    }
    if ui.selectable_label(kind == LayerKind::Mask, "Mask").on_hover_text("Shapes on this layer reveal the layer below; everything else is hidden").clicked() {
        if kind == LayerKind::Mask { app.ed.set_layer_kind(i, LayerKind::Normal) } else { app.ed.make_mask(i) }
        ui.close_menu();
    }
    ui.separator();
    if kind == LayerKind::Normal {
        let cur = app.ed.timeline().layers.get(i).and_then(|l| l.anim_parent);
        ui.menu_button("Parent Layer", |ui| {
            if ui.selectable_label(cur.is_none(), "None").clicked() { app.ed.set_anim_parent(i, None); ui.close_menu(); }
            for p in app.ed.parent_candidates(i) {
                let name = app.ed.timeline().layers[p].name.clone();
                if ui.selectable_label(cur == Some(p), name).on_hover_text("This layer's objects follow the first object on that layer").clicked() {
                    app.ed.set_anim_parent(i, Some(p));
                    ui.close_menu();
                }
            }
        });
    }
    if ui.button("Layer Properties…").clicked() { app.open_layer_props(i); ui.close_menu(); }
}

pub fn panel(app: &mut App, ctx: &egui::Context) {
    egui::TopBottomPanel::new(app.layout.timeline_side(), "timeline").resizable(true).default_height(230.0).min_height(120.0).show(ctx, |ui| {
        toolbar(app, ui);
        grid(app, ui);
    });
}

fn toolbar(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        crate::dock::grip(app, ui, crate::dock::Module::Timeline, Some("Timeline"));
        ui.separator();
        if ui.button("⏮").on_hover_text("First frame").clicked() { app.step(-100_000); }
        if ui.button("◀").on_hover_text("Step back (,)").clicked() { app.step(-1); }
        if ui.button(if app.playing { "⏸" } else { "▶" }).on_hover_text("Play / stop (Enter)").clicked() { app.toggle_play(); }
        if ui.button("▶|").on_hover_text("Step forward (.)").clicked() { app.step(1); }
        if ui.button("⏭").on_hover_text("Last frame").clicked() { app.step(100_000); }
        ui.toggle_value(&mut app.loop_play, "🔁").on_hover_text("Loop playback (Ctrl+Alt+L loops the selected frames)");
        ui.separator();
        ui.toggle_value(&mut app.onion, "Onion skin").on_hover_text("Show previous (blue) and next (green) frames; drag the markers above the frames to change the range");
        ui.toggle_value(&mut app.onion_opts.outline, "Outlines").on_hover_text("Onion Skin Outlines");
        ui.toggle_value(&mut app.tl.parent_view, "Parenting").on_hover_text("Show Parenting View: drag from a layer's link dot onto another layer to parent it (Alt+click a dot to unparent)");
        ui.toggle_value(&mut app.onion_opts.multi, "Edit Multiple").on_hover_text("Edit Multiple Frames: show every keyframe between the markers and move them together");
        let fv = match app.tl.frame_view {
            FrameView::Normal => "Frames",
            FrameView::Preview => "Preview",
            FrameView::PreviewInContext => "In context",
        };
        ui.menu_button(fv, |ui| {
            for (v, l, tip) in [
                (FrameView::Normal, "Normal", "Plain frame cells"),
                (FrameView::Preview, "Preview", "Thumbnails of each keyframe's content, fitted to the span"),
                (FrameView::PreviewInContext, "Preview in Context", "Thumbnails of each keyframe as it sits on the stage"),
            ] {
                if ui.selectable_label(app.tl.frame_view == v, l).on_hover_text(tip).clicked() {
                    app.tl.frame_view = v;
                    ui.close_menu();
                }
            }
            ui.separator();
            let cw = app.tl.cw();
            for (name, w) in FRAME_SIZES {
                if ui.selectable_label((cw - w).abs() < 0.5, name).on_hover_text("Frame width (Ctrl+wheel over the frames zooms freely)").clicked() {
                    app.tl.cell_w = w;
                    app.settings.tl_cell_w = w;
                    app.settings.save();
                    ui.close_menu();
                }
            }
            ui.separator();
            if ui.checkbox(&mut app.tl.seconds, "Show Time in Seconds").changed() {
                app.settings.tl_seconds = app.tl.seconds;
                app.settings.save();
            }
        })
        .response
        .on_hover_text("Frame View: keyframe thumbnails in the timeline");
        ui.toggle_value(&mut app.onion_opts.shift_trace, "Shift & Trace").on_hover_text("Shift and Trace: show the previous and next keyframes as ghosts you can slide under the current drawing to trace in-betweens (view only)");
        if app.onion_opts.shift_trace {
            ui.toggle_value(&mut app.onion_opts.shift_edit, "Edit Shift").on_hover_text("Drag on the stage to move the previous ghost; Shift+drag moves the next one; Alt+drag turns it");
            if ui.small_button("Reset Shift").clicked() {
                app.onion_opts.shift = [ac_core::Matrix::IDENTITY; 2];
            }
        }
        if app.onion || app.onion_opts.multi {
            ui.menu_button("Markers", |ui| {
                let anchored = app.onion_opts.anchor.is_some();
                if ui.selectable_label(anchored, "Anchor Markers").clicked() {
                    app.onion_opts.anchor = if anchored { None } else { Some(app.onion_range()) };
                    ui.close_menu();
                }
                ui.checkbox(&mut app.onion_opts.keys_only, "Keyframes Only").on_hover_text("Ghost only the drawings (keyframes), not the in-between frames");
                ui.separator();
                for (label, n) in [("Onion 2", 2u32), ("Onion 5", 5), ("Onion All", u32::MAX)] {
                    if ui.button(label).clicked() {
                        if n == u32::MAX {
                            app.onion_opts.anchor = Some((0, app.ed.frame_count().saturating_sub(1)));
                        } else {
                            app.onion_opts.anchor = None;
                            app.onion_opts.before = n;
                            app.onion_opts.after = n;
                        }
                        ui.close_menu();
                    }
                }
            });
        }
        ui.separator();
        let n = app.ed.frame_count();
        let fps = app.ed.doc.fps;
        // click the counter to switch between frames and seconds
        let (main, rest, other) = if app.tl.seconds {
            (fmt_time(app.ed.frame, fps), format!("/ {}", fmt_time(n, fps)), format!("frame {}", app.ed.frame + 1))
        } else {
            (format!("{}", app.ed.frame + 1), format!("/ {n}"), format!("{:.1}s", app.ed.frame as f32 / fps))
        };
        let r = ui.add(egui::Label::new(egui::RichText::new(main).monospace().color(ACCENT).size(15.0)).sense(Sense::click()));
        let r2 = ui.add(egui::Label::new(egui::RichText::new(rest).monospace()).sense(Sense::click()));
        if r.on_hover_text("Click: show frames / seconds").clicked() || r2.clicked() {
            app.tl.seconds = !app.tl.seconds;
            app.settings.tl_seconds = app.tl.seconds;
            app.settings.save();
        }
        ui.label(egui::RichText::new(format!("{fps:.2} fps")).monospace().color(Color32::from_gray(160)));
        ui.label(egui::RichText::new(other).monospace().color(Color32::from_gray(160)));
        if let Some((a, b)) = app.tl.loop_range.filter(|_| app.loop_play) {
            ui.label(egui::RichText::new(format!("loop {}–{}", a + 1, b + 1)).monospace().color(Color32::from_rgb(0xF0, 0x9A, 0x3A))).on_hover_text("Loop range (Control › Clear Loop Range, or right-click the frame numbers)");
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("🗑").on_hover_text("Delete layer").clicked() { app.ed.delete_layer(); }
            if ui.button("📁").on_hover_text("New folder (puts the current layer in it)").clicked() { app.ed.wrap_layer(LayerKind::Folder, "Folder"); }
            if ui.button("➕ Layer").on_hover_text("New layer").clicked() { app.ed.add_layer(); }
            ui.separator();
            if ui.button("Clear").on_hover_text("Clear selected frames (Delete)").clicked() { app.ed.clear_frames(); }
            if ui.button("F7").on_hover_text("Insert blank keyframe").clicked() { app.ed.insert_blank_keyframe(); }
            if ui.button("F6").on_hover_text("Insert keyframe").clicked() { app.ed.insert_keyframe(); }
            if ui.button("F5").on_hover_text("Insert frame").clicked() { app.ed.insert_frame(); }
        });
    });
}

fn grid(app: &mut App, ui: &mut egui::Ui) {
    let cw = app.tl.cw();
    let avail = ui.available_size();
    let (rect, _) = ui.allocate_exact_size(avail, Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, Color32::from_gray(0x28));

    let grid_x0 = rect.left() + LW;
    let grid_w = (rect.right() - grid_x0).max(0.0);
    let visible_cols = (grid_w / cw).ceil() as u32 + 1;
    let rows_top = rect.top() + HH;
    let visible_rows = (((rect.bottom() - rows_top) / RH).floor() as usize).max(1);
    let n = app.ed.frame_count();
    let layers = app.ed.timeline().layers.clone();
    let nl = layers.len();
    // visible rows (layers inside collapsed folders are hidden)
    let rows: Vec<usize> = (0..nl).filter(|i| !app.ed.layer_collapsed_away(*i)).collect();
    let nr = rows.len().max(1);
    let depths: Vec<usize> = (0..nl).map(|i| app.ed.layer_depth(i)).collect();
    // per-row heights (Layer Properties › Layer height: 100/200/300 %) and their tops
    let preview = app.tl.frame_view != FrameView::Normal;
    let heights: Vec<f32> = rows.iter().map(|&i| RH * (layers[i].row_height.clamp(if preview { 200 } else { 100 }, 300) as f32 / 100.0)).collect();
    // thumbnails are rebuilt after any edit or context change
    let tkey = (app.ed.revision, app.ed.editing, app.ed.scene);
    if app.tl.thumbs_key != tkey {
        app.tl.thumbs.clear();
        app.tl.thumbs_key = tkey;
    }
    let mut offs = vec![0.0f32];
    for h in &heights {
        offs.push(offs.last().copied().unwrap_or(0.0) + h);
    }

    // Follow the playhead while playing; wheel scrolls (horizontal over frames, vertical over layers).
    if app.playing {
        let f = app.ed.frame as f32;
        if f < app.tl.scroll || f >= app.tl.scroll + visible_cols as f32 - 2.0 {
            app.tl.scroll = (f - 2.0).max(0.0);
        }
    }
    let grid_rect = Rect::from_min_max(pos2(grid_x0, rect.top()), rect.max);
    let layer_col = Rect::from_min_max(pos2(rect.left(), rows_top), pos2(grid_x0, rect.bottom()));
    let d = ui.input(|i| i.raw_scroll_delta);
    let (ctrl, zoom) = ui.input(|i| (i.modifiers.command, i.zoom_delta()));
    let cw = if ui.rect_contains_pointer(grid_rect) && ctrl && (zoom != 1.0 || d.y != 0.0) {
        // Ctrl+wheel: wider or narrower frames, keeping the frame under the pointer in place
        let f = if zoom != 1.0 { zoom.clamp(1.0 / 1.15, 1.15) } else if d.y > 0.0 { 1.15 } else { 1.0 / 1.15 };
        let new = (cw * f).clamp(CW_MIN, CW_MAX);
        if let Some(px) = ui.input(|i| i.pointer.hover_pos()) {
            let at = (px.x - grid_x0) / cw + app.tl.scroll;
            app.tl.scroll = (at - (px.x - grid_x0) / new).max(0.0);
        }
        app.tl.cell_w = new;
        app.settings.tl_cell_w = new;
        new
    } else {
        cw
    };
    let visible_cols = (grid_w / cw).ceil() as u32 + 1;
    if !ctrl && ui.rect_contains_pointer(grid_rect) && (d.x != 0.0 || d.y != 0.0) {
        let delta = if d.x.abs() > d.y.abs() { -d.x } else { -d.y };
        app.tl.scroll = (app.tl.scroll + delta / cw).clamp(0.0, n as f32 + 200.0);
    }
    if ui.rect_contains_pointer(layer_col) && d.y != 0.0 {
        let max = nr.saturating_sub(visible_rows);
        app.tl.vscroll = if d.y < 0.0 { (app.tl.vscroll + 1).min(max) } else { app.tl.vscroll.saturating_sub(1) };
    }
    app.tl.vscroll = app.tl.vscroll.min(nr.saturating_sub(1));
    let scroll = app.tl.scroll;
    let vscroll = app.tl.vscroll;
    let first = scroll.floor() as u32;
    let x_of = |f: u32| grid_x0 + (f as f32 - scroll) * cw;
    let top_off = offs[vscroll.min(rows.len())];
    let row_y = |i: usize| rows_top + offs[i.min(rows.len())] - top_off;
    // row under a y position (clamped to the last row)
    let row_at = |y: f32| -> usize { offs.partition_point(|&o| o <= y - rows_top + top_off).saturating_sub(1).min(nr.saturating_sub(1)) };
    let rows_bottom = rows_top + offs[rows.len()] - top_off;
    let cell_at = |pos: egui::Pos2| -> (usize, u32) {
        let f = (((pos.x - grid_x0) / cw) + scroll).floor().max(0.0) as u32;
        let r = row_at(pos.y);
        (rows.get(r).copied().unwrap_or(0), f)
    };

    // --- header (scrub)
    let header = Rect::from_min_size(pos2(grid_x0, rect.top()), vec2(grid_w, HH));
    p.rect_filled(header, 0.0, Color32::from_gray(0x33));
    p.rect_filled(Rect::from_min_size(rect.min, vec2(LW, HH)), 0.0, Color32::from_gray(0x33));
    p.text(rect.min + vec2(8.0, HH / 2.0), Align2::LEFT_CENTER, "Layers", FontId::proportional(12.0), Color32::from_gray(160));
    for (k, label) in ["👁", "🔒", "□"].iter().enumerate() {
        p.text(pos2(rect.left() + LW - 70.0 + k as f32 * 22.0, rect.top() + HH / 2.0), Align2::CENTER_CENTER, *label, FontId::proportional(11.0), Color32::from_gray(140));
    }
    let fps = app.ed.doc.fps;
    let step = label_step(cw, app.tl.seconds, fps);
    for f in first..first + visible_cols {
        let x = x_of(f);
        // frames: labels on 1, 5, 10 … (Animate's numbering); seconds: on whole-second frames
        let labelled = if app.tl.seconds { f % step == 0 } else { f == 0 || (f + 1) % step == 0 };
        let minor = cw >= 5.0 || labelled;
        if minor {
            let tick = if labelled { 6.0 } else { 3.0 };
            p.line_segment([pos2(x, header.bottom() - tick), pos2(x, header.bottom())], Stroke::new(1.0_f32, Color32::from_gray(110)));
        }
        if labelled {
            let label = if app.tl.seconds { fmt_time(f, fps) } else { format!("{}", f + 1) };
            p.text(pos2(x + 1.0, header.top() + 3.0), Align2::LEFT_TOP, label, FontId::monospace(10.0), Color32::from_gray(175));
        }
    }
    // Loop range: an orange bar along the top of the header with draggable ends.
    let loop_on = app.loop_play && app.tl.loop_range.is_some();
    let (la, lb) = app.tl.loop_range.unwrap_or((0, 0));
    let (lx0, lx1) = (x_of(la), x_of(lb + 1));
    if loop_on {
        let col = Color32::from_rgb(0xF0, 0x9A, 0x3A);
        let bar = Rect::from_min_max(pos2(lx0.max(grid_x0), header.top()), pos2(lx1.min(rect.right()), header.top() + 4.0));
        if bar.width() > 0.0 {
            p.rect_filled(bar, 0.0, col);
        }
        for x in [lx0, lx1] {
            if x >= grid_x0 - 1.0 && x <= rect.right() {
                p.line_segment([pos2(x, header.top()), pos2(x, header.bottom())], Stroke::new(1.5_f32, col));
            }
        }
    }
    // Button symbols: the first four frames are the Up / Over / Down / Hit states.
    let is_button = app.ed.editing.and_then(|id| app.ed.doc.symbol(id)).map(|s| s.kind == ac_core::SymbolKind::Button).unwrap_or(false);
    if is_button {
        for (f, name) in ["Up", "Over", "Down", "Hit"].iter().enumerate() {
            let f = f as u32;
            if f >= first && f < first + visible_cols {
                let r = Rect::from_min_size(pos2(x_of(f), header.top()), vec2(cw, HH - 6.0));
                p.rect_filled(r, 0.0, Color32::from_gray(0x33));
                p.text(pos2(x_of(f) + 1.0, header.top() + 3.0), Align2::LEFT_TOP, &name[..1], FontId::proportional(10.0), Color32::from_rgb(0xFF, 0xC8, 0x60));
            }
        }
    }
    // Onion skin markers: brackets at the ends of the range, draggable.
    let show_markers = app.onion || app.onion_opts.multi;
    let (oa, ob) = app.onion_range();
    let (mx0, mx1) = (x_of(oa), x_of(ob + 1));
    if show_markers {
        let col = if app.onion_opts.anchor.is_some() { Color32::from_rgb(0x9A, 0xB8, 0xFF) } else { Color32::from_gray(210) };
        let (yt, yb) = (header.top() + 2.0, header.bottom() - 2.0);
        p.rect_filled(Rect::from_min_max(pos2(mx0, header.bottom() - 4.0), pos2(mx1, header.bottom() - 2.0)), 0.0, col.gamma_multiply(0.35));
        for (x, dir) in [(mx0, 1.0), (mx1, -1.0)] {
            if x >= grid_x0 - 1.0 && x <= rect.right() {
                p.line_segment([pos2(x, yt), pos2(x, yb)], Stroke::new(2.0_f32, col));
                p.add(egui::Shape::convex_polygon(vec![pos2(x, yt), pos2(x + 6.0 * dir, yt), pos2(x, yt + 6.0)], col, Stroke::NONE));
            }
        }
    }
    let hresp = ui.interact(header, ui.id().with("tl-header"), Sense::click_and_drag());
    let origin = ui.input(|i| i.pointer.press_origin());
    if hresp.drag_started() && loop_on {
        if let Some(o) = origin {
            // the loop ends take precedence in the top half of the header
            if o.y < header.center().y || !show_markers {
                if (o.x - lx0).abs() < 5.0 {
                    app.tl.loop_drag = Some(true);
                } else if (o.x - lx1).abs() < 5.0 {
                    app.tl.loop_drag = Some(false);
                }
            }
        }
    }
    if hresp.drag_started() && show_markers && app.tl.loop_drag.is_none() {
        if let Some(o) = origin {
            if (o.x - mx0).abs() < 5.0 {
                app.tl.marker_drag = Some(true);
            } else if (o.x - mx1).abs() < 5.0 {
                app.tl.marker_drag = Some(false);
            }
        }
    }
    if hresp.drag_stopped() {
        app.tl.marker_drag = None;
        app.tl.loop_drag = None;
    }
    hresp.context_menu(|ui| {
        if ui.add(egui::Button::new("Loop Range from Selection").shortcut_text("Ctrl+Alt+L")).clicked() {
            app.set_loop_range();
            ui.close_menu();
        }
        if ui.add_enabled(app.tl.loop_range.is_some(), egui::Button::new("Clear Loop Range")).clicked() {
            app.tl.loop_range = None;
            ui.close_menu();
        }
        ui.separator();
        if ui.checkbox(&mut app.tl.seconds, "Show Time in Seconds").changed() {
            app.settings.tl_seconds = app.tl.seconds;
            app.settings.save();
        }
    });
    if let (Some(start), Some(pos), Some((a, b))) = (app.tl.loop_drag, hresp.interact_pointer_pos(), app.tl.loop_range) {
        let edge = ((pos.x - grid_x0) / cw + scroll).round().max(0.0) as u32;
        app.tl.loop_range = Some(if start { (edge.min(b), b) } else { (a, edge.saturating_sub(1).max(a)) });
    } else if let (Some(start), Some(pos)) = (app.tl.marker_drag, hresp.interact_pointer_pos()) {
        let edge = ((pos.x - grid_x0) / cw + first as f32).round().max(0.0) as u32;
        let cur = app.ed.frame;
        let o = &mut app.onion_opts;
        match (&mut o.anchor, start) {
            (Some(r), true) => r.0 = edge.min(r.1),
            (Some(r), false) => r.1 = edge.saturating_sub(1).max(r.0),
            (None, true) => o.before = cur.saturating_sub(edge.min(cur)),
            (None, false) => o.after = edge.saturating_sub(1).max(cur) - cur,
        }
    } else if let Some(pos) = hresp.interact_pointer_pos() {
        if hresp.clicked() || hresp.dragged() {
            app.playing = false;
            let (_, f) = cell_at(pos);
            app.ed.set_frame(f.min(n + 200));
            app.ed.frame_sel = None;
        }
    }

    // --- frame cells: one interaction for the whole area (selection, drag-move, context menu)
    let cells = Rect::from_min_max(pos2(grid_x0, rows_top), pos2(rect.right(), rows_bottom.max(rows_top + RH).min(rect.bottom())));
    let cresp = ui.interact(cells, ui.id().with("tl-cells"), Sense::click_and_drag());
    let shift = ui.input(|i| i.modifiers.shift);
    if cresp.hovered() || cresp.dragged() {
        // Clicking the timeline takes keyboard focus away from text fields.
        if cresp.clicked() || cresp.drag_started() || cresp.secondary_clicked() {
            if let Some(id) = ui.ctx().memory(|m| m.focused()) {
                ui.ctx().memory_mut(|m| m.surrender_focus(id));
            }
        }
    }
    if let Some(pos) = cresp.interact_pointer_pos() {
        let (l, f) = cell_at(pos);
        let pressed = cresp.drag_started() || cresp.clicked() || cresp.secondary_clicked();
        if std::env::var("AC_DEBUG").is_ok() && (pressed || cresp.dragged()) {
            eprintln!("tl: pressed={pressed} drag_started={} clicked={} dbl={} dragged={} gesture={:?} sel={:?} cell={:?} origin={:?}", cresp.drag_started(), cresp.clicked(), cresp.double_clicked(), cresp.dragged(), app.tl.gesture, app.ed.frame_sel, (l, f), ui.input(|i| i.pointer.press_origin()));
        }
        if pressed && app.tl.gesture == Gesture::None {
            // Gestures start where the button went down, not where the drag was detected.
            let (l, f) = ui.input(|i| i.pointer.press_origin()).filter(|_| cresp.drag_started()).map(cell_at).unwrap_or((l, f));
            app.playing = false;
            let inside = app.ed.frame_sel.map(|s| s.contains(l, f)).unwrap_or(false);
            if cresp.secondary_clicked() {
                if !inside {
                    app.ed.select_frames(FrameSel::cell(l, f));
                }
            } else if cresp.double_clicked() {
                app.ed.select_span(l, f);
                if app.ed.motion_span(l, f).is_some() {
                    // as in Animate: double-clicking a motion tween opens its Motion Editor
                    app.ed.frame_sel = None;
                    app.ed.layer = l;
                    app.ed.frame = f;
                    app.motion.open = true;
                }
            } else if shift {
                let a = app.tl.anchor.unwrap_or((app.ed.layer, app.ed.frame));
                app.ed.select_frames(FrameSel::between(a, (l, f)));
                app.ed.frame = f;
            } else if let Some(start) = cresp.drag_started().then(|| span_end_at(app, l, f, ui.input(|i| i.modifiers.command))).flatten() {
                app.tl.gesture = Gesture::SpanEnd { layer: l, start, end: f + 1 };
            } else if inside && cresp.drag_started() {
                app.tl.anchor = Some((l, f));
                app.tl.gesture = Gesture::Move(0);
            } else {
                app.tl.anchor = Some((l, f));
                app.ed.select_frames(FrameSel::cell(l, f));
                app.tl.gesture = if cresp.drag_started() { Gesture::Select } else { Gesture::None };
            }
        }
        if cresp.dragged() {
            match app.tl.gesture {
                Gesture::Select => {
                    if let Some(a) = app.tl.anchor {
                        let sel = FrameSel::between(a, (l, f));
                        app.ed.frame_sel = Some(sel);
                        app.ed.layer = sel.layer0;
                        app.ed.frame = f;
                    }
                }
                Gesture::Move(_) => {
                    if let Some(a) = app.tl.anchor {
                        app.tl.gesture = Gesture::Move(f as i64 - a.1 as i64);
                    }
                }
                Gesture::SpanEnd { layer, start, .. } => {
                    app.tl.gesture = Gesture::SpanEnd { layer, start, end: (f + 1).max(start + 1) };
                    app.status = format!("Span ends at frame {}", f + 1);
                }
                Gesture::None => {}
            }
        }
    }
    if cresp.drag_stopped() {
        if let Gesture::SpanEnd { layer, start, end } = app.tl.gesture {
            let old = app.ed.timeline().layers.get(layer).and_then(|l| l.span_at(start).map(|(_, _, n)| n.unwrap_or(l.end).min(l.end)));
            if old.is_some_and(|o| o != end) {
                app.ed.set_span_end(layer, start, end);
            }
        }
        if let Gesture::Move(delta) = app.tl.gesture {
            if delta != 0 {
                app.ed.move_frames(delta);
                app.status = format!("Moved frames by {delta}");
            }
        }
        app.tl.gesture = Gesture::None;
    }
    cresp.context_menu(|ui| frame_menu(app, ui));

    // --- parenting view: links from each child's dot to its parent's dot
    let link_lines: Vec<(usize, usize)> = if app.tl.parent_view { rows.iter().filter_map(|&i| layers[i].anim_parent.map(|p| (i, p))).collect() } else { vec![] };
    // --- rows
    let move_delta = if let Gesture::Move(d) = app.tl.gesture { d } else { 0 };
    let sel = app.ed.frame_sel;
    for (ri, &i) in rows.iter().enumerate().skip(vscroll) {
        let layer = &layers[i];
        let y = row_y(ri);
        let rh = heights[ri];
        if y > rect.bottom() {
            break;
        }
        let row_left = Rect::from_min_size(pos2(rect.left(), y), vec2(LW, rh));
        let selected = i == app.ed.layer;
        p.rect_filled(row_left, 0.0, if selected { Color32::from_rgb(0x2B, 0x4F, 0x7A) } else { Color32::from_gray(0x30) });
        p.line_segment([pos2(rect.left(), y + rh), pos2(rect.right(), y + rh)], Stroke::new(1.0_f32, Color32::from_gray(0x22)));

        // Name: click = select layer (+ its frames), double-click = rename, drag = reorder.
        // nesting indent + type icon (folder ▸/▾, mask, guide, camera)
        let indent = depths[i] as f32 * 14.0;
        let icon_c = pos2(rect.left() + 12.0 + indent, y + rh / 2.0);
        let ic = Color32::from_gray(200);
        match layer.kind {
            LayerKind::Folder => {
                let tri = if layer.collapsed {
                    vec![icon_c + vec2(-3.0, -5.0), icon_c + vec2(4.0, 0.0), icon_c + vec2(-3.0, 5.0)]
                } else {
                    vec![icon_c + vec2(-5.0, -3.0), icon_c + vec2(5.0, -3.0), icon_c + vec2(0.0, 4.0)]
                };
                p.add(egui::Shape::convex_polygon(tri, Color32::from_rgb(0xE8, 0xC0, 0x50), Stroke::NONE));
                if ui.interact(Rect::from_center_size(icon_c, vec2(16.0, 18.0)), ui.id().with(("fold", i)), Sense::click()).on_hover_text("Expand / collapse folder").clicked() {
                    app.ed.toggle_folder(i);
                }
            }
            LayerKind::Mask => {
                p.rect_stroke(Rect::from_center_size(icon_c, vec2(11.0, 11.0)), 1.0, Stroke::new(1.2_f32, ic));
                p.circle_filled(icon_c, 3.0, ic);
            }
            LayerKind::Guide => {
                let mut x = icon_c.x - 6.0;
                while x < icon_c.x + 6.0 {
                    p.line_segment([pos2(x, icon_c.y + 4.0 - (x - icon_c.x + 6.0) * 0.6), pos2(x + 2.5, icon_c.y + 4.0 - (x - icon_c.x + 8.5) * 0.6)], Stroke::new(1.4_f32, Color32::from_rgb(0x70, 0xB0, 0xFF)));
                    x += 4.0;
                }
            }
            LayerKind::Camera => {
                p.rect_stroke(Rect::from_center_size(icon_c - vec2(1.0, 0.0), vec2(9.0, 7.0)), 1.0, Stroke::new(1.2_f32, Color32::from_rgb(0xFF, 0xA0, 0x30)));
            }
            LayerKind::Normal => {
                if layer.parent.map(|p| layers.get(p).map(|l| l.kind == LayerKind::Mask).unwrap_or(false)).unwrap_or(false) {
                    p.circle_stroke(icon_c, 4.0, Stroke::new(1.1_f32, ic)); // masked
                } else if layer.parent.map(|p| layers.get(p).map(|l| l.kind == LayerKind::Guide).unwrap_or(false)).unwrap_or(false) {
                    p.circle_filled(icon_c, 2.5, Color32::from_rgb(0x70, 0xB0, 0xFF)); // guided
                }
            }
        }
        let link_w = if app.tl.parent_view { 18.0 } else { 0.0 };
        let name_rect = Rect::from_min_size(pos2(rect.left() + 22.0 + indent, y), vec2(LW - 104.0 - indent - link_w, rh));
        if app.tl.parent_view && layer.kind == LayerKind::Normal {
            let dot = link_dot(rect, y);
            let col = if layer.anim_parent.is_some() { c32(layer.color) } else { Color32::from_gray(120) };
            p.circle_stroke(dot, 4.0, Stroke::new(1.3_f32, col));
            if layer.anim_parent.is_some() {
                p.circle_filled(dot, 2.2, col);
            }
            let r = ui.interact(Rect::from_center_size(dot, vec2(16.0, 18.0)), ui.id().with(("link", i)), Sense::click_and_drag()).on_hover_text("Drag onto the parent layer · Alt+click: no parent");
            if r.clicked() && ui.input(|inp| inp.modifiers.alt) {
                app.ed.set_anim_parent(i, None);
            }
            if r.drag_started() {
                app.tl.link_drag = Some(i);
            }
            if r.drag_stopped() {
                if let (Some(from), Some(pos)) = (app.tl.link_drag.take(), ui.input(|inp| inp.pointer.latest_pos())) {
                    let to_row = row_at(pos.y);
                    let to = rows[to_row];
                    if to == from {
                        app.ed.set_anim_parent(from, None);
                    } else if app.ed.parent_candidates(from).contains(&to) {
                        app.ed.set_anim_parent(from, Some(to));
                        app.status = format!("{} follows {}", app.ed.timeline().layers[from].name, app.ed.timeline().layers[to].name);
                    } else {
                        app.status = "That layer can't be the parent (it would make a loop, or isn't a normal layer)".into();
                    }
                }
            }
        }
        let renaming = matches!(&app.tl.renaming, Some((ri, _)) if *ri == i);
        if renaming {
            let (_, buf) = app.tl.renaming.as_mut().unwrap();
            let r = ui.put(name_rect.shrink(2.0), egui::TextEdit::singleline(buf));
            r.request_focus();
            if r.lost_focus() {
                let (_, name) = app.tl.renaming.take().unwrap();
                if !name.trim().is_empty() {
                    app.ed.rename_layer(i, name.trim());
                }
            }
        } else {
            let lock_note = if layer.locked { "  🔒" } else { "" };
            let r = p.text(name_rect.left_center(), Align2::LEFT_CENTER, format!("{}{lock_note}", layer.name), FontId::proportional(13.0), if layer.visible { Color32::from_gray(225) } else { Color32::from_gray(120) });
            if let Some(pn) = layer.anim_parent.and_then(|pi| layers.get(pi)).map(|pl| pl.name.clone()) {
                // layer parenting: "→ Parent" after the name, in the parent's colour
                let col = layer.anim_parent.and_then(|pi| layers.get(pi)).map(|pl| c32(pl.color)).unwrap_or(Color32::GRAY);
                let (x, yc) = (r.right() + 6.0, name_rect.center().y);
                let st = Stroke::new(1.2_f32, col);
                p.line_segment([pos2(x, yc), pos2(x + 8.0, yc)], st);
                p.line_segment([pos2(x + 5.0, yc - 3.0), pos2(x + 8.0, yc)], st);
                p.line_segment([pos2(x + 5.0, yc + 3.0), pos2(x + 8.0, yc)], st);
                p.text(pos2(x + 11.0, yc), Align2::LEFT_CENTER, pn, FontId::proportional(10.0), col);
            }
            let r = ui.interact(name_rect, ui.id().with(("ln", i)), Sense::click_and_drag());
            if r.clicked() {
                app.ed.select_layer_frames(i);
            }
            if r.double_clicked() {
                app.tl.renaming = Some((i, layer.name.clone()));
            }
            if r.drag_started() {
                app.tl.layer_drag = Some(i);
            }
            if r.drag_stopped() {
                if let (Some(from), Some(pos)) = (app.tl.layer_drag.take(), ui.input(|inp| inp.pointer.latest_pos())) {
                    let to_row = row_at(pos.y);
                    let to = rows[to_row];
                    if to != from {
                        app.ed.drop_layer(from, to);
                    }
                }
            }
            let r = r.on_hover_text("Click: select layer · double-click: rename · drag: reorder or into a folder · right-click: layer menu");
            r.context_menu(|ui| layer_menu(app, ui, i));
        }
        // Eye / lock / outline toggles
        let icon = |k: usize| Rect::from_center_size(pos2(rect.left() + LW - 70.0 + k as f32 * 22.0, y + rh / 2.0), vec2(18.0, 18.0));
        let eye = icon(0);
        if layer.visible {
            p.circle_stroke(eye.center(), 5.0, Stroke::new(1.3_f32, Color32::from_gray(200)));
            p.circle_filled(eye.center(), 2.2, Color32::from_gray(200));
        } else {
            p.line_segment([eye.center() - vec2(5.0, 0.0), eye.center() + vec2(5.0, 0.0)], Stroke::new(1.6_f32, Color32::from_rgb(0xE5, 0x6B, 0x3B)));
        }
        let lock = icon(1);
        if layer.locked {
            p.rect_filled(Rect::from_center_size(lock.center() + vec2(0.0, 2.0), vec2(9.0, 7.0)), 1.0, Color32::from_gray(210));
            p.circle_stroke(lock.center() - vec2(0.0, 2.0), 3.0, Stroke::new(1.3_f32, Color32::from_gray(210)));
        } else {
            p.circle_filled(lock.center(), 1.6, Color32::from_gray(110));
        }
        let ol = icon(2);
        let sq = Rect::from_center_size(ol.center(), vec2(10.0, 10.0));
        if layer.outline {
            p.rect_stroke(sq, 0.0, Stroke::new(1.5_f32, c32(layer.color)));
        } else {
            p.rect_filled(sq, 0.0, c32(layer.color));
        }
        for (k, flag, tip) in [(0, LayerFlag::Visible, "Show / hide"), (1, LayerFlag::Locked, "Lock / unlock"), (2, LayerFlag::Outline, "Outline mode")] {
            if ui.interact(icon(k), ui.id().with(("lf", i, k)), Sense::click()).on_hover_text(tip).clicked() {
                app.ed.toggle_layer_flag(i, flag);
            }
        }

        // Frame cells
        let row = Rect::from_min_max(pos2(grid_x0, y), pos2(rect.right(), y + rh));
        let rp = ui.painter_at(row.intersect(rect));
        if layer.kind == LayerKind::Folder {
            rp.rect_filled(row, 0.0, Color32::from_gray(0x2E));
            continue;
        }
        for f in first..first + visible_cols {
            let x = x_of(f);
            let cell = Rect::from_min_size(pos2(x, y), vec2(cw, rh));
            let in_span = f < layer.end;
            let fill = if in_span {
                if (f + 1) % 5 == 0 { Color32::from_gray(0x52) } else { Color32::from_gray(0x47) }
            } else if (f + 1) % 5 == 0 {
                Color32::from_gray(0x30)
            } else {
                Color32::from_gray(0x2A)
            };
            rp.rect_filled(cell, 0.0, fill);
            rp.line_segment([pos2(x + cw, y), pos2(x + cw, y + rh)], Stroke::new(1.0_f32, Color32::from_gray(0x24)));
        }
        for (&start, kf) in &layer.keyframes {
            let end = layer.span_end(start);
            let (x0, x1) = (x_of(start), x_of(end));
            let span = Rect::from_min_max(pos2(x0, y + 1.0), pos2(x1, y + rh - 1.0));
            let has_next = layer.keyframes.contains_key(&end);
            match kf.tween {
                Tween::Classic { .. } | Tween::Shape { .. } => {
                    let col = if matches!(kf.tween, Tween::Classic { .. }) { CLASSIC } else { SHAPE };
                    rp.rect_filled(span, 0.0, col);
                    let ym = y + rh / 2.0;
                    let stroke = Stroke::new(1.0_f32, Color32::from_gray(20));
                    if has_next {
                        rp.line_segment([pos2(x0 + cw, ym), pos2(x1 - 3.0, ym)], stroke);
                        rp.line_segment([pos2(x1 - 6.0, ym - 3.0), pos2(x1 - 2.0, ym)], stroke);
                        rp.line_segment([pos2(x1 - 6.0, ym + 3.0), pos2(x1 - 2.0, ym)], stroke);
                    } else {
                        let mut x = x0 + cw;
                        while x < x1 - 4.0 {
                            rp.line_segment([pos2(x, ym), pos2((x + 3.0).min(x1), ym)], stroke);
                            x += 6.0;
                        }
                    }
                }
                Tween::Motion { .. } => {
                    rp.rect_filled(span, 0.0, MOTION);
                    if let Some(m) = &kf.motion {
                        for k in m.key_frames().into_iter().filter(|&k| k > 0 && start + k < end) {
                            let c = pos2(x_of(start + k) + cw / 2.0, y + rh - 7.0);
                            let d = 3.2;
                            rp.add(egui::Shape::convex_polygon(vec![c + vec2(0.0, -d), c + vec2(d, 0.0), c + vec2(0.0, d), c + vec2(-d, 0.0)], Color32::from_gray(20), Stroke::NONE));
                        }
                    }
                }
                Tween::None => {
                    if end > start + 1 {
                        let r = Rect::from_center_size(pos2(x1 - cw / 2.0, y + rh - 7.0), vec2(5.0, 7.0));
                        rp.rect_stroke(r, 0.0, Stroke::new(1.0_f32, Color32::from_gray(30)));
                    }
                }
            }
            if let Some(fs) = &kf.sound {
                draw_waveform(&rp, &app.ed.doc, fs, start, end, x0, x1, y, rh);
            }
            if preview && !kf.elements.is_empty() && rh > RH + 4.0 {
                let th = (rh - 16.0).max(8.0);
                let ctx_mode = app.tl.frame_view == FrameView::PreviewInContext;
                let aspect = if ctx_mode { app.ed.doc.width / app.ed.doc.height.max(1.0) } else { 1.0 };
                let tw = (th * aspect).min((x1 - x0 - 4.0).max(0.0));
                if tw >= 8.0 && x1 > grid_x0 && x0 < rect.right() {
                    let key = (i, start, ctx_mode, tw as u32, th as u32);
                    if !app.tl.thumbs.contains_key(&key) {
                        let img = keyframe_thumb(app, i, start, ctx_mode, tw as u32, th as u32);
                        let tex = img.map(|im| ui.ctx().load_texture(format!("tl-thumb-{i}-{start}"), im, egui::TextureOptions::LINEAR));
                        app.tl.thumbs.insert(key, tex);
                    }
                    if let Some(Some(tex)) = app.tl.thumbs.get(&key) {
                        let r = Rect::from_min_size(pos2(x0 + 2.0, y + 2.0), vec2(tw, th));
                        rp.rect_filled(r, 0.0, Color32::from_gray(if ctx_mode { 0xF2 } else { 0x5A }));
                        rp.image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    }
                }
            }
            let c = pos2(x0 + cw / 2.0, y + rh - 7.0);
            if kf.elements.is_empty() {
                rp.circle_stroke(c, 3.0, Stroke::new(1.2_f32, Color32::from_gray(20)));
            } else {
                rp.circle_filled(c, 3.3, Color32::from_gray(20));
            }
            if let Some(label) = &kf.label {
                // Animate's markers: red flag = name, green "//" = comment, gold anchor = anchor
                let (mark, col) = match kf.label_kind {
                    ac_core::LabelKind::Name => ("⚑", Color32::from_rgb(0xFF, 0x60, 0x60)),
                    ac_core::LabelKind::Comment => ("//", Color32::from_rgb(0x60, 0xD0, 0x70)),
                    ac_core::LabelKind::Anchor => ("⚓", Color32::from_rgb(0xF0, 0xC0, 0x40)),
                };
                rp.text(pos2(x0 + 2.0, y + 2.0), Align2::LEFT_TOP, format!("{mark}{label}"), FontId::proportional(10.0), col);
            }
        }
        if let Gesture::SpanEnd { layer: gl, start, end } = app.tl.gesture {
            if gl == i {
                let r = Rect::from_min_max(pos2(x_of(start), y + 1.0), pos2(x_of(end.max(start + 1)), y + rh - 1.0));
                rp.rect_stroke(r, 0.0, Stroke::new(2.0_f32, Color32::WHITE));
            }
        }
        // Frame selection highlight (and its ghost while dragging).
        if let Some(s) = sel {
            if (s.layer0..=s.layer1).contains(&i) {
                let r = Rect::from_min_max(pos2(x_of(s.f0), y), pos2(x_of(s.f1 + 1), y + rh));
                rp.rect_filled(r, 0.0, ACCENT.gamma_multiply(0.45));
                rp.rect_stroke(r, 0.0, Stroke::new(1.0_f32, ACCENT));
                if move_delta != 0 {
                    let g = r.translate(vec2(move_delta as f32 * cw, 0.0));
                    rp.rect_stroke(g, 0.0, Stroke::new(2.0_f32, Color32::WHITE));
                }
            }
        }
    }
    // Parenting links: a bracket from the child's dot to its parent's dot
    let row_of = |l: usize| rows.iter().position(|&r| r == l);
    for (k, &(c, par)) in link_lines.iter().enumerate() {
        let (Some(rc), Some(rpar)) = (row_of(c), row_of(par)) else { continue };
        let (yc, yp) = (row_y(rc) + heights[rc] / 2.0, row_y(rpar) + heights[rpar] / 2.0);
        let x = link_dot(rect, 0.0).x;
        let bend = x - 8.0 - (k % 3) as f32 * 3.0;
        let col = c32(layers[c].color);
        let st = Stroke::new(1.3_f32, col);
        let lp = ui.painter_at(Rect::from_min_max(pos2(rect.left(), rows_top), pos2(grid_x0, rect.bottom())));
        lp.add(egui::Shape::line(vec![pos2(x - 4.0, yc), pos2(bend, yc), pos2(bend, yp), pos2(x - 4.0, yp)], st));
        lp.circle_filled(pos2(x - 4.0, yp), 2.0, col);
    }
    if let (Some(from), Some(pos)) = (app.tl.link_drag, ui.input(|i| i.pointer.latest_pos())) {
        if let Some(r) = row_of(from) {
            p.line_segment([link_dot(rect, row_y(r)), pos], Stroke::new(1.5_f32, ACCENT));
        }
    }
    // Layer reorder indicator
    if let (Some(_), Some(pos)) = (app.tl.layer_drag, ui.input(|i| i.pointer.latest_pos())) {
        let to = row_at(pos.y);
        let y = row_y(to);
        p.line_segment([pos2(rect.left(), y), pos2(grid_x0, y)], Stroke::new(2.0_f32, ACCENT));
    }

    // Playhead
    let x = x_of(app.ed.frame);
    if x >= grid_x0 - cw && x <= rect.right() {
        let gp = ui.painter_at(grid_rect);
        let bottom = rows_bottom.max(rows_top + RH).min(rect.bottom());
        gp.line_segment([pos2(x + cw / 2.0, rect.top()), pos2(x + cw / 2.0, bottom)], Stroke::new(1.5_f32, PLAYHEAD));
        let tag = Rect::from_center_size(pos2(x + cw / 2.0, rect.top() + HH / 2.0), vec2(26.0, HH - 4.0));
        gp.rect_filled(tag, 3.0, PLAYHEAD);
        gp.text(tag.center(), Align2::CENTER_CENTER, format!("{}", app.ed.frame + 1), FontId::monospace(10.5), Color32::WHITE);
    }
    p.line_segment([pos2(grid_x0, rect.top()), pos2(grid_x0, rect.bottom())], Stroke::new(1.0_f32, Color32::from_gray(0x1E)));
}

/// The sound a keyframe starts, drawn across its span (Animate shows the
/// waveform in the frames). Event sounds play past the span, so only the
/// part inside it is drawn.
#[allow(clippy::too_many_arguments)]
fn draw_waveform(rp: &egui::Painter, doc: &ac_core::Document, fs: &ac_core::sound::FrameSound, start: u32, end: u32, x0: f32, x1: f32, y: f32, rh: f32) {
    if fs.sync == ac_core::sound::SoundSync::Stop {
        return;
    }
    let Some(p) = doc.sound(fs.sound).and_then(ac_audio::pcm) else { return };
    let len = p.seconds().max(1e-3);
    let total = if fs.forever { f32::INFINITY } else { len * fs.repeat.max(1) as f32 };
    let fps = doc.fps.max(1.0);
    let n = (x1 - x0).max(1.0) as usize;
    let secs = (end - start) as f32 / fps;
    let mid = y + rh / 2.0 - 2.0;
    let h = rh / 2.0 - 4.0;
    let col = Color32::from_rgb(0x3A, 0x8F, 0xD8);
    // per-pixel peaks; cached per (sound, width, span) since this runs every repaint
    type Key = (u32, usize, u32, u32, u32, bool);
    thread_local! {
        static CACHE: std::cell::RefCell<std::collections::HashMap<Key, Vec<(f32, f32)>>> = Default::default();
    }
    let key = (fs.sound, n, end - start, fps.to_bits(), fs.repeat, fs.forever);
    let pk = CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 256 {
            c.clear();
        }
        c.entry(key)
            .or_insert_with(|| {
                (0..n)
                    .map(|i| {
                        let t = secs * i as f32 / n as f32;
                        if t >= total {
                            return (0.0, 0.0);
                        }
                        let lt = t % len;
                        ac_audio::peaks(&p, lt, (lt + secs / n as f32).min(len), 1)[0]
                    })
                    .collect()
            })
            .clone()
    });
    for (i, (lo, hi)) in pk.iter().enumerate() {
        if hi - lo < 1e-4 {
            continue;
        }
        let x = x0 + i as f32 + 0.5;
        rp.line_segment([pos2(x, mid - hi * h), pos2(x, mid - lo * h)], Stroke::new(1.0_f32, col));
    }
}

/// A keyframe's content on its own as a small image: fitted to its bounds
/// (Preview) or the whole stage scaled down (Preview in Context).
fn keyframe_thumb(app: &App, li: usize, frame: u32, in_context: bool, w: u32, h: u32) -> Option<egui::ColorImage> {
    let doc = &app.ed.doc;
    let mut tl = app.ed.timeline().clone();
    let mut layer = tl.layers.get(li)?.clone();
    if layer.anim_parent.is_some() {
        let pm = ac_core::layer_parent_matrix(&tl, li, frame);
        for kf in layer.keyframes.values_mut() {
            for el in &mut kf.elements {
                el.matrix = pm.concat(&el.matrix);
            }
        }
    }
    layer.parent = None;
    layer.anim_parent = None;
    layer.visible = true;
    layer.kind = LayerKind::Normal;
    tl.layers = vec![layer];
    let ctx = ac_core::EvalCtx::new(doc, frame);
    let mut items = Vec::new();
    ac_core::evaluate(&ctx, &tl, frame, &ac_core::Matrix::IDENTITY, 1.0, 0, &mut items);
    let (lo, hi) = if in_context {
        (ac_core::pt(0.0, 0.0), ac_core::pt(doc.width, doc.height))
    } else {
        items.iter().filter_map(|it| it.path.transformed(&it.matrix).bounds()).reduce(|a, b| (ac_core::pt(a.0.x.min(b.0.x), a.0.y.min(b.0.y)), ac_core::pt(a.1.x.max(b.1.x), a.1.y.max(b.1.y))))?
    };
    let (bw, bh) = ((hi.x - lo.x).max(1.0), (hi.y - lo.y).max(1.0));
    let k = ((w as f32 - 2.0) / bw).min((h as f32 - 2.0) / bh);
    let view = ac_core::Matrix::translate((w as f32 - bw * k) / 2.0, (h as f32 - bh * k) / 2.0).concat(&ac_core::Matrix::scale(k, k)).concat(&ac_core::Matrix::translate(-lo.x, -lo.y));
    let mut pix = tiny_skia::Pixmap::new(w.max(1), h.max(1))?;
    ac_render::draw_items(&mut pix.as_mut(), &items, &ac_render::RenderOpts { view, ..Default::default() });
    Some(egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], pix.data()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playback_loops_inside_the_range() {
        // whole timeline
        assert_eq!(play_next(3, 10, true, None), Some((4, false)));
        assert_eq!(play_next(9, 10, true, None), Some((0, true)));
        assert_eq!(play_next(9, 10, false, None), None);
        // range 2..=5: 5 wraps to 2; before the range plays into it
        assert_eq!(play_next(5, 10, true, Some((2, 5))), Some((2, true)));
        assert_eq!(play_next(0, 10, true, Some((2, 5))), Some((1, false)));
        assert_eq!(play_next(4, 10, true, Some((2, 5))), Some((5, false)));
        // range ignored without looping; clamped to the timeline
        assert_eq!(play_next(5, 10, false, Some((2, 5))), Some((6, false)));
        assert_eq!(play_next(9, 10, true, Some((2, 50))), Some((2, true)));
    }

    #[test]
    fn header_labels_and_time_format() {
        assert_eq!(label_step(11.0, false, 24.0), 5);
        assert_eq!(label_step(5.0, false, 24.0), 10);
        assert_eq!(label_step(24.0, false, 24.0), 2);
        assert_eq!(label_step(11.0, true, 24.0), 24);
        assert_eq!(label_step(1.0, true, 24.0), 48);
        assert_eq!(fmt_time(48, 24.0), "2s");
        assert_eq!(fmt_time(30, 24.0), "1.25s");
        assert_eq!(fmt_time(24 * 75, 24.0), "1:15.0");
    }
}
