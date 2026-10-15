//! Frame scripts: saved and loaded back, compiled into the SWF, published for HTML5
//! Canvas, and played (with movie clips at their own frames) by Test Movie's player.

use std::fs;
use std::path::{Path, PathBuf};

use minnamate_core::doc::{Document, Timeline};
use minnamate_core::player::Player;
use minnamate_core::render::{render, render_playing};
use minnamate_core::{html5, swf_export, xfl, xfl_write};
use swf::avm1::read::Reader;
use swf::avm1::types::Action;
use swf::Tag;

/// The tween fixture (11 frames; a two-frame movie clip "Blink", green then blank) with an
/// Actions layer on top: `stop()` on frame 5 and a label "end" on frame 8, and a script on the
/// clip's second frame.
fn doc() -> Document {
    let mut doc = xfl::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tween")).unwrap();
    let mut actions = Timeline::new("").layers.remove(0);
    actions.name = "Actions".into();
    actions.insert_frames(0, 10);
    for (frame, script) in [(4, "stop();"), (7, "")] {
        actions.insert_blank_keyframe(frame);
        let k = actions.keyframe_at(frame).unwrap();
        actions.frames[k].set_script(script);
    }
    actions.frames[2].set_label("end", "name");
    doc.scenes[0].layers.insert(0, actions);
    doc.library.get_mut("Blink").unwrap().timeline.layers[0].frames[1].set_script("gotoAndPlay(1);");
    doc
}

/// An empty folder for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn actions(bytes: &[u8]) -> Vec<Action<'_>> {
    let mut r = Reader::new(bytes, 11);
    let mut out = Vec::new();
    while let Ok(action) = r.read_action() {
        if action == Action::End {
            break;
        }
        out.push(action);
    }
    out
}

/// Each frame's DoAction bodies and labels in `tags`, by frame.
fn frames<'a>(tags: &'a [Tag<'a>]) -> Vec<(u32, &'a [u8])> {
    let mut frame = 0;
    let mut out = Vec::new();
    for tag in tags {
        match tag {
            Tag::DoAction(bytes) => out.push((frame, *bytes)),
            Tag::ShowFrame => frame += 1,
            _ => {}
        }
    }
    out
}

#[test]
fn scripts_save_and_load_back() {
    let mut doc = doc();
    doc.set_canvas(true);
    let dir = scratch("scripts_save");
    xfl_write::save(&doc, &dir.join("movie.fla")).unwrap();
    let back = xfl::load(&dir.join("movie.fla")).unwrap();
    assert_eq!(back, doc);
    assert_eq!(back.scenes[0].layers[0].frames[1].script().as_deref(), Some("stop();"));
    assert!(back.is_canvas());
}

#[test]
fn the_swf_runs_scripts_as_avm1() {
    let (bytes, skipped) = swf_export::export(&doc()).unwrap();
    assert!(skipped.is_empty(), "{skipped:?}");
    let buf = swf::decompress_swf(&bytes[..]).unwrap();
    let movie = swf::parse_swf(&buf).unwrap();
    // FileAttributes comes first, without ActionScript 3: Ruffle plays it as AVM1.
    assert!(matches!(movie.tags[0], Tag::FileAttributes(a) if !a.contains(swf::FileAttributes::IS_ACTION_SCRIPT_3)));
    assert!(!movie.header.is_action_script_3());

    // stop() on frame 5 (index 4), and nothing for the empty script on frame 8.
    let main = frames(&movie.tags);
    assert_eq!(main.len(), 1);
    assert_eq!((main[0].0, actions(main[0].1)), (4, vec![Action::Stop]));
    let label = movie.tags.iter().find_map(|t| if let Tag::FrameLabel(l) = t { Some(l) } else { None }).unwrap();
    assert_eq!((label.label.as_bytes(), label.is_anchor), (&b"end"[..], false));
    let mut frame = 0;
    for tag in &movie.tags {
        match tag {
            Tag::FrameLabel(_) => assert_eq!(frame, 7, "the label sits on frame 8"),
            Tag::ShowFrame => frame += 1,
            _ => {}
        }
    }

    // The clip's own script, in its sprite: back to its frame 1, playing.
    let sprite = movie.tags.iter().find_map(|t| if let Tag::DefineSprite(s) = t { Some(s) } else { None }).unwrap();
    let clip = frames(&sprite.tags);
    assert_eq!(clip.len(), 1);
    let goto = actions(clip[0].1);
    assert_eq!(clip[0].0, 1);
    assert!(matches!(goto[..], [Action::GotoFrame(g), Action::Play] if g.frame == 0), "{goto:?}");
}

