use super::*;
use ac_core::{Document, Editor};

fn opts() -> ScriptOptions {
    ScriptOptions { timeout: Duration::from_secs(20), base_dir: std::env::temp_dir(), ..Default::default() }
}

fn have(prog: &str) -> bool {
    Command::new(prog).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

#[test]
fn js_drives_the_editor_as_one_undo_step() {
    let mut ed = Editor::new(Document::default());
    let r = run_js(&mut ed, Language::Js.example(), &opts());
    assert!(r.ok(), "{:?} {:?}", r.error, r.log);
    assert_eq!(r.log, vec!["Stars: 5"]);
    assert_eq!(ed.timeline().layers.len(), 2);
    assert_eq!(ed.undo_label(), Some("Run Script"));
    // on a busy document the example only touches its own layer
    let mut demo = Editor::new(ac_core::demo::demo_document());
    let before: Vec<_> = demo.doc.scene.layers.iter().map(|l| l.keyframes.keys().copied().collect::<Vec<_>>()).collect();
    let r = run_js(&mut demo, Language::Js.example(), &opts());
    assert_eq!(r.log, vec!["Stars: 5"]);
    let after: Vec<_> = demo.doc.scene.layers.iter().skip(1).map(|l| l.keyframes.keys().copied().collect::<Vec<_>>()).collect();
    assert_eq!(before, after, "other layers untouched");
    ed.undo();
    assert_eq!(ed.timeline().layers.len(), 1, "one undo reverts the whole script");
}

#[test]
fn js_errors_are_reported_with_location() {
    let mut ed = Editor::new(Document::default());
    let r = run_js(&mut ed, "ac.draw.rect({ x: 1 });", &opts());
    let e = r.error.unwrap();
    assert!(e.starts_with("Error: draw.rect") && e.contains("missing"), "{e}");
    let r = run_js(&mut ed, "let x = ;", &opts());
    let e = r.error.unwrap();
    assert!(e.contains("SyntaxError"), "{e}");
}

#[test]
fn js_infinite_loop_is_stopped() {
    let mut ed = Editor::new(Document::default());
    let o = ScriptOptions { timeout: Duration::from_millis(300), ..opts() };
    let r = run_js(&mut ed, "while (true) {}", &o);
    assert!(r.error.unwrap().contains("longer than"));
    // the editor is intact afterwards
    assert!(run_js(&mut ed, "ac.draw.rect({x:0,y:0,w:5,h:5})", &opts()).ok());
}

#[test]
fn js_can_export() {
    let mut ed = Editor::new(ac_core::demo::demo_document());
    let dir = std::env::temp_dir().join("ac-script-test-export");
    let _ = std::fs::create_dir_all(&dir);
    let o = ScriptOptions { base_dir: dir.clone(), ..opts() };
    let r = run_js(&mut ed, "ac.export.png({ path: 'f.png', frame: 5 }); ac.doc.save({ path: 'd.acx.json' });", &o);
    assert!(r.ok(), "{:?}", r.error);
    assert!(dir.join("f.png").exists() && dir.join("d.acx.json").exists());
}

#[test]
fn python_bridge() {
    if !have("python3") && !have("python") {
        eprintln!("python not installed; skipping");
        return;
    }
    let mut ed = Editor::new(Document::default());
    let r = run_code(&mut ed, Language::Python, Language::Python.example(), &opts());
    assert!(r.ok(), "{:?}\n{}", r.error, r.log.join("\n"));
    assert!(r.log.iter().any(|l| l == "Stars: 5"), "{:?}", r.log);
    assert_eq!(ed.timeline().layers.len(), 2);
    assert!(r.commands >= 9);
    // Python exceptions come back as an error with the traceback in the log
    let r = run_code(&mut ed, Language::Python, "import flare as ac\nac.draw.rect(x=1)\n", &opts());
    assert!(r.error.is_some());
    assert!(r.log.iter().any(|l| l.contains("missing number")), "{:?}", r.log);
    // scripts written when the app was called AnimationCraft still run
    let r = run_code(&mut ed, Language::Python, "import animationcraft as ac\nac.draw.rect(x=1, y=1, w=5, h=5)\nassert ac.AnimationCraftError is ac.FlareError\n", &opts());
    assert!(r.ok(), "{:?}\n{}", r.error, r.log.join("\n"));
}

#[test]
fn node_bridge() {
    if !have("node") {
        eprintln!("node not installed; skipping");
        return;
    }
    let mut ed = Editor::new(Document::default());
    let r = run_code(&mut ed, Language::Node, Language::Node.example(), &opts());
    assert!(r.ok(), "{:?}\n{}", r.error, r.log.join("\n"));
    assert!(r.log.iter().any(|l| l.contains("Layers: 2")), "{:?}", r.log);
}

#[test]
fn ruby_bridge() {
    if !have("ruby") {
        eprintln!("ruby not installed; skipping");
        return;
    }
    let mut ed = Editor::new(Document::default());
    let r = run_code(&mut ed, Language::Ruby, Language::Ruby.example(), &opts());
    assert!(r.ok(), "{:?}\n{}", r.error, r.log.join("\n"));
    assert!(r.log.iter().any(|l| l.contains("Layers: 2")), "{:?}", r.log);
    // the old module name still works
    let r = run_code(&mut ed, Language::Ruby, "require \"animationcraft\"\nAnimationCraft.call(\"draw.rect\", x: 1, y: 1, w: 5, h: 5)\n", &opts());
    assert!(r.ok(), "{:?}\n{}", r.error, r.log.join("\n"));
}

#[test]
fn missing_interpreter_is_a_clear_error() {
    let mut ed = Editor::new(Document::default());
    let dir = std::env::temp_dir();
    let p = dir.join("ac-test.xyz");
    std::fs::write(&p, "x").unwrap();
    let mut o = opts();
    o.interpreters.push(("xyz".into(), vec![vec!["definitely-not-a-real-interpreter".into()]]));
    let r = run_file(&mut ed, &p, &o);
    assert!(r.error.unwrap().contains("couldn't start"));
}

#[test]
fn frame_scripts_control_playback() {
    let mut fp = FramePlayer::new().unwrap();
    assert_eq!(fp.run("this.stop();", 3, 10, &[]).unwrap(), vec![PlayAction::Stop]);
    let labels = vec![("loop".to_string(), 4)];
    assert_eq!(fp.run("this.gotoAndPlay('loop');", 9, 10, &labels).unwrap(), vec![PlayAction::Goto { frame: 4, play: true }]);
    assert_eq!(fp.run("gotoAndStop(2)", 9, 10, &labels).unwrap(), vec![PlayAction::Goto { frame: 2, play: false }]);
    // state persists between frames, like a published movie
    fp.run("this.count = (this.count || 0) + 1; trace('n', this.count);", 0, 10, &[]).unwrap();
    fp.run("this.count += 1; trace('n', this.count); if (this.currentFrame === 5) this.stop();", 5, 10, &[]).unwrap();
    assert_eq!(fp.take_log(), vec!["n 1", "n 2"]);
    assert!(fp.run("gotoAndPlay('nope')", 0, 10, &labels).unwrap_err().contains("nope"));
}

#[test]
fn frame_scripts_are_sandboxed() {
    let mut fp = FramePlayer::new().unwrap();
    // no editor API, no host I/O
    fp.run("trace(typeof ac, typeof __ac_call, typeof require, typeof std, typeof os, typeof fetch)", 0, 1, &[]).unwrap();
    assert_eq!(fp.take_log(), vec!["undefined undefined undefined undefined undefined undefined"]);
    // runaway loops and memory are stopped, and the player keeps working
    assert!(fp.run("for(;;){}", 0, 1, &[]).unwrap_err().contains("stopped"));
    assert!(fp.run("let a = []; for(;;) a.push(new Array(100000).fill(1));", 0, 1, &[]).is_err());
    assert_eq!(fp.run("stop()", 0, 1, &[]).unwrap(), vec![PlayAction::Stop]);
}

#[test]
fn frame_scripts_see_named_instances() {
    let mut fp = FramePlayer::new().unwrap();
    let ball = Instance { name: "ball".into(), x: 10.0, y: 20.0, rotation: 0.0, scale_x: 1.0, scale_y: 1.0, alpha: 1.0, visible: true, text: None };
    let (_, changed) = fp.run_with("this.ball.x += 5; this.getChildByName('ball').alpha = 0.5; trace(this.ball.y);", 0, 10, &[], std::slice::from_ref(&ball)).unwrap();
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].x, 15.0);
    assert_eq!(changed[0].alpha, 0.5);
    assert_eq!(fp.take_log(), vec!["20".to_string()]);
    let (_, none) = fp.run_with("var y = this.ball.y;", 1, 10, &[], &[ball]).unwrap();
    assert!(none.is_empty());
}

