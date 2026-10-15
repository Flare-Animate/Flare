//! Sound in the editor (docs/animate-ui.md §7.2.5, §5.6, §7.3, §7.4): importing WAV and
//! MP3 files, putting them on keyframes, Properties > Frame > Sound, the waveforms in the
//! timeline and the Library's preview, and playing them with the playhead and Test Movie.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use eframe::egui::{self, Color32, Painter, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use minnamate_core::doc::{Document, Frame, LayerKind, Timeline};
use minnamate_core::sound::{self, Effect, FrameSound, Pcm, Sound, Sync};

use crate::app::Editor;
use crate::audio::{Owner, Player, Voice};
use crate::icons::Icon;
use crate::theme;
use crate::widgets::{self, Number, WORD_W};

/// The waveforms' green, over a span; darker on the Library's white preview.
const WAVE: Color32 = Color32::from_rgb(0x74, 0xC8, 0x6C);
const WAVE_ON_WHITE: Color32 = Color32::from_rgb(0x2F, 0x8A, 0x3B);

/// The editor's sound: the sounds decoded, the output, and what the playheads last did.
#[derive(Default)]
pub struct State {
    /// Each sound decoded, by its bytes' address (the copy of them kept here keeps that
    /// from being reused); None for one that can't be.
    decoded: RefCell<HashMap<usize, (Arc<[u8]>, Option<Arc<Decoded>>)>>,
    pub player: Player,
    /// The edited timeline's playhead as last seen: its frame, whether it played, and which
    /// timeline it was (the scene, and the symbols being edited).
    last: Option<(u32, bool, (usize, Vec<String>))>,
    /// Test Movie's frame as last seen.
    movie: Option<u32>,
    /// The Output panel was told there's no sound output.
    warned: bool,
}

/// A sound decoded, with its peaks for drawing.
pub struct Decoded {
    pub pcm: Arc<Pcm>,
    /// The lowest and highest sample of each run of `PEAK` samples, left and right.
    peaks: Vec<[(i16, i16); 2]>,
}

const PEAK: usize = 256;

impl Decoded {
    fn new(pcm: Pcm) -> Decoded {
        let peaks = (0..pcm.len().div_ceil(PEAK))
            .map(|b| {
                let run = b * PEAK..((b + 1) * PEAK).min(pcm.len());
                [0, 1].map(|c| run.clone().fold((0, 0), |(lo, hi), i| (lo.min(pcm.at(i, c)), hi.max(pcm.at(i, c)))))
            })
            .collect();
        Decoded { pcm: Arc::new(pcm), peaks }
    }

    pub fn seconds(&self) -> f64 {
        self.pcm.len() as f64 / self.pcm.rate.max(1) as f64
    }

    /// The lowest and highest level (-1..=1) of channel `c` (None for both) from `a` to `b`
    /// seconds in; None past the end.
    fn levels(&self, c: Option<usize>, a: f64, b: f64) -> Option<(f32, f32)> {
        let rate = self.pcm.rate.max(1) as f64;
        let i = (a * rate) as usize / PEAK;
        let j = ((b * rate) as usize / PEAK).max(i + 1).min(self.peaks.len());
        let peaks = self.peaks.get(i..j).filter(|p| !p.is_empty())?;
        let channels = c.map_or(0..2, |c| c..c + 1);
        let (lo, hi) = peaks.iter().flat_map(|p| p[channels.clone()].iter()).fold((0, 0), |(lo, hi), &(l, h)| (lo.min(l), hi.max(h)));
        Some((lo as f32 / 32768.0, hi as f32 / 32768.0))
    }
}

impl State {
    /// `s` decoded, once.
    // ponytail: on the UI thread, when first drawn or played, so a long MP3 stalls a frame; decode
    // on a thread if that's felt.
    pub fn decoded(&self, s: &Sound) -> Option<Arc<Decoded>> {
        let mut cache = self.decoded.borrow_mut();
        let entry = cache.entry(s.data.as_ptr() as usize).or_insert_with(|| (s.data.clone(), s.decode().ok().map(|pcm| Arc::new(Decoded::new(pcm)))));
        entry.1.clone()
    }
}

// ---- Import and attach ---------------------------------------------------------------------

/// Whether Import takes the file at `path` as a sound.
pub fn is_sound(path: &Path) -> bool {
    path.extension().is_some_and(|e| ["wav", "mp3"].iter().any(|x| e.eq_ignore_ascii_case(x)))
}

