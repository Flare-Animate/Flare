//! HTML5 publish: every filter the model has reaches the page (canvas filter
//! strings, runtime pixel stages, or a baked sprite), and what can't be shown
//! exactly is reported.

use ac_core::*;
use ac_render::html5::{publish_report, publish_to_report, Html5Opts};

const RED: Color = Color::rgb(255, 0, 0);

fn doc_with(filters: Vec<Filter>, blend: BlendMode) -> Document {
    let mut d = Document { width: 120.0, height: 80.0, ..Default::default() };
    let mut sq = Element::shape("", Shape::new(Path::rect(40.0, 30.0, 20.0, 20.0), Some(RED), None));
    sq.filters = filters;
    sq.blend = blend;
    let mut l = Layer::new("a", 0);
    l.keyframes.insert(0, Keyframe::with(vec![sq]));
    d.scene.layers = vec![l];
    d
}

fn movie(files: &[(String, Vec<u8>)]) -> serde_json::Value {
    let js = files.iter().find(|f| f.0 == "movie.js").map(|f| String::from_utf8(f.1.clone()).unwrap()).unwrap();
    serde_json::from_str(js.trim_start_matches("var AC_MOVIE=").trim_end().trim_end_matches(';')).unwrap()
}

struct Png {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}
impl Png {
    fn a(&self, x: u32, y: u32) -> u8 {
        self.rgba[((y * self.width + x) * 4 + 3) as usize]
    }
}
fn decode(bytes: &[u8]) -> Png {
    let mut r = png::Decoder::new(std::io::Cursor::new(bytes)).read_info().unwrap();
    let mut buf = vec![0; r.output_buffer_size()];
    let info = r.next_frame(&mut buf).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    Png { width: info.width, height: info.height, rgba: buf }
}

fn all_filters() -> Vec<Filter> {
    let c = Color::rgb(0, 0, 0);
    vec![
        Filter::Blur { blur_x: 4.0, blur_y: 4.0, quality: 1 },
        Filter::DropShadow { blur_x: 4.0, blur_y: 4.0, strength: 100.0, quality: 1, angle: 45.0, distance: 4.0, color: c, knockout: false, inner: true, hide_object: false },
        Filter::Glow { blur_x: 4.0, blur_y: 4.0, strength: 100.0, quality: 1, color: RED, knockout: false, inner: false },
        Filter::Bevel { blur_x: 4.0, blur_y: 4.0, strength: 100.0, quality: 1, shadow: c, highlight: Color::rgb(255, 255, 255), angle: 45.0, distance: 4.0, knockout: false, kind: BevelType::Inner },
        Filter::GradientGlow { blur_x: 4.0, blur_y: 4.0, strength: 100.0, quality: 1, angle: 45.0, distance: 4.0, knockout: false, kind: BevelType::Outer, stops: vec![(0.0, Color { r: 0, g: 0, b: 0, a: 0 }), (1.0, RED)] },
        Filter::GradientBevel { blur_x: 4.0, blur_y: 4.0, strength: 100.0, quality: 1, angle: 45.0, distance: 4.0, knockout: false, kind: BevelType::Inner, stops: vec![(0.0, RED), (0.5, Color { r: 0, g: 0, b: 0, a: 0 }), (1.0, c)] },
        Filter::AdjustColor { brightness: 5.0, contrast: 5.0, saturation: 5.0, hue: 5.0 },
    ]
}

#[test]
fn every_model_filter_reaches_the_published_page() {
    let r = publish_report(&doc_with(all_filters(), BlendMode::Normal), &Html5Opts::default());
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    let m = movie(&r.files);
    let stages = m["fx"][0]["s"].as_array().expect("pixel stages");
    // blur (css) | inner shadow | glow | bevel | gradient glow | gradient bevel | adjust colour
    let kinds: Vec<String> = stages
        .iter()
        .map(|s| {
            if let Some(c) = s.get("c") {
                format!("css:{}", c.as_str().unwrap())
            } else if let Some(g) = s.get("g") {
                g["k"].as_str().unwrap().to_string()
            } else {
                "adjust".into()
            }
        })
        .collect();
    assert_eq!(kinds, ["css:blur(2px)", "glow", "glow", "sbevel", "glow", "bevel", "adjust"]);
    // and the page carries the code that runs them
    let rt = String::from_utf8(r.files.iter().find(|f| f.0 == "flare-runtime.js").unwrap().1.clone()).unwrap();
    assert!(rt.contains("function adjustColor") && rt.contains("\"sbevel\"") && rt.contains("function rampFilter"));
    let idx = String::from_utf8(r.files[0].1.clone()).unwrap();
    assert!(idx.contains("flare-runtime.js") && idx.contains("movie.js"));
}

