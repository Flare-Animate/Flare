//! Test Movie's player: the movie's main timeline and every movie clip on stage play
//! their own timelines, running the frame scripts script.rs reads as they enter frames.
//! render.rs draws each clip at the frame it has reached (`render::render_playing`).

use std::collections::{HashMap, HashSet};

use crate::display::{Item, Placed, display_list};
use crate::doc::{Document, Element, Symbol, SymbolKind, Timeline};
use crate::hit::ElementRef;
use crate::script::{Command, Dialect, Step, Stmt, Track, Value};

/// A timeline playing: the movie's, or a movie clip instance's.
#[derive(Clone, Debug)]
pub struct Clip {
    id: u64,
    /// Its library symbol; None for the movie's main timeline.
    pub symbol: Option<String>,
    /// The frame it shows, from 0; the main timeline's counts across its scenes.
    pub frame: u32,
    pub playing: bool,
    /// A button shows its first frame, and doesn't play.
    pub button: bool,
    /// The instance name scripts reach it by.
    pub name: Option<String>,
    /// Where its parent shows it (`Keys`).
    key: (usize, usize, usize),
    /// It entered a frame, whose scripts are due.
    entered: bool,
    pub children: Vec<Clip>,
}

pub struct Player {
    pub root: Clip,
    /// What trace printed, and notes on what scripts couldn't do, for the Output panel.
    pub output: Vec<String>,
    /// navigateToURL and getURL calls: (url, window).
    pub urls: Vec<(String, String)>,
    next_id: u64,
    /// Notes already made, made once.
    noted: HashSet<String>,
}

impl Player {
    /// The movie on its first frame, that frame's scripts run.
    pub fn new(doc: &Document) -> Player {
        let root = Clip { id: 0, symbol: None, frame: 0, playing: true, button: false, name: None, key: (0, 0, 0), entered: true, children: Vec::new() };
        let mut p = Player { root, output: Vec::new(), urls: Vec::new(), next_id: 0, noted: HashSet::new() };
        sync(doc, &mut p.root, &mut p.next_id);
        p.run(doc);
        p
    }

    /// One frame on: each playing timeline moves on a frame (back to its first after its
    /// last), clips come and go with what their parents show, and the scripts run of every
    /// frame entered, parents' before children's.
    pub fn step(&mut self, doc: &Document) {
        advance(doc, &mut self.root);
        sync(doc, &mut self.root, &mut self.next_id);
        self.run(doc);
    }

    fn run(&mut self, doc: &Document) {
        let dialect = Dialect::of(doc);
        // A goto that lands on a frame runs its scripts too; a goto loop stops here.
        for _ in 0..1000 {
            let Some(path) = take_entered(&mut self.root, &mut Vec::new()) else { return };
            let Some(clip) = self.clip(&path) else { continue };
            let (id, frame) = (clip.id, clip.frame);
            let Some(track) = track(doc, clip) else { continue };
            for (layer, script) in track.scripts(frame, dialect) {
                for stmt in &script.stmts {
                    match stmt {
                        Stmt::Skip { line, source } => self.note(format!("Not supported, skipped: {source} ({})", track.place(frame, layer, *line))),
                        // The clip may be gone: a goto on its parent can take it off.
                        Stmt::Run { path, command } => match find(&self.root, id, &mut Vec::new()) {
                            Some(from) => self.exec(doc, &from, path, command, dialect),
                            None => break,
                        },
                    }
                }
            }
        }
        self.note("Frame scripts went to over 1000 frames in one frame; the rest wait for the next.".into());
    }

    fn exec(&mut self, doc: &Document, from: &[usize], to: &[Step], command: &Command, dialect: Dialect) {
        match command {
            Command::Trace(values) => {
                let line: Vec<_> = values.iter().map(|v| self.eval(doc, from, v, dialect).to_string()).collect();
                return self.output.push(line.join(" "));
            }
            Command::OpenUrl { url, window } => return self.urls.push((url.clone(), window.clone())),
            _ => {}
        }
        let Some(target) = self.resolve(from, to) else {
            return self.note(format!("{}: there's no {} to {} on.", self.describe(from), named(to), command.name()));
        };
        let Some(clip) = at_mut(&mut self.root, &target).filter(|c| !c.button) else { return };
        if let Command::Stop | Command::Play = command {
            clip.playing = *command == Command::Play;
            return;
        }
        if let Some((frame, play)) = track(doc, clip).and_then(|t| t.go(clip.frame, command)) {
            clip.playing = play;
            if clip.frame != frame {
                (clip.frame, clip.entered) = (frame, true);
                sync(doc, clip, &mut self.next_id);
            }
        }
    }