/// Adds the WAV or MP3 file at `path` to the library, and with `to_stage` puts it on the
/// current keyframe, as Import to Stage does with a sound. Returns its library name.
pub fn import(ed: &mut Editor, path: &Path, to_stage: bool) -> Option<String> {
    let file = path.file_name()?.to_string_lossy().into_owned();
    let read = std::fs::read(path).map_err(|e| e.to_string());
    // A new name, and a file of its own to save it in, when it's taken.
    let name = unique(&file, |n| ed.doc.sounds.contains_key(n) || ed.doc.library.contains_key(n));
    let href = unique(&file, |h| ed.doc.sounds.values().any(|s| s.extra.get("href").is_some_and(|x| x.eq_ignore_ascii_case(h))));
    let sound = match read.and_then(|data| Sound::from_file(data, &href).map_err(|e| e.to_string())) {
        Ok(s) => s,
        Err(e) => {
            ed.error = Some(format!("Couldn't import {}.\n\n{e}", path.display()));
            return None;
        }
    };
    ed.checkpoint();
    ed.doc.sounds.insert(name.clone(), sound);
    ed.library_selection = Some(name.clone());
    ed.log(format!("Imported {}", path.display()));
    if to_stage {
        put(ed, (ed.layer, ed.frame), &name);
    }
    Some(name)
}

/// Plays library sound `name` from the start, as the Library's Play does.
pub fn preview(ed: &mut Editor, name: &str) {
    ed.sound.player.stop(|v| v.owner == Owner::Library);
    let pcm = ed.doc.sounds.get(name).and_then(|s| ed.sound.decoded(s)).map(|d| d.pcm.clone());
    if let Some(v) = pcm.and_then(|pcm| Voice::new(Owner::Library, pcm, &FrameSound::new(name), 0.0, None)) {
        ed.sound.player.play(v);
    }
}

/// Renames the library sounds `renamed` gives a new name (None keeps it), and the
/// keyframes playing them.
pub fn rename(doc: &mut Document, renamed: &dyn Fn(&str) -> Option<String>) {
    let moved: Vec<(String, String)> = doc.sounds.keys().filter_map(|n| Some((n.clone(), renamed(n)?))).collect();
    for (from, to) in moved {
        if let Some(s) = doc.sounds.remove(&from) {
            doc.sounds.insert(to, s);
        }
    }
    for_keyframes(doc, &mut |key| {
        if let Some(mut s) = key.sound()
            && let Some(to) = renamed(&s.name)
        {
            s.name = to;
            key.set_sound(Some(&s));
        }
    });
}

/// Takes library sounds `gone` out, and off the keyframes playing them, as the Library's trash does.
pub fn delete(doc: &mut Document, gone: &[String]) {
    for name in gone {
        doc.sounds.remove(name);
    }
    for_keyframes(doc, &mut |key| {
        if key.sound().is_some_and(|s| gone.contains(&s.name)) {
            key.set_sound(None);
        }
    });
}

fn for_keyframes(doc: &mut Document, f: &mut dyn FnMut(&mut Frame)) {
    let timelines = doc.scenes.iter_mut().chain(doc.library.values_mut().map(|s| &mut s.timeline));
    timelines.flat_map(|tl| &mut tl.layers).flat_map(|l| &mut l.frames).for_each(f);
}

/// `name`, or "name 2.ext", "name 3.ext"... the first `taken` doesn't take.
fn unique(name: &str, taken: impl Fn(&str) -> bool) -> String {
    let (stem, ext) = name.rsplit_once('.').map_or((name, String::new()), |(s, e)| (s, format!(".{e}")));
    (1..).map(|n| if n == 1 { name.to_string() } else { format!("{stem} {n}{ext}") }).find(|n| !taken(n)).unwrap_or_default()
}

/// Puts library sound `name` on the keyframe at `frame` of layer `l` of the edited timeline:
/// the one whose span holds it, or one made there past the layer's end. A sound it had
/// gives way, its sync and repeat kept. One undo step.
pub fn attach(ed: &mut Editor, at: (usize, u32), name: &str) {
    let can = ed.timeline().layers.get(at.0).is_some_and(|l| l.kind != LayerKind::Folder);
    if can && ed.doc.sounds.contains_key(name) {
        ed.checkpoint();
        put(ed, at, name);
    }
}

/// `attach`, inside a checkpoint already made.
fn put(ed: &mut Editor, (l, frame): (usize, u32), name: &str) {
    let Some(layer) = ed.timeline_mut().layers.get_mut(l).filter(|l| l.kind != LayerKind::Folder) else { return };
    if layer.keyframe_at(frame).is_none() {
        layer.insert_blank_keyframe(frame);
    }
    let Some(key) = layer.keyframe_at(frame).map(|k| &mut layer.frames[k]) else { return };
    let s = match key.sound() {
        Some(old) => FrameSound { name: name.into(), in_point: 0, out_point: None, envelope: Vec::new(), ..old },
        None => FrameSound::new(name),
    };
    key.set_sound(Some(&s));
    ed.layer = l;
}

