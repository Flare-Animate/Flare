//! Phase 4 publishing: HTML5 Canvas, sprite sheets, animated SVG, Lottie.

use ac_core::*;
use ac_render::formats::{lottie, sprite_sheet, AtlasFormat, SheetOpts};
use ac_render::html5::{publish, Html5Opts};
use ac_render::{export_animated_svg, ExportOpts};

/// 10 frames: a static square, a ball tweening right on frames 1-5, then still.
fn doc() -> Document {
    let mut d = Document { width: 100.0, height: 50.0, fps: 10.0, ..Default::default() };
    let mut bg = Layer::new("bg", 0);
    bg.keyframes.insert(0, Keyframe::with(vec![Element::shape("", Shape::new(Path::rect(0.0, 0.0, 20.0, 20.0), Some(Color::rgb(0, 0, 255)), None))]));
    bg.end = 10;
    let mut ball = Layer::new("ball", 1);
    let el = |x: f32| Element { name: "ball".into(), matrix: Matrix::translate(x, 25.0), ..Element::group(vec![Element::shape("", Shape::new(Path::ellipse(0.0, 0.0, 5.0, 5.0), Some(Color::rgb(255, 0, 0)), None))]) };
    let mut k0 = Keyframe::with(vec![el(10.0)]);
    k0.tween = Tween::Classic { ease: 0.0, rotate_turns: 0 };
    k0.script = Some("this.stop();".into());
    k0.label = Some("start".into());
    ball.keyframes.insert(0, k0);
    ball.keyframes.insert(4, Keyframe::with(vec![el(90.0)]));
    ball.end = 10;
    d.scene.layers = vec![ball, bg];
    d
}

#[test]
fn html5_shares_static_content_and_carries_scripts() {
    let files = publish(&doc(), &Html5Opts { loop_play: true, ..Default::default() });
    let names: Vec<&str> = files.iter().map(|f| f.0.as_str()).collect();
    assert_eq!(names, ["index.html", "movie.js", "flare-runtime.js"]);
    let movie = String::from_utf8(files[1].1.clone()).unwrap();
    let json = movie.trim_start_matches("var AC_MOVIE=").trim_end().trim_end_matches(';');
    let m: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(m["frames"].as_array().unwrap().len(), 10);
    // the square + 5 ball positions (frames 4-9 are the same) = 6 items, 2 paths
    assert_eq!(m["items"].as_array().unwrap().len(), 6, "{}", m["items"]);
    assert_eq!(m["paths"].as_array().unwrap().len(), 2);
    assert_eq!(m["extra"][0]["js"][0], "this.stop();");
    assert_eq!(m["extra"][0]["n"]["ball"]["x"], 10);
    assert_eq!(m["labels"][0][0], "start");
    let index = String::from_utf8(files[0].1.clone()).unwrap();
    assert!(index.contains("movie.js") && index.contains("<canvas id=\"ac-stage\" width=\"100\" height=\"50\""));
    // one self-contained page
    let single = publish(&doc(), &Html5Opts { single_file: true, ..Default::default() });
    assert_eq!(single.len(), 1);
    let page = String::from_utf8(single[0].1.clone()).unwrap();
    assert!(page.contains("var AC_MOVIE=") && page.contains("Flare HTML5 Canvas runtime") && !page.contains("src=\"movie.js\""));
}

#[test]
fn sprite_sheet_dedupes_and_writes_atlases() {
    let o = ExportOpts { transparent: true, ..Default::default() };
    let (png_bytes, atlas) = sprite_sheet(&doc(), &o, &SheetOpts::default(), "ball.png").unwrap();
    let a: serde_json::Value = serde_json::from_str(&atlas).unwrap();
    let frames = a["frames"].as_object().unwrap();
    assert_eq!(frames.len(), 10);
    // frames 5..10 show the same picture: one cell
    assert_eq!(frames["ball0005"]["frame"], frames["ball0009"]["frame"]);
    assert_ne!(frames["ball0000"]["frame"], frames["ball0001"]["frame"]);
    assert_eq!(a["meta"]["image"], "ball.png");
    let dec = png::Decoder::new(std::io::Cursor::new(png_bytes)).read_info().unwrap();
    let (w, h) = (dec.info().width, dec.info().height);
    assert_eq!((a["meta"]["size"]["w"].as_u64().unwrap() as u32, a["meta"]["size"]["h"].as_u64().unwrap() as u32), (w, h));
    // trimmed: the first frame's cell is the content's bounding box
    assert!(frames["ball0000"]["trimmed"].as_bool().unwrap());
    let xml = sprite_sheet(&doc(), &o, &SheetOpts { format: AtlasFormat::StarlingXml, ..Default::default() }, "ball.png").unwrap().1;
    assert_eq!(xml.matches("<SubTexture").count(), 10);
    let arr = sprite_sheet(&doc(), &o, &SheetOpts { format: AtlasFormat::JsonArray, ..Default::default() }, "ball.png").unwrap().1;
    let arr: serde_json::Value = serde_json::from_str(&arr).unwrap();
    assert_eq!(arr["frames"][3]["filename"], "ball0003");
}