    fn eval(&self, doc: &Document, from: &[usize], v: &Value, dialect: Dialect) -> Shown {
        match v {
            Value::Text(s) => Shown::Text(s.clone()),
            Value::Number(n) => Shown::Number(*n),
            Value::CurrentFrame(path) | Value::TotalFrames(path) => {
                let clip = self.resolve(from, path).and_then(|p| self.clip(&p));
                let Some((clip, track)) = clip.and_then(|c| Some((c, track(doc, c)?))) else { return Shown::Text("undefined".into()) };
                // A movie's currentFrame counts within its scene, as in ActionScript 3.0.
                let n = match v {
                    Value::CurrentFrame(_) => track.at(clip.frame).1 + dialect.first(),
                    _ => track.len(),
                };
                Shown::Number(n as f64)
            }
            Value::Sum(parts) => parts.iter().map(|p| self.eval(doc, from, p, dialect)).reduce(|a, b| match (a, b) {
                (Shown::Number(a), Shown::Number(b)) => Shown::Number(a + b),
                (a, b) => Shown::Text(format!("{a}{b}")),
            }).unwrap_or(Shown::Text(String::new())),
        }
    }

    /// The clip `steps` lead to from the one at `from`.
    fn resolve(&self, from: &[usize], steps: &[Step]) -> Option<Vec<usize>> {
        let mut path = from.to_vec();
        for step in steps {
            match step {
                Step::Parent => {
                    path.pop()?;
                }
                Step::Root => path.clear(),
                Step::Child(name) => {
                    let i = self.clip(&path)?.children.iter().position(|c| c.name.as_deref() == Some(name.as_str()))?;
                    path.push(i);
                }
            }
        }
        Some(path)
    }

    /// The clip at `path` of child indices.
    pub fn clip(&self, path: &[usize]) -> Option<&Clip> {
        path.iter().try_fold(&self.root, |c, &i| c.children.get(i))
    }

    /// The clip at `path` as scripts would name it from the root.
    fn describe(&self, path: &[usize]) -> String {
        let names: Vec<_> = (1..=path.len()).filter_map(|n| self.clip(&path[..n])).map(|c| c.name.clone().unwrap_or_else(|| "(unnamed)".into())).collect();
        if names.is_empty() { "root".into() } else { names.join(".") }
    }

    fn note(&mut self, note: String) {
        if self.noted.insert(note.clone()) {
            self.output.push(note);
        }
    }
}

/// Steps as a script writes them: "parent.ball".
fn named(steps: &[Step]) -> String {
    let words: Vec<&str> = steps.iter().map(|s| match s {
        Step::Parent => "parent",
        Step::Root => "root",
        Step::Child(name) => name.as_str(),
    }).collect();
    words.join(".")
}

/// A value trace prints, as JavaScript would print it.
enum Shown {
    Text(String),
    Number(f64),
}

impl std::fmt::Display for Shown {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Shown::Text(s) => f.write_str(s),
            Shown::Number(n) if n.is_infinite() => f.write_str(if *n > 0.0 { "Infinity" } else { "-Infinity" }),
            // No "-0".
            Shown::Number(n) => write!(f, "{}", if *n == 0.0 { 0.0 } else { *n }),
        }
    }
}

/// The timeline `clip` plays, as scripts count its frames.
fn track<'a>(doc: &'a Document, clip: &Clip) -> Option<Track<'a>> {
    match &clip.symbol {
        None => Some(Track::movie(doc)),
        Some(name) => Some(Track::symbol(&doc.library.get(name)?.timeline)),
    }
}

fn advance(doc: &Document, clip: &mut Clip) {
    if clip.playing && !clip.button {
        let len = track(doc, clip).map_or(1, |t| t.len());
        if len > 1 {
            (clip.frame, clip.entered) = ((clip.frame + 1) % len, true);
        }
    }
    clip.children.iter_mut().for_each(|c| advance(doc, c));
}

/// Brings `clip`'s children, and theirs, in line with what each shows now: a clip still shown
/// plays on, a new one starts on its first frame, its scripts due.
fn sync(doc: &Document, clip: &mut Clip, next_id: &mut u64) {
    let shown = shown(doc, clip);
    let mut old = std::mem::take(&mut clip.children);
    for (key, symbol, button, name) in shown {
        let child = match old.iter().position(|c| c.key == key && c.symbol.as_deref() == Some(symbol.as_str())) {
            Some(i) => Clip { name, ..old.swap_remove(i) },
            None => {
                *next_id += 1;
                Clip { id: *next_id, symbol: Some(symbol), frame: 0, playing: !button, button, name, key, entered: !button, children: Vec::new() }
            }
        };
        clip.children.push(child);
    }
    clip.children.iter_mut().for_each(|c| sync(doc, c, next_id));
}