// ---- Properties > Frame > Sound ------------------------------------------------------------

/// Properties > Frame > Sound for keyframe `k` of layer `l`, which is `key`: the sound
/// (None or a library sound), its effect, sync, repeat or loop, and what the sound is.
pub fn section(ui: &mut Ui, ed: &mut Editor, (l, k): (usize, usize), key: &Frame) {
    let had = key.sound();
    let mut s = had.clone();
    let mut repeat_field = None;
    theme::section(ui, "Sound", |ui| {
        let mut names: Vec<&str> = ed.doc.sounds.keys().map(String::as_str).collect();
        // One that's gone from the library still shows.
        if let Some(gone) = had.as_ref().filter(|h| !ed.doc.sounds.contains_key(&h.name)) {
            names.push(&gone.name);
        }
        widgets::row(ui, |ui| {
            widgets::label(ui, "Name", WORD_W);
            let current = s.as_ref().and_then(|s| names.iter().position(|n| *n == s.name)).map_or(0, |i| i + 1);
            let items: Vec<(usize, &str, bool)> = std::iter::once((0, "None", true)).chain(names.iter().enumerate().map(|(i, n)| (i + 1, *n, true))).collect();
            if let Some(i) = widgets::pick(ui, "Sound name", current, &items, ui.available_width() - 4.0) {
                s = (i > 0).then(|| match &s {
                    // Another sound keeps how it plays, but not where the last one started or its envelope.
                    Some(old) => FrameSound { name: names[i - 1].into(), in_point: 0, out_point: None, envelope: Vec::new(), ..old.clone() },
                    None => FrameSound::new(names[i - 1]),
                });
            }
        });
        let shown = s.clone().unwrap_or_else(|| FrameSound::new(""));
        let mut cur = shown.clone();
        ui.add_enabled_ui(s.is_some(), |ui| {
            widgets::row(ui, |ui| {
                widgets::label(ui, "Effect", WORD_W);
                // ponytail: no envelope editor yet, so Custom and the edit button stay greyed.
                let effects = [
                    (Effect::None, "None", true),
                    (Effect::LeftChannel, "Left channel", true),
                    (Effect::RightChannel, "Right channel", true),
                    (Effect::FadeLeftToRight, "Fade left to right", true),
                    (Effect::FadeRightToLeft, "Fade right to left", true),
                    (Effect::FadeIn, "Fade in", true),
                    (Effect::FadeOut, "Fade out", true),
                    (Effect::Custom, "Custom", false),
                ];
                if let Some(e) = widgets::pick(ui, "Sound effect", cur.effect, &effects, ui.available_width() - 38.0) {
                    // A preset makes its own volumes.
                    (cur.effect, cur.envelope) = (e, Vec::new());
                }
                widgets::quick(ui, Icon::EditEnvelope, false, false, "Edit sound envelope");
            });
            widgets::row(ui, |ui| {
                widgets::label(ui, "Sync", WORD_W);
                let syncs = [(Sync::Event, "Event", true), (Sync::Start, "Start", true), (Sync::Stop, "Stop", true), (Sync::Stream, "Stream", true)];
                if let Some(v) = widgets::pick(ui, "Sound sync", cur.sync, &syncs, ui.available_width() - 4.0) {
                    cur.sync = v;
                }
            });
            widgets::row(ui, |ui| {
                widgets::label(ui, "", WORD_W);
                if let Some(v) = widgets::pick(ui, "Sound repeat", cur.looping, &[(false, "Repeat", true), (true, "Loop", true)], 96.0) {
                    cur.looping = v;
                }
                if !cur.looping {
                    ui.label(RichText::new("x").color(theme::TEXT_DIM));
                    let mut times = cur.repeat as f32;
                    let field = ui.add(Number::new("Repeat count", &mut times, 1.0..=9999.0).speed(0.2));
                    cur.repeat = times.round().max(1.0) as u32;
                    repeat_field = Some(field);
                }
            });
        });
        if s.is_some() && cur != shown {
            s = Some(cur);
        }
        let about = s.as_ref().and_then(|s| ed.doc.sounds.get(&s.name)).and_then(Sound::info).map(|i| {
            let size = i.samples as f64 * i.channels as f64 * i.bits as f64 / 8.0 / 1000.0;
            let channels = if i.channels > 1 { "Stereo" } else { "Mono" };
            format!("{} kHz {channels} {} Bit {:.1} s {size:.1} kB", i.rate / 1000, i.bits, i.seconds())
        });
        widgets::row(ui, |ui| ui.label(RichText::new(about.as_deref().unwrap_or("No sound selected")).color(theme::TEXT_DIM)));
    });
    if s != had {
        // Scrubbing the count is one undo step, as typing it is.
        match &repeat_field {
            Some(f) if f.changed() => ed.checkpoint_gesture("frame-sound-repeat"),
            _ => ed.checkpoint(),
        }
        if let Some(key) = ed.timeline_mut().layers.get_mut(l).and_then(|layer| layer.frames.get_mut(k)) {
            key.set_sound(s.as_ref());
        }
    }
    if let Some(f) = repeat_field {
        widgets::settle(ed, &f);
    }
}