#[test]
fn animated_svg_and_lottie_cover_every_frame() {
    let svg = export_animated_svg(&doc(), &ExportOpts::default());
    // the square is in every frame group; frames 4-9 share one group
    assert_eq!(svg.matches("<animate ").count(), 5, "{svg}");
    assert!(svg.contains("repeatCount=\"indefinite\""));
    let (json, warn) = lottie(&doc(), &ExportOpts::default());
    assert!(warn.is_empty(), "{warn:?}");
    let l: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(l["op"], 10);
    assert_eq!(l["w"], 100);
    let layers = l["layers"].as_array().unwrap();
    // 5 distinct frames (one shape layer each) + the stage colour
    assert_eq!(layers.len(), 6);
    assert_eq!((layers[4]["ip"].as_u64(), layers[4]["op"].as_u64()), (Some(4), Some(10)));
    // the red ball is a bezier with a red fill
    let shapes = layers[0]["shapes"].as_array().unwrap();
    assert!(shapes.iter().any(|g| g["it"].as_array().unwrap().iter().any(|x| x["ty"] == "fl" && x["c"]["k"][0] == 1)));
}

/// A movie clip blinking red/green/blue/white on a main timeline that a
/// script stops on frame 1.
fn held_doc() -> Document {
    let mut d = Document { width: 40.0, height: 40.0, fps: 10.0, ..Default::default() };
    let mut tl = Timeline::new("Blink");
    let cols = [Color::rgb(255, 0, 0), Color::rgb(0, 255, 0), Color::rgb(0, 0, 255), Color::rgb(255, 255, 255)];
    for (i, c) in cols.iter().enumerate() {
        tl.layers[0].keyframes.insert(i as u32, Keyframe::with(vec![Element::shape("", Shape::new(Path::rect(0.0, 0.0, 40.0, 40.0), Some(*c), None))]));
    }
    tl.layers[0].end = 4;
    d.library.push(Symbol { id: 1, name: "Blink".into(), kind: SymbolKind::MovieClip, timeline: tl });
    d.next_symbol_id = 2;
    let mut k = Keyframe::with(vec![Element::instance("", 1, Matrix::IDENTITY)]);
    k.script = Some("this.stop();".into());
    d.scene.layers[0].keyframes.insert(0, k);
    d.scene.layers[0].end = 2;
    d
}

#[test]
fn held_frames_keep_movie_clips_playing() {
    let d = held_doc();
    assert_eq!(d.movie_clip_lengths(0), vec![4]);
    let files = publish(&d, &Html5Opts::default());
    let movie = String::from_utf8(files[1].1.clone()).unwrap();
    let json = movie.trim_start_matches("var AC_MOVIE=").trim_end().trim_end_matches(';');
    let m: serde_json::Value = serde_json::from_str(json).unwrap();
    let v = m["extra"][0]["v"].as_array().expect("clock variants for the held frame");
    assert_eq!(v.len(), 4);
    assert_ne!(v[0], v[1]);
    assert_eq!(v[0], m["frames"][0], "phase 0 is the normal frame");
}

#[test]
fn lottie_masks_become_track_mattes() {
    let mut d = Document { width: 100.0, height: 100.0, ..Default::default() };
    let mut mask = Layer::new("Mask", 0);
    mask.kind = LayerKind::Mask;
    mask.keyframes.insert(0, Keyframe::with(vec![Element::shape("", Shape::new(Path::ellipse(50.0, 50.0, 30.0, 30.0), Some(Color::rgb(0, 0, 0)), None))]));
    let mut art = Layer::new("Art", 1);
    art.parent = Some(0);
    art.keyframes.insert(0, Keyframe::with(vec![Element::shape("", Shape::new(Path::rect(0.0, 0.0, 100.0, 100.0), Some(Color::rgb(255, 0, 0)), None))]));
    d.scene.layers = vec![mask, art];
    let (json, warn) = lottie(&d, &ExportOpts::default());
    assert!(warn.is_empty(), "{warn:?}");
    let l: serde_json::Value = serde_json::from_str(&json).unwrap();
    let layers = l["layers"].as_array().unwrap();
    let matte = layers.iter().position(|x| x["td"] == 1).expect("a matte layer");
    assert_eq!(layers[matte + 1]["tt"], 1, "the masked content follows its matte");
}