/// The movie clips and buttons `clip` shows at its frame, as (key, symbol, button, instance name).
fn shown(doc: &Document, clip: &Clip) -> Vec<((usize, usize, usize), String, bool, Option<String>)> {
    let Some(track) = track(doc, clip) else { return Vec::new() };
    let (s, frame) = track.at(clip.frame);
    let tl = track.scenes[s].timeline;
    let list = display_list(doc, tl, frame, &mut Default::default());
    let mut keys = Keys::default();
    let mut out = Vec::new();
    clips(&list, &mut |source, symbol, kind| {
        let key = keys.next(source);
        if let Some(symbol) = symbol_name(doc, symbol) {
            out.push((key, symbol.to_string(), kind == SymbolKind::Button, instance_name(tl, source)));
        }
    });
    out
}

/// The library name of `symbol`.
pub(crate) fn symbol_name<'a>(doc: &'a Document, symbol: &Symbol) -> Option<&'a str> {
    doc.library.iter().find(|(_, s)| std::ptr::eq(*s, symbol)).map(|(name, _)| name.as_str())
}

/// The instance name of the clip `source` places on `tl` itself; one inside a graphic or group has none here.
pub(crate) fn instance_name(tl: &Timeline, source: ElementRef) -> Option<String> {
    match tl.element(source) {
        Some(Element::Instance(i)) if i.kind != SymbolKind::Graphic => i.extra.get("name").map(String::from),
        _ => None,
    }
}

/// Each movie clip and button of `list` in the order render.rs draws them, masks before what
/// they mask; one hidden (Visible unchecked) isn't drawn, but plays as the SWF's does.
pub(crate) fn clips<'a>(list: &[Placed<'a>], each: &mut dyn FnMut(ElementRef, &'a Symbol, SymbolKind)) {
    for p in list {
        match &p.item {
            Item::Clip(symbol, kind) => each(p.source, symbol, *kind),
            Item::Masked { mask, content } => {
                clips(mask, each);
                clips(content, each);
            }
            _ => {}
        }
    }
}

/// A clip's key within its parent's display: the element of the parent's timeline that shows
/// it, and which of that element's clips it is (a graphic can hold several).
#[derive(Default)]
pub(crate) struct Keys(HashMap<(usize, usize), usize>);

impl Keys {
    pub(crate) fn next(&mut self, source: ElementRef) -> (usize, usize, usize) {
        let n = self.0.entry((source.layer, source.element)).or_default();
        *n += 1;
        (source.layer, source.element, *n - 1)
    }
}

fn at_mut<'c>(root: &'c mut Clip, path: &[usize]) -> Option<&'c mut Clip> {
    path.iter().try_fold(root, |c, &i| c.children.get_mut(i))
}

/// The path to clip `id`.
fn find(clip: &Clip, id: u64, path: &mut Vec<usize>) -> Option<Vec<usize>> {
    if clip.id == id {
        return Some(path.clone());
    }
    for (i, c) in clip.children.iter().enumerate() {
        path.push(i);
        if let Some(found) = find(c, id, path) {
            return Some(found);
        }
        path.pop();
    }
    None
}

/// The first clip, parents before children, that entered a frame: marked done, its path.
fn take_entered(clip: &mut Clip, path: &mut Vec<usize>) -> Option<Vec<usize>> {
    if std::mem::take(&mut clip.entered) {
        return Some(path.clone());
    }
    for (i, c) in clip.children.iter_mut().enumerate() {
        path.push(i);
        if let Some(found) = take_entered(c, path) {
            return Some(found);
        }
        path.pop();
    }
    None
}

/// A player's clips as render.rs walks the display down: `enter` gives each movie clip the
/// frame its instance has reached, keyed as the player keyed them.
pub struct ClipFrames<'a> {
    stack: Vec<(Option<&'a Clip>, Keys)>,
}

impl<'a> ClipFrames<'a> {
    pub fn new(player: &'a Player) -> Self {
        ClipFrames { stack: vec![(Some(&player.root), Keys::default())] }
    }