// ---- Waveforms -------------------------------------------------------------------------------

/// Keyframe `key`'s sound, if it has one, as a waveform across its span (`span`, a frame
/// `cell_w` wide): what plays of it frame by frame, repeats and loops included.
pub fn paint_wave(p: &Painter, ed: &Editor, key: &Frame, span: Rect, cell_w: f32) {
    let Some(s) = key.sound() else { return };
    let Some(d) = ed.doc.sounds.get(&s.name).and_then(|x| ed.sound.decoded(x)) else { return };
    let fps = ed.doc.frame_rate.max(0.01) as f64;
    let (from, to) = (s.in_point as f64 / 44100.0, s.out_point.map_or(d.seconds(), |o| o as f64 / 44100.0).min(d.seconds()));
    if to <= from {
        return;
    }
    let passes = if s.looping { f64::INFINITY } else { s.repeat.max(1) as f64 };
    let (mid, half) = (span.center().y, span.height() * 0.32);
    let clip = p.clip_rect().intersect(span);
    let seconds = |x: f32| (x - span.left()) as f64 / cell_w as f64 / fps;
    let mut x = clip.left().floor();
    while x < clip.right() {
        let (a, b) = (seconds(x), seconds(x + 1.0));
        if a / (to - from) >= passes {
            break;
        }
        let into = |t: f64| from + t % (to - from);
        if let Some((lo, hi)) = d.levels(None, into(a), into(a) + (b - a)) {
            bar(p, x, mid, half, (lo, hi), WAVE);
        }
        x += 1.0;
    }
}

/// One pixel column `x` of a waveform around `mid`: from level `lo` to `hi` (-1..=1) of
/// `half` its height, a pixel at least, so silence shows as a line.
fn bar(p: &Painter, x: f32, mid: f32, half: f32, (lo, hi): (f32, f32), color: Color32) {
    let mid = mid.floor();
    let (top, bottom) = ((mid - hi * half).floor().min(mid), (mid - lo * half).ceil().max(mid + 1.0));
    p.rect_filled(Rect::from_x_y_ranges(x..=x + 1.0, top..=bottom), 0.0, color);
}

/// The Library's preview of the selected item, when it's a sound: its two channels'
/// waveforms in `rect`, with Stop and Play at the top right.
pub fn library_preview(ui: &mut Ui, ed: &mut Editor, rect: Rect) {
    let Some(name) = ed.library_selection.clone() else { return };
    let Some(d) = ed.doc.sounds.get(&name).and_then(|s| ed.sound.decoded(s)) else { return };
    let p = ui.painter_at(rect);
    let band = |c: usize| Rect::from_min_size(pos2(rect.left(), rect.top() + 26.0 + c as f32 * (rect.height() - 30.0) / 2.0), vec2(rect.width(), (rect.height() - 30.0) / 2.0));
    for c in 0..2 {
        let r = band(c);
        p.hline(r.x_range(), r.center().y, Stroke::new(1.0, Color32::from_gray(200)));
        for x in 0..r.width() as usize {
            let t = |x: usize| x as f64 / r.width() as f64 * d.seconds();
            if let Some((lo, hi)) = d.levels(Some(c), t(x), t(x + 1)) {
                bar(&p, r.left() + x as f32, r.center().y, r.height() * 0.45, (lo, hi), WAVE_ON_WHITE);
            }
        }
    }
    // Stop and Play, as Animate puts them on a sound's preview.
    for (i, (tip, play)) in [("Stop", false), ("Play", true)].into_iter().enumerate() {
        let b = Rect::from_center_size(pos2(rect.right() - 34.0 + i as f32 * 18.0, rect.top() + 12.0), vec2(16.0, 16.0));
        let resp = ui.interact(b, ui.id().with(("sound-preview", tip)), Sense::click()).on_hover_text(tip);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
        let ink = if resp.hovered() { Color32::BLACK } else { Color32::from_gray(90) };
        let c = b.center();
        if play {
            p.add(egui::Shape::convex_polygon(vec![c + vec2(-4.0, -5.0), c + vec2(5.0, 0.0), c + vec2(-4.0, 5.0)], ink, Stroke::NONE));
        } else {
            p.rect_filled(Rect::from_center_size(c, vec2(9.0, 9.0)), 1.0, ink);
        }
        if resp.clicked() && play {
            preview(ed, &name);
        } else if resp.clicked() {
            ed.sound.player.stop(|v| v.owner == Owner::Library);
        }
    }
}