#[test]
fn html5_canvas_publishing() {
    // An ActionScript document's scripts translate: frames count from 0 in JavaScript.
    let mut doc = doc();
    let dir = scratch("html5_canvas");
    assert!(html5::publish(&doc, &dir, "movie").unwrap().is_empty());
    let page = fs::read_to_string(dir.join("movie.html")).unwrap();
    for part in [r#"<canvas id="movie" width="550" height="400">"#, r#"<script src="movie.js"></script>"#, r#"<script src="runtime.js"></script>"#] {
        assert!(page.contains(part), "{part} in\n{page}");
    }
    let movie = fs::read_to_string(dir.join("movie.js")).unwrap();
    for frame in 1..=11 {
        assert!(movie.contains(&format!("// frame {frame}\n")), "frame {frame}");
    }
    assert!(movie.contains("4: function () {\n          this.stop();\n        },"), "{movie}");
    assert!(movie.contains("labels: [[\"end\",7]]") && movie.contains("scenes: [[\"Scene 1\",0,11]]"));
    assert!(movie.contains("\"Blink\": {") && movie.contains("this.gotoAndPlay(0);"), "the clip's timeline and script");
    assert!(fs::read_to_string(dir.join("runtime.js")).unwrap().contains("window.exportRoot = root;"));

    // An HTML5 Canvas document's are JavaScript: a bare stop() would be the window's, so it's reported.
    doc.set_canvas(true);
    doc.scenes[0].layers[0].frames[1].set_script("this.stop();\nstop();");
    doc.library.get_mut("Blink").unwrap().timeline.layers[0].frames[1].set_script("this.gotoAndPlay(0);");
    let skipped = html5::publish(&doc, &dir, "movie").unwrap();
    assert_eq!(skipped.into_iter().collect::<Vec<_>>(), ["script `stop();` (Scene 1, Actions, frame 5, line 2)"]);
    let movie = fs::read_to_string(dir.join("movie.js")).unwrap();
    assert!(movie.contains("          this.stop();\n          // Not supported: stop();\n"), "{movie}");
}

#[test]
fn html5_canvas_publishes_bitmaps() {
    use minnamate_core::doc::{BitmapFill, BitmapInstance, BitmapItem, Element, Extra, Matrix, Media, Paint};
    let mut doc = Document::new(100.0, 100.0, 24.0);
    let mut pm = minnamate_core::tiny_skia::Pixmap::new(4, 2).unwrap();
    pm.fill(minnamate_core::tiny_skia::Color::from_rgba8(255, 0, 0, 255));
    doc.media.push(Media::Bitmap(BitmapItem::new("red.png", pm.encode_png().unwrap())));
    let fill = BitmapFill { name: "red.png".into(), matrix: Matrix::IDENTITY, clipped: false, extra: Extra::default() };
    let tiled = minnamate_core::draw::rectangle([(0.0, 0.0), (50.0, 50.0)], 0.0, None, Some(Paint::Bitmap(fill)));
    let placed = BitmapInstance { name: "red.png".into(), matrix: Matrix { tx: 60.0, ..Matrix::IDENTITY }, extra: Extra::default() };
    doc.scenes[0].layers[0].frames[0].elements.extend([Element::Shape(tiled), Element::Bitmap(placed)]);

    let dir = scratch("html5_bitmaps");
    assert!(html5::publish(&doc, &dir, "movie").unwrap().is_empty());
    assert_eq!(fs::read(dir.join("images/0.png")).unwrap(), pm.encode_png().unwrap(), "the file as imported, once");
    let movie = fs::read_to_string(dir.join("movie.js")).unwrap();
    assert!(movie.contains("images: [\n    {src:\"images/0.png\",w:4,h:2},\n  ]"), "{movie}");
    assert!(movie.contains("b:{i:0,m:[1,0,0,1,0,0],r:1}") && movie.contains("{i:0,m:[1,0,0,1,60,0]}"), "{movie}");
    if let Some(printed) = play_page(&dir, 2) {
        assert_eq!(printed, ["0 true"]);
    }
}