    /// Into the clip `source` shows: its frame. Each `enter` has its `leave`.
    pub(crate) fn enter(&mut self, source: ElementRef) -> u32 {
        let (clip, keys) = self.stack.last_mut().expect("the movie's entry stays");
        let key = keys.next(source);
        let child = clip.and_then(|c| c.children.iter().find(|c| c.key == key));
        self.stack.push((child, Keys::default()));
        child.map_or(0, |c| c.frame)
    }

    pub(crate) fn leave(&mut self) {
        self.stack.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Instance, Loop, Matrix, Timeline, ColorTransform, Extra};

    /// A movie of `frames` frames on one layer, with `scripts` as (frame, script) keyframes.
    fn movie(frames: u32, scripts: &[(u32, &str)]) -> Document {
        let mut doc = Document::new(100.0, 100.0, 24.0);
        let layer = &mut doc.scenes[0].layers[0];
        layer.insert_frames(0, frames - 1);
        for &(f, script) in scripts {
            layer.insert_keyframe(f);
            let k = layer.keyframe_at(f).unwrap();
            layer.frames[k].set_script(script);
        }
        doc
    }

    fn instance(symbol: &str, name: &str) -> Element {
        let mut extra = Extra::default();
        extra.set("name", Some(name));
        Element::Instance(Instance {
            symbol: symbol.into(),
            kind: SymbolKind::MovieClip,
            matrix: Matrix::IDENTITY,
            pivot: (0.0, 0.0),
            color: ColorTransform::IDENTITY,
            looping: Loop::Loop,
            first_frame: 0,
            extra,
        })
    }

    #[test]
    fn stop_holds_the_frame_and_goto_jumps() {
        let doc = movie(10, &[(4, "stop();"), (7, "gotoAndPlay(2); trace('looped')")]);
        let mut p = Player::new(&doc);
        for _ in 0..20 {
            p.step(&doc);
        }
        assert_eq!((p.root.frame, p.root.playing), (4, false), "stopped on frame 5");
        // Played on, frame 8 sends it back to frame 2.
        p.root.playing = true;
        for _ in 0..3 {
            p.step(&doc);
        }
        assert_eq!((p.root.frame, p.output.as_slice()), (1, &["looped".to_string()][..]));
    }

    #[test]
    fn clips_play_their_own_timelines_and_reach_each_other() {
        // A 6-frame clip named "spin" stopping itself on its frame 3; the movie stops it at
        // frame 2 on the main timeline's frame 2, then sends it to its label.
        let mut doc = movie(4, &[(1, "spin.gotoAndStop(\"mark\"); trace(spin.currentFrame, spin.totalFrames)")]);
        let mut clip = Timeline::new("Spin");
        clip.layers[0].insert_frames(0, 5);
        clip.layers[0].insert_keyframe(2);
        clip.layers[0].frames[1].set_script("stop(); MovieClip(parent).gotoAndStop(4);");
        clip.layers[0].insert_keyframe(4);
        clip.layers[0].frames[2].set_label("mark", "name");
        doc.library.insert("Spin".into(), Symbol { kind: SymbolKind::MovieClip, timeline: clip, extra: Extra::default() });
        for f in &mut doc.scenes[0].layers[0].frames {
            f.elements.push(instance("Spin", "spin"));
        }
        let mut p = Player::new(&doc);
        assert_eq!((p.root.children.len(), p.root.children[0].frame), (1, 0));
        p.step(&doc);
        // Main frame 2: the script sends the clip to its frame 5 ("mark") and traces 5 of 6.
        assert_eq!((p.root.frame, p.root.children[0].frame, p.root.children[0].playing), (1, 4, false));
        assert_eq!(p.output, ["5 6"]);
        // Played again from its frame 1, the clip's own stop() holds it on frame 3, and its
        // parent goes to frame 4 and stops.
        p.root.children[0].frame = 0;
        p.root.children[0].playing = true;
        p.step(&doc);
        p.step(&doc);
        assert_eq!((p.root.children[0].frame, p.root.children[0].playing), (2, false));
        assert_eq!((p.root.frame, p.root.playing), (3, false));
        assert!(p.output.iter().all(|line| !line.contains("there's no")), "{:?}", p.output);
    }

    #[test]
    fn what_cant_run_is_noted_once() {
        let doc = movie(3, &[(1, "x = 1; ghost.play();")]);
        let mut p = Player::new(&doc);
        for _ in 0..6 {
            p.step(&doc);
        }
        assert_eq!(p.output, [
            "Not supported, skipped: x = 1; (Scene 1, Layer_1, frame 2, line 1)",
            "root: there's no ghost to play on.",
        ]);
    }
}