// ---- Playing ---------------------------------------------------------------------------------

/// What playing a stretch of frames does: stop a sound, or start one.
enum Op {
    Stop(String),
    Play(Voice),
}

/// What playing `tls` (one after another) from just past `last` (or joining at `frame`,
/// streams under way and all) up to `frame` does to `owner`'s sounds, in step with it.
fn ops(state: &State, doc: &Document, tls: &[Timeline], owner: Owner, last: Option<u32>, frame: u32) -> Vec<Op> {
    let fps = doc.frame_rate.max(0.01) as f64;
    let pcm = |name: &str| Some(state.decoded(doc.sounds.get(name)?)?.pcm.clone());
    let sounds = sound::sounds(tls);
    let mut ops = Vec::new();
    let play = |start: u32, key: &Frame, s: &FrameSound, ops: &mut Vec<Op>| {
        // A stream ends with its keyframe's frames.
        let limit = (s.sync == Sync::Stream).then(|| (start + key.duration).saturating_sub(frame) as f64 / fps);
        let voice = pcm(&s.name).and_then(|pcm| Voice::new(owner, pcm, s, frame.saturating_sub(start) as f64 / fps, limit));
        ops.extend(voice.map(Op::Play));
    };
    let first = match last {
        Some(last) => last + 1,
        None => {
            for (start, key, s) in sounds.iter().filter(|(start, key, s)| s.sync == Sync::Stream && *start < frame && frame < start + key.duration) {
                play(*start, key, s, &mut ops);
            }
            frame
        }
    };
    for f in first..=frame {
        for (start, key, s) in sounds.iter().filter(|(start, ..)| *start == f) {
            match s.sync {
                Sync::Stop => ops.push(Op::Stop(s.name.clone())),
                Sync::Start if state.player.playing(|v| v.owner == owner && v.name == s.name) => {}
                _ => play(*start, key, s, &mut ops),
            }
        }
    }
    ops
}

fn apply(state: &mut State, owner: Owner, ops: Vec<Op>) {
    for op in ops {
        match op {
            Op::Stop(name) => state.player.stop(|v| v.owner == owner && v.name == name),
            Op::Play(v) => state.player.play(v),
        }
    }
}

/// Plays the edited timeline's sounds with its playhead, as Animate does: playback joins
/// the stream sounds under way, and each frame it reaches starts, stops or streams its
/// keyframes' sounds; scrubbing plays a frame's worth of the streams; stopping silences all.
pub fn tick(ed: &mut Editor) {
    // Sounds nothing holds any more (gone with their document and its undo steps) are let go.
    ed.sound.decoded.get_mut().retain(|_, (bytes, _)| Arc::strong_count(bytes) > 1);
    let place = (ed.scene, ed.editing.iter().map(|t| t.symbol.clone()).collect());
    let now = (ed.frame, ed.playing.is_some(), place);
    let Some(last) = ed.sound.last.replace(now.clone()) else { return };
    if last == now {
        return;
    }
    let (frame, playing, same) = (now.0, now.1, last.2 == now.2);
    let on = playing && last.1 && same && frame > last.0;
    // Looping back round, the event sounds play on.
    let round = playing && last.1 && same && frame < last.0;
    if !on {
        ed.sound.player.stop(|v| v.owner == Owner::Editor && (v.stream || !round));
    }
    let tls = std::slice::from_ref(ed.timeline());
    let ops = if playing {
        ops(&ed.sound, &ed.doc, tls, Owner::Editor, on.then_some(last.0), frame)
    } else if same && !last.1 {
        // Scrubbed: the streams there, for a frame.
        let fps = ed.doc.frame_rate.max(0.01) as f64;
        let sounds = sound::sounds(tls);
        let under = sounds.iter().filter(|(start, key, s)| s.sync == Sync::Stream && *start <= frame && frame < start + key.duration);
        let voices = under.filter_map(|(start, _, s)| {
            let pcm = ed.sound.decoded(ed.doc.sounds.get(&s.name)?)?.pcm.clone();
            Voice::new(Owner::Editor, pcm, s, (frame - start) as f64 / fps, Some(1.0 / fps))
        });
        voices.map(Op::Play).collect()
    } else {
        Vec::new()
    };
    apply(&mut ed.sound, Owner::Editor, ops);
    if let Some(e) = ed.sound.player.failed.as_ref().filter(|_| !ed.sound.warned) {
        let line = format!("Warning: no sound output ({e})");
        ed.sound.warned = true;
        ed.log(line);
    }
}