#[test]
fn gradient_filters_publish_as_pixel_stages() {
    let mut d = doc();
    let el = &mut d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements[0];
    el.filters = vec![Filter::default_of("blur").unwrap(), Filter::default_of("gradient glow").unwrap(), Filter::default_of("adjust color").unwrap()];
    let files = publish(&d, &Html5Opts::default());
    let movie = String::from_utf8(files[1].1.clone()).unwrap();
    let json = movie.trim_start_matches("var AC_MOVIE=").trim_end().trim_end_matches(';');
    let m: serde_json::Value = serde_json::from_str(json).unwrap();
    let fx = m["fx"].as_array().unwrap().iter().find(|f| f.get("s").is_some()).expect("a staged effect");
    let s = fx["s"].as_array().unwrap();
    assert_eq!(s.len(), 3, "css blur, the ramp glow, css adjust colour: {s:?}");
    assert!(s[0]["c"].as_str().unwrap().starts_with("blur("));
    assert_eq!(s[1]["g"]["k"], "glow");
    assert_eq!(s[1]["g"]["r"].as_array().unwrap().len(), 2);
    assert!(s[2]["c"].is_string());
    let rt = String::from_utf8(files[2].1.clone()).unwrap();
    assert!(rt.contains("function rampFilter"));
}

/// Asset Warp on a bitmap: the stage/PNG render, SVG (embedded picture) and
/// HTML5 (mesh in the movie data) all carry the deformation; .fla warns.
#[test]
fn asset_warp_on_a_bitmap_reaches_every_export() {
    use ac_core::commands::execute;
    use serde_json::json;
    let mut ed = Editor::new(Document { width: 100.0, height: 100.0, ..Default::default() });
    let rgba: Vec<u8> = (0..40).flat_map(|_| (0..40).flat_map(|_| [200u8, 30, 30, 255])).collect();
    let id = ed.add_bitmap(bitmap::BitmapAsset::from_rgba(0, "Red", 40, 40, &rgba).unwrap());
    ed.place_bitmap(id, pt(20.0, 20.0)).unwrap();
    execute(&mut ed, "select.all", &json!({})).unwrap();
    let before = ac_render::render_frame_opts(&ed.doc, 0, 1.0, true);
    let at = |p: &tiny_skia::Pixmap, x: u32, y: u32| p.pixel(x, y).unwrap().alpha();
    assert_eq!(at(&before, 72, 72), 0);
    // three corners pinned, the fourth pulled out by 20 px
    execute(&mut ed, "selection.mesh_warp", &json!({"pins": [[20,20,20,20],[60,20,60,20],[20,60,20,60],[60,60,80,80]]})).unwrap();
    let after = ac_render::render_frame_opts(&ed.doc, 0, 1.0, true);
    assert!(at(&after, 72, 72) > 200, "the pulled corner reaches out");
    assert!(at(&after, 22, 22) > 200 && at(&after, 10, 10) == 0, "pinned corner stays");
    let svg = ac_render::export_svg(&ed.doc, 0);
    assert!(svg.contains("<image x=") && svg.contains("data:image/png;base64"));
    let files = publish(&ed.doc, &Html5Opts::default());
    if let Ok(dir) = std::env::var("AC_WARP_HTML5_OUT") {
        let page = publish(&ed.doc, &Html5Opts { single_file: true, ..Default::default() });
        std::fs::write(std::path::Path::new(&dir).join("warp.html"), &page[0].1).unwrap();
    }
    let movie = String::from_utf8(files[1].1.clone()).unwrap();
    assert!(movie.contains("\"mw\":{\"c\":"), "mesh in the HTML5 movie");
    let runtime = String::from_utf8(files[2].1.clone()).unwrap();
    assert!(runtime.contains("function warpMesh"));
    // reset removes it
    execute(&mut ed, "selection.mesh_warp", &json!({"reset": true})).unwrap();
    assert_eq!(at(&ac_render::render_frame_opts(&ed.doc, 0, 1.0, true), 72, 72), 0);
}