#[test]
fn the_swf_knows_the_main_timelines_scenes() {
    use minnamate_core::doc::{ColorTransform, Element, Extra, Instance, Loop, Matrix, Symbol, SymbolKind};
    // Scene 1 is frames 0..5 and Scene 2 frames 5..8 of the SWF's one timeline.
    let mut doc = Document::new(100.0, 100.0, 24.0);
    doc.scenes[0].layers[0].insert_frames(0, 4);
    doc.insert_scene(0);
    doc.scenes[1].layers[0].insert_frames(0, 2);
    // On the main timeline the root is the script's own: its frame 1 is Scene 2's first.
    doc.scenes[1].layers[0].frames[0].set_script("MovieClip(root).gotoAndStop(1);");
    // From a clip, a root goto naming a scene is a frame; the root's next scene isn't known there.
    let mut clip = Timeline::new("C");
    clip.layers[0].frames[0].set_script("MovieClip(root).gotoAndPlay(2, \"Scene 2\");\nMovieClip(root).nextScene();");
    doc.library.insert("C".into(), Symbol { kind: SymbolKind::MovieClip, timeline: clip, extra: Extra::default() });
    let c = Instance { symbol: "C".into(), kind: SymbolKind::MovieClip, matrix: Matrix::IDENTITY, pivot: (0.0, 0.0), color: ColorTransform::IDENTITY, looping: Loop::Loop, first_frame: 0, extra: Extra::default() };
    doc.scenes[0].layers[0].frames[0].elements.push(Element::Instance(c));

    let (bytes, skipped) = swf_export::export(&doc).unwrap();
    assert_eq!(skipped.into_iter().collect::<Vec<_>>(), ["nextScene() of another timeline's scenes (C, Layer_1, frame 1)"]);
    let buf = swf::decompress_swf(&bytes[..]).unwrap();
    let movie = swf::parse_swf(&buf).unwrap();
    let main = frames(&movie.tags);
    assert_eq!(main.len(), 1);
    assert!(matches!(actions(main[0].1)[..], [Action::GotoFrame(g), Action::Stop] if g.frame == 5) && main[0].0 == 5, "{:?}", actions(main[0].1));
    let sprite = movie.tags.iter().find_map(|t| if let Tag::DefineSprite(s) = t { Some(s) } else { None }).unwrap();
    let told = actions(frames(&sprite.tags)[0].1);
    assert!(matches!(told[..], [Action::SetTarget(_), Action::GotoFrame(g), Action::Play, Action::SetTarget(_)] if g.frame == 6), "{told:?}");
}

/// Plays the page published in `dir` for `frames` frames under node, on a stub canvas: what it
/// printed, then the main timeline's frame and whether it plays. None where node isn't installed.
fn play_page(dir: &Path, frames: u32) -> Option<Vec<String>> {
    const HARNESS: &str = r#"const fs = require("fs"), path = require("path"), vm = require("vm");
const [dir, frames] = process.argv.slice(2);
const noop = () => {};
function canvas() {
  const c = { width: 0, height: 0, style: {} };
  const g = { canvas: c, getTransform: () => ({}), createLinearGradient: () => ({ addColorStop: noop }), createRadialGradient: () => ({ addColorStop: noop }) };
  for (const m of ["save", "restore", "transform", "setTransform", "fillRect", "fill", "stroke", "clip", "drawImage"]) g[m] = noop;
  g.createPattern = () => ({ setTransform: noop });
  c.getContext = () => g;
  return c;
}
const frame = [];
const page = { console, Path2D: function () {}, Image: function () { this.complete = true; this.naturalWidth = 1; }, DOMMatrix: function () {},
  performance: { now: () => 0 }, requestAnimationFrame: f => frame.push(f),
  document: { getElementById: canvas, createElement: canvas }, devicePixelRatio: 1 };
page.window = page;
vm.createContext(page);
for (const file of ["movie.js", "runtime.js"]) vm.runInContext(fs.readFileSync(path.join(dir, file), "utf8"), page);
for (let i = 1; i <= Number(frames); i++) frame.shift()(i * 1000 / page.movie.fps + 0.5);
console.log(page.exportRoot.currentFrame, page.exportRoot.playing);
"#;
    fs::write(dir.join("harness.js"), HARNESS).unwrap();
    let out = std::process::Command::new("node").arg(dir.join("harness.js")).arg(dir).arg(frames.to_string()).output().ok()?;
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    Some(String::from_utf8(out.stdout).unwrap().lines().map(String::from).collect())
}