#[test]
fn jsfl_scripts_run_through_the_shim() {
    let mut ed = Editor::new(ac_core::Document::default());
    let code = r##"
var doc = fl.getDocumentDOM();
var tl = doc.getTimeline();
doc.width = 640;
doc.frameRate = 12;
var li = tl.addNewLayer("Boxes");
doc.setFillColor("#33AAFF");
for (var i = 0; i < 3; i++) {
  doc.addNewRectangle({ left: 10 + i * 50, top: 10, right: 50 + i * 50, bottom: 50 }, 4);
}
doc.selectAll();
fl.trace("selected " + doc.selection.length);
doc.convertToSymbol("movie clip", "Row", "center");
doc.selection[0].name = "row";
tl.insertFrames(9);
tl.insertKeyframe(9);
tl.currentFrame = 9;
doc.selectAll();
doc.moveSelectionBy({ x: 100, y: 0 });
tl.setFrameProperty("tweenType", "motion", 0, 1);
tl.layers[li].frames[0].name = "go";
tl.layers[li].frames[0].actionScript = "this.stop();";
doc.library.addNewItem("graphic", "Empty");
fl.trace(tl.layerCount + " layers, " + tl.frameCount + " frames, items: " + doc.library.items.map(function (i) { return i.name + ":" + i.itemType; }).join(","));
fl.trace("frame0 " + tl.layers[li].frames[0].tweenType + " " + tl.layers[li].frames[0].duration + " " + tl.getFrameProperty("name", 3));
try { tl.copyLayers(); } catch (e) { fl.trace("unsupported ok"); }
"##;
    let rep = run_code(&mut ed, Language::Jsfl, code, &ScriptOptions::default());
    assert!(rep.error.is_none(), "{:?} {:?}", rep.error, rep.log);
    assert_eq!(rep.log[0], "selected 3");
    assert_eq!(rep.log[1], "2 layers, 10 frames, items: Row:movie clip,Empty:graphic");
    assert_eq!(rep.log[2], "frame0 motion 9 go");
    assert_eq!(rep.log[3], "unsupported ok");
    assert_eq!((ed.doc.width, ed.doc.fps), (640.0, 12.0));
    let l = &ed.doc.scene.layers[ed.doc.scene.layers.iter().position(|l| l.name == "Boxes").unwrap()];
    let k0 = &l.keyframes[&0];
    assert!(matches!(k0.tween, ac_core::Tween::Classic { .. }));
    assert_eq!(k0.script.as_deref(), Some("this.stop();"));
    assert_eq!(k0.elements[0].name, "row");
    assert!((l.keyframes[&9].elements[0].matrix.tx - k0.elements[0].matrix.tx - 100.0).abs() < 0.01);
    // one undo step for the whole script
    assert!(ed.undo().is_some());
    assert_eq!(ed.doc.width, 550.0);
    // errors keep the script's own line numbers
    let rep = run_code(&mut ed, Language::Jsfl, "var a = 1;\nnosuchfn();", &ScriptOptions::default());
    assert!(rep.error.as_deref().unwrap_or("").contains(":2"), "{:?}", rep.error);
}