#[test]
fn effectcraft_effects_are_baked_into_a_sprite_through_the_rasteriser() {
    let fx = Filter::Effect { id: "ec.blur.directional".into(), params: vec![("direction".into(), EffectValue::Num(90.0)), ("length".into(), EffectValue::Num(40.0))] };
    let r = publish_report(&doc_with(vec![fx], BlendMode::Screen), &Html5Opts::default());
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    let m = movie(&r.files);
    // the group is a sprite drawn with the blend, with no filter left to run
    assert_eq!(m["frames"][0][0]["sp"], 0, "{}", m["frames"][0]);
    assert_eq!(m["fx"][m["frames"][0][0]["x"].as_u64().unwrap() as usize], serde_json::json!({"f": "", "o": "screen"}));
    assert_eq!(m["images"][0][0], "images/fx1.png");
    let (_, png) = r.files.iter().find(|f| f.0 == "images/fx1.png").expect("sprite file");
    let img = decode(png);
    assert_eq!((img.width, img.height), (120, 80));
    // the directional blur spread it sideways but not vertically
    assert!(img.a(50, 40) > 60, "the square is there (blurred, so not fully opaque)");
    assert!(img.a(35, 40) > 0, "blurred out to the left");
    assert_eq!(img.a(50, 10), 0, "not vertically");
    assert_eq!(img.a(0, 0), 0, "elsewhere transparent");
}

#[test]
fn identical_baked_frames_share_one_sprite_and_single_file_inlines_it() {
    let fx = Filter::Effect { id: "ec.blur.directional".into(), params: vec![] };
    let mut d = doc_with(vec![fx], BlendMode::Normal);
    d.scene.layers[0].end = 4; // four identical frames
    let r = publish_report(&d, &Html5Opts::default());
    assert_eq!(r.files.iter().filter(|f| f.0.starts_with("images/fx")).count(), 1);
    let single = publish_report(&d, &Html5Opts { single_file: true, ..Default::default() });
    assert_eq!(single.files.len(), 1);
    let page = String::from_utf8(single.files[0].1.clone()).unwrap();
    assert!(page.contains("\"sp\":0") && page.contains("data:image/png;base64,"));
}

#[test]
fn what_the_page_cannot_show_exactly_is_reported_not_dropped() {
    let r = publish_report(&doc_with(vec![Filter::Effect { id: "no.such.effect".into(), params: vec![] }], BlendMode::Subtract), &Html5Opts::default());
    assert!(r.warnings.iter().any(|w| w.contains("no.such.effect")), "{:?}", r.warnings);
    assert!(r.warnings.iter().any(|w| w.contains("Subtract")), "{:?}", r.warnings);
    // the same notes ride along in the page's data
    let m = movie(&r.files);
    assert_eq!(m["warnings"].as_array().unwrap().len(), r.warnings.len());
}

#[test]
fn publish_to_returns_the_report_and_writes_the_files() {
    let dir = std::env::temp_dir().join(format!("flare-html5-{}", std::process::id()));
    let fx = Filter::Effect { id: "ec.blur.directional".into(), params: vec![] };
    let (page, warnings) = publish_to_report(&doc_with(vec![fx, Filter::Glow { blur_x: 3.0, blur_y: 3.0, strength: 100.0, quality: 1, color: RED, knockout: false, inner: false }], BlendMode::Normal), &Html5Opts::default(), &dir).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(page, dir.join("index.html"));
    for f in ["index.html", "movie.js", "flare-runtime.js", "images/fx1.png"] {
        assert!(dir.join(f).is_file(), "{f}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