/// Plays the document's sounds with Test Movie, which is at `frame` of its scenes played
/// one after another; None once it's closed, which silences them.
// ponytail: the scenes' sounds only; movie clips play in Test Movie (player.rs) without theirs.
// Follow each clip's frame from `Player` when they should.
pub fn movie(state: &mut State, doc: &Document, frame: Option<u32>) {
    let last = std::mem::replace(&mut state.movie, frame);
    let Some(frame) = frame else {
        if last.is_some() {
            state.player.stop(|v| v.owner == Owner::Movie);
        }
        return;
    };
    if last == Some(frame) {
        return;
    }
    let on = last.is_some_and(|l| frame > l);
    // Round again, its streams start over while event sounds play on.
    if !on {
        state.player.stop(|v| v.owner == Owner::Movie && v.stream);
    }
    let ops = ops(state, doc, &doc.scenes, Owner::Movie, last.filter(|_| on), frame);
    apply(state, Owner::Movie, ops);
}

#[cfg(test)]
mod tests {
    use eframe::egui::Modifiers;
    use egui_kittest::kittest::Queryable;
    use minnamate_core::doc::Document;

    use super::*;
    use crate::app::testkit::*;
    use crate::app::{Action, PropsTab};

    /// A 16-bit mono WAV file `seconds` long at 22 kHz, sample `i` (at `t` seconds) `f(i, t)`.
    fn wav(seconds: f32, f: impl Fn(usize, f32) -> f32) -> Vec<u8> {
        let rate = 22050;
        let n = (rate as f32 * seconds) as usize;
        let samples = (0..n).map(|i| (f(i, i as f32 / rate as f32).clamp(-1.0, 1.0) * 32767.0) as i16);
        let body: Vec<u8> = samples.flat_map(i16::to_le_bytes).collect();
        let mut out = b"RIFF".to_vec();
        out.extend((36 + body.len() as u32).to_le_bytes());
        out.extend(b"WAVEfmt ");
        out.extend([16, 0, 0, 0, 1, 0, 1, 0]);
        out.extend((rate as u32).to_le_bytes());
        out.extend((rate as u32 * 2).to_le_bytes());
        out.extend([2, 0, 16, 0]);
        out.extend(b"data");
        out.extend((body.len() as u32).to_le_bytes());
        out.extend(body);
        out
    }

    /// A 330 Hz tone, `seconds` long.
    fn tone_wav(seconds: f32) -> Vec<u8> {
        wav(seconds, |_, t| (t * 330.0 * std::f32::consts::TAU).sin() * 0.8)
    }

    fn tone(seconds: f32) -> Sound {
        Sound::from_file(tone_wav(seconds), "tone.wav").unwrap()
    }

    /// `beats` a second, each a burst of noise and a low tone dying away: a drum loop, roughly.
    fn drums(seconds: f32, beats: f32) -> Vec<u8> {
        wav(seconds, |i, t| {
            let noise = ((i as u32).wrapping_mul(2_654_435_761) >> 16) as f32 / 32768.0 - 1.0;
            let hit = (-(t * beats).fract() * 7.0).exp();
            hit * (0.55 * noise + 0.45 * (t * 110.0 * std::f32::consts::TAU).sin()) * (0.6 + 0.4 * (t * 1.3).sin().abs())
        })
    }

    /// An empty folder for one test.
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("minnamate-sound-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Opens the dropdown named `dropdown` and picks `item` from its list.
    fn pick(h: &mut Harness<'_, Editor>, dropdown: &str, item: &str) {
        let field = h.get_by_label(dropdown).rect();
        h.get_by_label(dropdown).click();
        h.run();
        // The list opens under the field; the same words may be elsewhere in the window.
        let listed = h.get_all_by_label(item).find(|n| (n.rect().left() - field.left()).abs() < 12.0 && n.rect().top() > field.top());
        listed.unwrap_or_else(|| panic!("{item} isn't listed under {dropdown}")).click();
        h.run();
    }

    #[test]
    fn properties_puts_a_sound_on_the_keyframe() {
        let mut h = blank();
        h.state_mut().doc.sounds.insert("tone.wav".into(), tone(1.0));
        h.run();
        click_cell(&mut h, (0, 0), Modifiers::NONE);
        h.run();
        assert_eq!(h.state().props_tab, PropsTab::Frame);
        assert!(h.query_by_label("No sound selected").is_some());

        pick(&mut h, "Sound name", "tone.wav");
        assert_eq!(h.state().doc.scenes[0].layers[0].frames[0].sound(), Some(FrameSound::new("tone.wav")));
        assert!(h.query_by_label("22 kHz Mono 16 Bit 1.0 s 44.1 kB").is_some(), "what the sound is");

        pick(&mut h, "Sound sync", "Stream");
        pick(&mut h, "Sound repeat", "Loop");
        let s = h.state().doc.scenes[0].layers[0].frames[0].sound().unwrap();
        assert_eq!((s.sync, s.looping), (Sync::Stream, true));

        // Each setting is an undo step; None takes the sound off.
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::Undo, &ctx);
        let s = h.state().doc.scenes[0].layers[0].frames[0].sound().unwrap();
        assert_eq!((s.sync, s.looping), (Sync::Stream, false));
        h.run();
        pick(&mut h, "Sound name", "None");
        assert_eq!(h.state().doc.scenes[0].layers[0].frames[0].sound(), None);
    }