#[test]
fn the_published_page_plays_as_test_movie_does() {
    use minnamate_core::doc::{ColorTransform, Element, Extra, Instance, Loop, Matrix, Symbol, SymbolKind};
    let dir = scratch("html5_page");
    html5::publish(&doc(), &dir, "movie").unwrap();
    let Some(printed) = play_page(&dir, 20) else { return eprintln!("node isn't installed, so the page isn't played") };
    assert_eq!(printed, ["4 false"], "stopped on frame 5");

    // A clip its parent's next frame takes off runs no script there: the main timeline plays on.
    let mut gone = Document::new(100.0, 100.0, 24.0);
    let mut clip = Timeline::new("A");
    clip.layers[0].insert_blank_keyframe(1);
    clip.layers[0].frames[1].set_script("trace(\"gone\"); MovieClip(parent).stop();");
    gone.library.insert("A".into(), Symbol { kind: SymbolKind::MovieClip, timeline: clip, extra: Extra::default() });
    let layer = &mut gone.scenes[0].layers[0];
    layer.frames[0].elements.push(Element::Instance(Instance {
        symbol: "A".into(),
        kind: SymbolKind::MovieClip,
        matrix: Matrix::IDENTITY,
        pivot: (0.0, 0.0),
        color: ColorTransform::IDENTITY,
        looping: Loop::Loop,
        first_frame: 0,
        extra: Extra::default(),
    }));
    layer.insert_blank_keyframe(1);
    html5::publish(&gone, &dir, "movie").unwrap();
    assert_eq!(play_page(&dir, 9).unwrap(), ["1 true"]);
    let mut player = Player::new(&gone);
    (0..9).for_each(|_| player.step(&gone));
    assert_eq!((player.root.frame, player.root.playing, player.output.len()), (1, true, 0), "{:?}", player.output);
}

#[test]
fn hidden_clips_play_on_undrawn() {
    use minnamate_core::doc::{ColorTransform, Element, Extra, Instance, Loop, Matrix, Symbol, SymbolKind};
    let instance = |symbol: &str, kind, (tx, ty), hidden: bool| {
        let mut extra = Extra::default();
        extra.set("isVisible", hidden.then_some("false"));
        let matrix = Matrix { tx, ty, ..Matrix::IDENTITY };
        Element::Instance(Instance { symbol: symbol.into(), kind, matrix, pivot: (0.0, 0.0), color: ColorTransform::IDENTITY, looping: Loop::Loop, first_frame: 0, extra })
    };
    // In place of the fixture's Blink (green, then blank), a graphic holding a hidden clip that
    // stops the movie, then Blink: Visible unchecked, a clip plays on, as the SWF's does.
    let mut doc = xfl::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tween")).unwrap();
    let mut halt = Timeline::new("Halt");
    halt.layers[0].frames[0].set_script("MovieClip(parent).stop();");
    doc.library.insert("Halt".into(), Symbol { kind: SymbolKind::MovieClip, timeline: halt, extra: Extra::default() });
    let mut g = Timeline::new("G");
    g.layers[0].frames[0].elements = vec![instance("Halt", SymbolKind::MovieClip, (0.0, 0.0), true), instance("Blink", SymbolKind::MovieClip, (0.0, 0.0), false)];
    doc.library.insert("G".into(), Symbol { kind: SymbolKind::Graphic, timeline: g, extra: Extra::default() });
    doc.scenes[0].layers[0].frames[0].elements = vec![instance("G", SymbolKind::Graphic, (520.0, 30.0), false)];

    let mut player = Player::new(&doc);
    player.step(&doc);
    assert_eq!((player.root.frame, player.root.playing), (0, false), "the hidden clip's script ran");
    // Blink, on its own second frame, is drawn so: blank.
    let (pixmap, _) = render_playing(&doc, &player, 1.0, None).unwrap();
    let c = pixmap.pixel(520, 30).unwrap().demultiply();
    assert_eq!([c.red(), c.green(), c.blue()], [255, 255, 255]);

    let dir = scratch("html5_hidden");
    html5::publish(&doc, &dir, "movie").unwrap();
    assert!(fs::read_to_string(dir.join("movie.js")).unwrap().contains(",h:1,"));
    if let Some(printed) = play_page(&dir, 3) {
        assert_eq!(printed, ["0 false"]);
    }
}

