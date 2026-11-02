//! Control > Test (Ctrl+Enter): plays the whole movie in its own window, the main
//! timeline and each movie clip on its own frames, running the frame scripts
//! (minnamate_core::player), with the sounds. What they trace goes to the Output panel.
//! Its buttons answer the pointer: Over under it, Down while pressed, as far as their Hit
//! frame reaches.

use std::collections::BTreeSet;

use eframe::egui::{self, Color32, ColorImage, CursorIcon, Frame, Rect, TextureHandle, TextureOptions, ViewportBuilder, ViewportId, pos2, vec2};
use minnamate_core::button::{self, Live};
use minnamate_core::display::display_list;
use minnamate_core::doc::{Document, Point};
use minnamate_core::player::Player;
use minnamate_core::render::render_playing;

use crate::app::{Editor, PanelId};

pub struct TestMovie {
    start: f64,
    /// Frames played since `start`.
    played: u64,
    pub player: Player,
    /// The frame drawn, with a hash of what it was drawn from: frames played, scale, document
    /// revision and the pointer.
    texture: Option<(TextureHandle, u64)>,
}

impl TestMovie {
    pub fn new(now: f64, doc: &Document) -> Self {
        TestMovie { start: now, played: 0, player: Player::new(doc), texture: None }
    }
}

pub fn show(ctx: &egui::Context, ed: &mut Editor) {
    let Some(movie) = &mut ed.test else {
        // Closed, its sounds stop.
        return crate::sound::movie(&mut ed.sound, &ed.doc, None);
    };
    // The player keeps up with the clock, a frame each 1/fps s; one far behind drops frames.
    let fps = ed.doc.frame_rate.max(0.01) as f64;
    let due = ((ctx.input(|i| i.time) - movie.start).max(0.0) * fps) as u64;
    for _ in 0..due.saturating_sub(movie.played).min(4) {
        movie.player.step(&ed.doc);
    }
    movie.played = movie.played.max(due);
    crate::sound::movie(&mut ed.sound, &ed.doc, Some(movie.player.root.frame));
    let traced: Vec<String> = movie.player.output.drain(..).collect();
    let urls: Vec<(String, String)> = movie.player.urls.drain(..).collect();
    if !traced.is_empty() {
        // The Output panel comes forward with what the movie prints, as in Animate.
        (ed.show_bottom, ed.bottom_tab) = (true, 1);
        ed.closed.retain(|&p| p != PanelId::Output);
        traced.into_iter().for_each(|line| ed.log(line));
    }
    for (url, window) in urls {
        open_url(ed, &url, &window);
    }

    let Editor { doc, test, title, path, rev, .. } = ed;
    let Some(movie) = test else { return };
    let name = path.as_ref().and_then(|p| p.file_stem()).map_or_else(|| title.clone(), |s| s.to_string_lossy().into_owned());
    let ext = if doc.is_canvas() { "html" } else { "swf" };
    let builder = ViewportBuilder::default().with_title(format!("{name}.{ext}")).with_inner_size([doc.width, doc.height]);
    let mut close = false;
    ctx.show_viewport_immediate(ViewportId::from_hash_of("test-movie"), builder, |ui, _| {
        close = ui.input(|i| i.viewport().close_requested());
        let [r, g, b, _] = doc.background.map(|c| (c * 255.0) as u8);
        egui::CentralPanel::default().frame(Frame::NONE.fill(Color32::from_rgb(r, g, b))).show(ui, |ui| {
            // Scale to fit the window, keeping the aspect ratio (SWF "show all").
            let view = ui.max_rect();
            let scale = (view.size() / vec2(doc.width, doc.height)).min_elem().max(0.01);
            let rect = Rect::from_center_size(view.center(), vec2(doc.width, doc.height) * scale);
            // The pointer over a button, in stage pixels, and whether it's pressed.
            let pointer = ui.input(|i| i.pointer.hover_pos()).filter(|p| rect.contains(*p)).map(|p| ((p.x - rect.left()) / scale, (p.y - rect.top()) / scale));
            let live = pointer.filter(|&p| over_button(doc, movie.player.root.frame, p)).map(|p| (p, ui.input(|i| i.pointer.primary_down())));
            if live.is_some() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            if let Some(tex) = movie_texture(ui.ctx(), doc, movie, scale * ui.ctx().pixels_per_point(), *rev, live) {
                ui.painter().image(tex, rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            ui.ctx().request_repaint_after_secs(1.0 / fps as f32);
        });
    });
    if close {
        *test = None;
    }
}

/// Whether a button is under `p` (stage pixels) at `frame`, counted across the scenes.
// ponytail: movie clips are looked into at their first frame, not the one the player has them at.
fn over_button(doc: &Document, frame: u32, p: Point) -> bool {
    let mut local = frame;
    for scene in &doc.scenes {
        if local < scene.frame_count() {
            return button::under(doc, &display_list(doc, scene, local, &mut BTreeSet::new()), p);
        }
        local -= scene.frame_count();
    }
    false
}

fn movie_texture(ctx: &egui::Context, doc: &Document, movie: &mut TestMovie, scale: f32, rev: u32, live: Option<(Point, bool)>) -> Option<egui::TextureId> {
    let scale = scale.min(4096.0 / doc.width.max(doc.height).max(1.0));
    let live = live.map(|((x, y), down)| Live { pointer: (x * scale, y * scale), down });
    let key = crate::app::hash((movie.played, scale.to_bits(), rev, live.map(|l| (l.pointer.0.to_bits(), l.pointer.1.to_bits(), l.down))));
    if movie.texture.as_ref().map(|(_, k)| *k) != Some(key) {
        let (pixmap, _) = render_playing(doc, &movie.player, scale, live).ok()?;
        let image = ColorImage::from_rgba_premultiplied([pixmap.width() as usize, pixmap.height() as usize], pixmap.data());
        match &mut movie.texture {
            Some((tex, k)) => {
                tex.set(image, TextureOptions::LINEAR);
                *k = key;
            }
            None => movie.texture = Some((ctx.load_texture("test-movie", image, TextureOptions::LINEAR), key)),
        }
    }
    movie.texture.as_ref().map(|(tex, _)| tex.id())
}

/// navigateToURL from the movie: a web page opens in the browser, as Test Movie is asked to
/// open it; anything else is only noted.
fn open_url(ed: &mut Editor, url: &str, window: &str) {
    let web = ["http://", "https://", "mailto:"].iter().any(|s| url.starts_with(s)) && !url.chars().any(|c| c.is_whitespace() || c.is_control() || c == '"');
    if !web {
        return ed.log(format!("navigateToURL: {url} isn't a web page, so it isn't opened."));
    }
    ed.log(format!("navigateToURL: {url} ({window})"));
    // Tests never open a browser.
    if cfg!(test) {
        return;
    }
    // The page is one argument to the system's opener; no shell reads it.
    let opener = if cfg!(windows) { "explorer" } else if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    if let Err(e) = std::process::Command::new(opener).arg(url).spawn() {
        ed.log(format!("Couldn't open {url}: {e}"));
    }
}