    #[test]
    fn importing_and_dropping_sound_files() {
        let dir = scratch("import");
        let (wav_path, mp3_path, junk) = (dir.join("tone.wav"), dir.join("beep.mp3"), dir.join("junk.wav"));
        std::fs::write(&wav_path, tone_wav(0.5)).unwrap();
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tnztools/tests/fixtures/sound/beep.mp3");
        std::fs::copy(fixture, &mp3_path).unwrap();
        std::fs::write(&junk, b"RIFF....WAVEnope").unwrap();

        // Dropped on the window, a sound comes in as Import to Stage brings it: onto the current keyframe.
        let mut h = blank();
        drop_file(&mut h, wav_path.clone());
        h.run();
        let ed = h.state();
        assert!(ed.doc.sounds.contains_key("tone.wav") && ed.home.doc_open, "imported, not opened");
        assert_eq!(ed.doc.scenes[0].layers[0].frames[0].sound().map(|s| s.name), Some("tone.wav".into()));
        // Again, it gets a name and a file of its own.
        drop_file(&mut h, wav_path);
        h.run();
        let again = &h.state().doc.sounds["tone 2.wav"];
        assert_eq!(again.extra.get("href"), Some("tone 2.wav"));

        // Import to Library leaves the keyframes be.
        let ed = h.state_mut();
        assert_eq!(import(ed, &mp3_path, false).as_deref(), Some("beep.mp3"));
        assert_eq!(ed.doc.scenes[0].layers[0].frames[0].sound().map(|s| s.name), Some("tone 2.wav".into()));
        assert_eq!(ed.doc.sounds["beep.mp3"].info().map(|i| i.codec), Some(sound::Codec::Mp3));
        assert!(import(ed, &junk, false).is_none() && ed.error.is_some(), "a file that isn't audio is turned down");
        assert_eq!(ed.doc.sounds.len(), 3);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_sound_dragged_from_the_library_lands_on_a_frame() {
        let mut h = blank();
        h.state_mut().doc.sounds.insert("tone.wav".into(), tone(0.5));
        h.state_mut().right_tab = 1;
        h.run();
        let row = h.get_by_label("tone.wav").rect();
        let target = cell(&h, 0, 3);
        screen_drag(&mut h, row.center(), target, Modifiers::NONE);
        h.run();
        let layer = &h.state().doc.scenes[0].layers[0];
        assert_eq!(layer.frames.iter().map(|k| k.index).collect::<Vec<_>>(), [0, 3], "a keyframe made for it past the layer's end");
        assert_eq!(layer.frames[1].sound(), Some(FrameSound::new("tone.wav")));
        assert_eq!(layer.frames[0].sound(), None);

        // Dropped on the stage, it goes on the current layer's keyframe.
        h.state_mut().frame = 1;
        h.run();
        let stage = h.state().stage_rect.center();
        screen_drag(&mut h, row.center(), stage, Modifiers::NONE);
        h.run();
        assert_eq!(h.state().doc.scenes[0].layers[0].frames[0].sound(), Some(FrameSound::new("tone.wav")));
    }

    /// Two layers over 25 frames at 24 fps: "a" starts on frame 0 and stops on frame 10;
    /// "b" streams over frames 5 to 24.
    fn playing_doc() -> Document {
        let mut doc = Document::new(550.0, 400.0, 24.0);
        doc.sounds.insert("a".into(), tone(2.0));
        doc.sounds.insert("b".into(), tone(2.0));
        let tl = &mut doc.scenes[0];
        tl.layers[0].frames[0].duration = 5;
        tl.layers[0].insert_blank_keyframe(5);
        tl.layers[0].frames[1].duration = 20;
        tl.layers[0].frames[1].set_sound(Some(&FrameSound { sync: Sync::Stream, ..FrameSound::new("b") }));
        tl.insert_layer(0, "Sound");
        let layer = &mut tl.layers[0];
        layer.insert_blank_keyframe(10);
        layer.frames[0].set_sound(Some(&FrameSound::new("a")));
        layer.frames[1].set_sound(Some(&FrameSound { sync: Sync::Stop, ..FrameSound::new("a") }));
        doc
    }

    #[test]
    fn sounds_play_with_the_playhead() {
        let mut h = blank();
        h.state_mut().load(playing_doc(), "sounds".into(), None);
        h.run();
        let ed = h.state_mut();
        let playing = |ed: &Editor| ed.sound.player.voices();
        let voice = |owner, name: &str, stream| (owner, name.to_string(), stream);
        ed.playing = Some((0.0, 0));
        tick(ed);
        assert_eq!(playing(ed), [voice(Owner::Editor, "a", false)]);
        ed.frame = 7;
        tick(ed);
        assert_eq!(playing(ed), [voice(Owner::Editor, "a", false), voice(Owner::Editor, "b", true)], "the stream starts on its keyframe");
        ed.frame = 12;
        tick(ed);
        assert_eq!(playing(ed), [voice(Owner::Editor, "b", true)], "frame 10 stops a");
        ed.playing = None;
        tick(ed);
        assert!(playing(ed).is_empty(), "stopping silences them");
        // Scrubbed, the stream plays a frame's worth.
        ed.frame = 8;
        tick(ed);
        assert_eq!(playing(ed), [voice(Owner::Editor, "b", true)]);
        // Playing from inside the stream joins it.
        ed.sound.player.stop(|_| true);
        ed.playing = Some((0.0, 8));
        ed.frame = 9;
        tick(ed);
        assert_eq!(playing(ed), [voice(Owner::Editor, "b", true)]);

        // Test Movie plays them by itself, and closing it stops them.
        let doc = ed.doc.clone();
        ed.sound.player.stop(|_| true);
        movie(&mut ed.sound, &doc, Some(0));
        movie(&mut ed.sound, &doc, Some(6));
        assert_eq!(playing(ed), [voice(Owner::Movie, "a", false), voice(Owner::Movie, "b", true)]);
        movie(&mut ed.sound, &doc, Some(1));
        assert_eq!(playing(ed), [voice(Owner::Movie, "a", false)], "round again, the stream starts over");
        movie(&mut ed.sound, &doc, None);
        assert!(playing(ed).is_empty());
    }

    #[test]
    fn library_renames_and_deletes_sounds_with_their_keyframes() {
        let mut doc = playing_doc();
        rename(&mut doc, &|n| (n == "a").then(|| "Sounds/a".to_string()));
        assert!(doc.sounds.contains_key("Sounds/a") && !doc.sounds.contains_key("a"));
        assert_eq!(doc.scenes[0].layers[0].frames[1].sound().map(|s| (s.name, s.sync)), Some(("Sounds/a".into(), Sync::Stop)));
        delete(&mut doc, &["Sounds/a".into()]);
        assert!(doc.scenes[0].layers[0].frames.iter().all(|k| k.sound().is_none()));
        assert!(doc.scenes[0].layers[1].frames[1].sound().is_some(), "other sounds stay");
    }

    /// Writes target/screenshots/sound_timeline.png (a stream's waveform across its span and
    /// event sounds on their keyframes, Properties > Frame > Sound beside) and
    /// sound_library.png (the Library's preview of a sound).
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn sound_timeline_screenshot() {
        let mut doc = Document::new(550.0, 400.0, 24.0);
        doc.sounds.insert("Music.wav".into(), Sound::from_file(drums(2.5, 4.0), "Music.wav").unwrap());
        doc.sounds.insert("Click.wav".into(), Sound::from_file(drums(0.3, 1.0), "Click.wav").unwrap());
        let tl = &mut doc.scenes[0];
        tl.layers[0].name = "Music".into();
        tl.layers[0].frames[0].duration = 60;
        tl.layers[0].frames[0].set_sound(Some(&FrameSound { sync: Sync::Stream, ..FrameSound::new("Music.wav") }));
        tl.insert_layer(0, "Clicks");
        for f in [12, 24, 36] {
            tl.layers[0].insert_blank_keyframe(f);
        }
        for k in 0..4 {
            tl.layers[0].frames[k].set_sound(Some(&FrameSound::new("Click.wav")));
        }
        let builder = egui_kittest::Harness::builder().with_size(eframe::egui::vec2(1600.0, 900.0)).with_step_dt(1.0 / 60.0).wgpu();
        let mut h = builder.build_eframe(|cc| Editor::new(&cc.egui_ctx, None));
        h.state_mut().load(doc, "sound.fla".into(), None);
        h.run();
        click_cell(&mut h, (1, 0), Modifiers::NONE);
        h.run();
        screenshot(&mut h, "sound_timeline");
        h.state_mut().right_tab = 1;
        h.state_mut().library_selection = Some("Music.wav".into());
        h.run();
        screenshot(&mut h, "sound_library");
    }
}