#[test]
fn frame_scripts_accept_html5_event_listeners() {
    let mut p = FramePlayer::new().unwrap();
    let inst = Instance { name: "go".into(), x: 1.0, y: 2.0, rotation: 0.0, scale_x: 1.0, scale_y: 1.0, alpha: 1.0, visible: true, text: None };
    let (_, changed) = p.run_with("this.go.addEventListener('click', () => {}); this.go.on('mouseover', () => {}); this.go.x = 5;", 0, 1, &[], &[inst]).unwrap();
    assert_eq!(changed[0].x, 5.0);
}

#[test]
fn jsfl_frames_clipboard_and_flfile() {
    let dir = std::env::temp_dir().join(format!("ac-jsfl-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut ed = Editor::new(Document::default());
    let code = r##"
var doc = fl.getDocumentDOM(), tl = doc.getTimeline();
doc.addNewRectangle({ left: 0, top: 0, right: 20, bottom: 20 }, 0);
tl.insertFrames(4);
tl.copyFrames(0, 1);
tl.pasteFrames(10);
var uri = FLfile.platformPathToURI("out/log.txt");
FLfile.createFolder(FLfile.platformPathToURI("out"));
FLfile.write(uri, "frames: " + tl.frameCount);
FLfile.write(uri, "\nmore", "append");
fl.trace(FLfile.read(uri));
fl.trace(FLfile.listFolder(FLfile.platformPathToURI("out")).join(","));
try { FLfile.remove(uri); } catch (e) { fl.trace("no remove"); }
"##;
    let o = ScriptOptions { base_dir: dir.clone(), ..opts() };
    let rep = run_code(&mut ed, Language::Jsfl, code, &o);
    assert!(rep.error.is_none(), "{:?} {:?}", rep.error, rep.log);
    assert_eq!(rep.log[0], "frames: 11\nmore");
    assert_eq!(rep.log[1], "log.txt");
    assert_eq!(rep.log[2], "no remove");
    assert!(ed.doc.scene.layers[0].keyframes.contains_key(&10), "{:?}", ed.doc.scene.layers[0].keyframes.keys().collect::<Vec<_>>());
    std::fs::remove_dir_all(&dir).ok();
}