#[test]
fn test_movie_plays_clips_on_their_own_frames() {
    let fixture = xfl::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tween")).unwrap();
    let mut player = Player::new(&fixture);
    player.step(&fixture);
    // Frame 2: the clip is on its own second frame, blank; drawn alone, a clip shows its first.
    let (playing, _) = render_playing(&fixture, &player, 1.0, None).unwrap();
    let (still, _) = render(&fixture, 1, 1.0).unwrap();
    let at = |p: &minnamate_core::tiny_skia::Pixmap| {
        let c = p.pixel(520, 30).unwrap().demultiply();
        [c.red(), c.green(), c.blue()]
    };
    assert_eq!((at(&playing), at(&still)), ([255, 255, 255], [0, 255, 0]));

    // With the scripts: the clip's own sends it back to its first frame at once, and the
    // movie stops on frame 5.
    let doc = doc();
    let mut player = Player::new(&doc);
    player.step(&doc);
    assert_eq!(player.root.children.iter().map(|c| c.frame).collect::<Vec<_>>(), [0]);
    for _ in 0..10 {
        player.step(&doc);
    }
    assert_eq!((player.root.frame, player.root.playing), (4, false));
}

#[test]
fn clips_keep_their_instance_names_in_the_swf() {
    use minnamate_core::doc::{Element, LayerKind};
    // The clip, named, under a mask (the background's rectangle), which takes the depth before it.
    let mut doc = doc();
    let tl = &mut doc.scenes[0];
    let Element::Instance(blink) = &mut tl.layers[1].frames[0].elements[0] else { panic!("the Blink layer holds the clip") };
    blink.extra.set("name", Some("blink"));
    let mut mask = Timeline::new("").layers.remove(0);
    (mask.name, mask.kind) = ("Mask".into(), LayerKind::Mask);
    mask.insert_frames(0, 10);
    mask.frames[0].elements.push(tl.layers[3].frames[0].elements[0].clone());
    tl.layers.insert(1, mask);
    tl.layers[2].extra.set("parentLayerIndex", Some("1"));
    let (bytes, _) = swf_export::export(&doc).unwrap();
    let buf = swf::decompress_swf(&bytes[..]).unwrap();
    let movie = swf::parse_swf(&buf).unwrap();
    // Background 1, ball 2, the mask 3 (clipping to 4), the clip 4.
    let placed: Vec<_> = movie.tags.iter().take_while(|t| !matches!(t, Tag::ShowFrame)).filter_map(|t| if let Tag::PlaceObject(p) = t { Some(p) } else { None }).collect();
    let named: Vec<_> = placed.iter().map(|p| (p.depth, p.clip_depth, p.name.map(|n| n.as_bytes().to_vec()))).collect();
    assert_eq!(named, [(1, None, None), (2, None, None), (3, Some(4), None), (4, None, Some(b"blink".to_vec()))]);
}