/// Particle emitters: placed by command, deterministic per frame, drawn by
/// the stage renderer and every export; symbol particles; editing by command.
#[test]
fn particles_render_in_every_export() {
    use ac_core::commands::execute;
    use serde_json::json;
    let mut ed = Editor::new(Document { width: 200.0, height: 200.0, fps: 24.0, ..Default::default() });
    ed.timeline_mut().layers[0].end = 24;
    let r = execute(&mut ed, "draw.particles", &json!({"x": 100, "y": 150, "rate": 40, "life": 1, "speed": 80, "spread": 30, "gravity": [0, 0], "color_start": "#FF0000", "color_end": "#FF0000", "fade_in": 0, "fade_out": 0, "seed": 7})).unwrap();
    assert_eq!(r["index"], json!(0));
    let items = |ed: &Editor, f: u32| ac_core::evaluate_scene(&ed.doc, f);
    // frame 0: the (invisible) emitter frame and the first particle
    assert_eq!(items(&ed, 0).iter().filter(|i| i.fill.is_some()).count(), 1);
    let n12 = items(&ed, 12).iter().filter(|i| i.fill.is_some()).count();
    assert_eq!(n12, 21, "40/s for half a second");
    assert_eq!(items(&ed, 12).len(), items(&ed, 12).len());
    // pixels above the emitter are red at frame 12
    let pix = ac_render::render_frame_opts(&ed.doc, 12, 1.0, true);
    let red = pix.pixels().iter().filter(|p| p.alpha() > 0 && p.red() > 200 && p.green() < 60).count();
    assert!(red > 50, "particles drawn: {red}");
    // exports: SVG paths, HTML5 frames differ, Lottie has content
    let svg = ac_render::export_svg(&ed.doc, 12);
    assert!(svg.matches("#ff0000").count() + svg.matches("#FF0000").count() >= 20, "SVG has the particles");
    let files = publish(&ed.doc, &Html5Opts::default());
    if let Ok(dir) = std::env::var("AC_WARP_HTML5_OUT") {
        let page = publish(&ed.doc, &Html5Opts { single_file: true, ..Default::default() });
        std::fs::write(std::path::Path::new(&dir).join("particles.html"), &page[0].1).unwrap();
        ac_render::render_frame(&ed.doc, 12, 1.0).save_png(std::path::Path::new(&dir).join("particles12.png")).unwrap();
    }
    let movie = String::from_utf8(files[1].1.clone()).unwrap();
    let json = movie.trim_start_matches("var AC_MOVIE=").trim_end().trim_end_matches(';');
    let m: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_ne!(m["frames"][2], m["frames"][12]);
    let (lot, _) = lottie(&ed.doc, &ExportOpts::default());
    assert!(lot.len() > 2000);
    // a symbol as the particle, tinted
    let sym = ed.new_symbol("Spark", SymbolKind::Graphic);
    ed.doc.symbol_mut(sym).unwrap().timeline.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::shape("", Shape::new(Path::rect(-3.0, -3.0, 6.0, 6.0), Some(Color::rgb(0, 0, 255)), None))]));
    execute(&mut ed, "symbol.exit", &json!({})).unwrap();
    execute(&mut ed, "select.all", &json!({})).unwrap();
    execute(&mut ed, "selection.particles", &json!({"source": {"symbol": "Spark"}, "rate": 10})).unwrap();
    let blues = items(&ed, 12).iter().filter(|i| i.fill == Some(Color::rgb(0, 0, 255))).count();
    assert_eq!(blues, 6, "10/s, 0.5 s → 6 symbol particles");
    assert!(execute(&mut ed, "selection.particles", &json!({"source": "smoke"})).is_err());
    // survives JSON save/load
    let back: Document = serde_json::from_str(&serde_json::to_string(&ed.doc).unwrap()).unwrap();
    assert_eq!(ac_core::evaluate_scene(&back, 12).len(), items(&ed, 12).len());
}

#[test]
fn text_fields_publish_for_scripts_and_links() {
    use ac_core::text::TextKind;
    let mut d = Document::default();
    let mut el = Element::text("0", 20.0, Color::rgb(0, 0, 0), ac_core::pt(10.0, 10.0));
    el.name = "score".into();
    if let ElementKind::Text(t) = &mut el.kind {
        t.field.kind = TextKind::Dynamic;
    }
    let mut link = Element::text("site", 12.0, Color::rgb(0, 0, 255), ac_core::pt(10.0, 60.0));
    link.name.clear();
    if let ElementKind::Text(t) = &mut link.kind {
        t.field.link = Some("https://example.org".into());
    }
    let kf = d.scene.layers[0].keyframes.get_mut(&0).unwrap();
    kf.elements.push(el);
    kf.elements.push(link);
    kf.script = Some("this.score.text = 'GOAL';".into());
    let files = publish(&d, &Html5Opts::default());
    let all: String = files.iter().map(|(_, b)| String::from_utf8_lossy(b).to_string()).collect();
    assert!(all.contains("\"tx\":{\"k\":1,\"s\":\"0\""), "dynamic text described");
    assert!(all.contains("__link0_1") && all.contains("\"url\":\"https://example.org\""), "link entry");
    assert!(all.contains("function drawLive") && all.contains("placeInput"), "runtime draws live text and inputs");
}
