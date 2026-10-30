//! The HTML5 page: its data is read back, and what the page says is what was meant. The player is
//! run against a stand-in canvas by `scripts/tests/test_html5_player.py`.
#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{
    DocumentId, DocumentPoint, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SymbolId,
    Transform2D,
};
use flare_document::{
    Content, Document, FrameSpan, Gradient, Instance, Layer, PathSegment, Scene, Shape, Stage,
    Stroke, Symbol, Timeline, Tween,
};
use flare_publish::{Html5Options, PublishError, publish_html5, publish_html5_to_file};
use glamx::{DAffine2, DVec2};
use serde_json::Value;

fn span(start: u32, duration: u32, content: Content) -> FrameSpan {
    FrameSpan::new(start, NonZeroU32::new(duration).expect("nonzero"), content)
}

fn rect(x: f64, w: f64, fill: Rgba8) -> Shape {
    Shape::rect(ShapeId::new(), x, 0.0, w, 10.0, fill).expect("rect")
}

fn layer(name: &str, spans: Vec<FrameSpan>) -> Layer {
    Layer::new(LayerId::new(), name, spans).expect("layer")
}

fn document(scenes: Vec<Scene>, symbols: Vec<Symbol>) -> Document {
    let stage = Stage::new(320, 200, Rgba8::opaque(16, 32, 48)).expect("stage");
    Document::new(
        DocumentId::new(),
        FrameRate::new(12, 1).expect("rate"),
        scenes,
        symbols,
    )
    .expect("document")
    .with_stage(stage)
}

fn scene(layers: Vec<Layer>) -> Scene {
    Scene::new(SceneId::new(), "S", Timeline::new(layers))
}

/// The JSON the page carries.
fn data(html: &str) -> Value {
    let open = "<script id=\"flare-data\" type=\"application/json\">";
    let start = html.find(open).expect("data element") + open.len();
    let end = start + html[start..].find("</script>").expect("closed");
    serde_json::from_str(&html[start..end]).expect("the data is JSON")
}

fn page(document: &Document) -> (String, Value) {
    let published = publish_html5(document, &Html5Options::default()).expect("publish");
    let value = data(&published.html);
    (published.html, value)
}

#[test]
fn the_page_has_a_canvas_the_stage_the_rate_and_the_player() {
    let held = layer(
        "L",
        vec![span(0, 4, Content::Shape(rect(0.0, 5.0, Rgba8::WHITE)))],
    );
    let (html, d) = page(&document(vec![scene(vec![held])], vec![]));
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("<canvas id=\"stage\">") && html.contains("requestAnimationFrame"));
    assert_eq!((d["w"].as_u64(), d["h"].as_u64()), (Some(320), Some(200)));
    assert_eq!(d["fps"].as_f64(), Some(12.0));
    assert_eq!(d["bg"], serde_json::json!([16, 32, 48, 255]));
    assert_eq!(d["seq"].as_array().expect("seq").len(), 4);
}

#[test]
fn a_drawing_held_for_many_frames_is_written_once() {
    let held = layer(
        "L",
        vec![span(0, 24, Content::Shape(rect(0.0, 5.0, Rgba8::WHITE)))],
    );
    let (_, d) = page(&document(vec![scene(vec![held])], vec![]));
    assert_eq!(d["shapes"].as_array().expect("shapes").len(), 1);
    assert_eq!(d["frames"].as_array().expect("frames").len(), 1);
    assert_eq!(d["seq"].as_array().expect("seq").len(), 24);
    assert!(d["seq"].as_array().expect("seq").iter().all(|i| i == 0));
}

#[test]
fn a_tween_makes_a_new_frame_with_a_new_matrix_each_time() {
    let symbol = SymbolId::new();
    let art = Symbol::new(
        symbol,
        "Box",
        Timeline::new(vec![layer(
            "in",
            vec![span(0, 1, Content::Shape(rect(0.0, 10.0, Rgba8::WHITE)))],
        )]),
    );
    let at = |x: f64| {
        let t = Transform2D::new(DAffine2::from_translation(DVec2::new(x, 0.0))).expect("finite");
        Content::Instance(Instance::new(InstanceId::new(), symbol, t))
    };
    let moving = layer(
        "L",
        vec![
            span(0, 5, at(0.0)).with_tween(Tween::Linear),
            span(5, 1, at(50.0)),
        ],
    );
    let (_, d) = page(&document(vec![scene(vec![moving])], vec![art]));
    let frames = d["frames"].as_array().expect("frames");
    assert_eq!(frames.len(), 6, "five tweened frames and the last");
    let x_of = |i: usize| frames[i]["shapes"][0][5].as_f64().expect("e");
    for i in 1..frames.len() {
        assert!(x_of(i) > x_of(i - 1), "frame {i} is further along");
    }
    assert!((x_of(5) - 50.0).abs() < 1e-6);
}

#[test]
fn fills_gradients_strokes_and_invisible_shapes_are_written_as_what_they_are() {
    let path = vec![
        PathSegment::MoveTo(DocumentPoint::new(0.0, 0.0).expect("p")),
        PathSegment::CubicTo(
            DocumentPoint::new(1.0, 2.0).expect("p"),
            DocumentPoint::new(3.0, 4.0).expect("p"),
            DocumentPoint::new(5.0, 6.0).expect("p"),
        ),
        PathSegment::Close,
    ];
    let solid = Shape::new(ShapeId::new(), path.clone(), Rgba8::new(255, 0, 0, 128)).expect("s");
    let gradient = Shape::new(ShapeId::new(), path.clone(), Rgba8::WHITE)
        .expect("s")
        .with_gradient(
            Gradient::linear(
                DocumentPoint::new(0.0, 0.0).expect("p"),
                DocumentPoint::new(10.0, 0.0).expect("p"),
                Rgba8::opaque(1, 2, 3),
                Rgba8::opaque(4, 5, 6),
            )
            .expect("g"),
        );
    let outline = Shape::new(ShapeId::new(), path.clone(), Rgba8::TRANSPARENT)
        .expect("s")
        .with_stroke(Stroke::new(3.5, Rgba8::opaque(9, 8, 7)).expect("stroke"));
    let nothing = Shape::new(ShapeId::new(), path, Rgba8::TRANSPARENT).expect("s");
    let layers = [solid, gradient, outline, nothing]
        .into_iter()
        .map(|s| layer("L", vec![span(0, 1, Content::Shape(s))]))
        .collect();
    let (_, d) = page(&document(vec![scene(layers)], vec![]));
    let shapes = d["shapes"].as_array().expect("shapes");
    assert_eq!(shapes.len(), 3, "the shape that paints nothing is left out");
    // The list order is paint order, and the first layer is the top one: outline, gradient, solid.
    assert_eq!(shapes[2]["d"], "M 0 0C 1 2 3 4 5 6Z");
    assert_eq!(shapes[2]["fill"], serde_json::json!([255, 0, 0, 128]));
    assert_eq!(
        shapes[1]["grad"],
        serde_json::json!([0, 0, 10, 0, [1, 2, 3, 255], [4, 5, 6, 255]])
    );
    assert!(
        shapes[1].get("fill").is_none(),
        "a gradient replaces the solid fill"
    );
    assert_eq!(shapes[0]["stroke"][0].as_f64(), Some(3.5));
    assert!(shapes[0].get("fill").is_none());
    assert_eq!(d["frames"][0]["shapes"].as_array().expect("drawn").len(), 3);
}

#[test]
fn a_mask_is_written_as_a_clip_region_the_masked_shapes_refer_to() {
    let mask = layer(
        "mask",
        vec![span(0, 2, Content::Shape(rect(0.0, 5.0, Rgba8::WHITE)))],
    )
    .with_mask(true);
    let masked = layer(
        "art",
        vec![span(0, 2, Content::Shape(rect(0.0, 50.0, Rgba8::WHITE)))],
    );
    // A mask masks the layer after it in the list, which is the one below it in the timeline.
    let (_, d) = page(&document(vec![scene(vec![mask, masked])], vec![]));
    let frame = &d["frames"][0];
    assert_eq!(frame["clips"].as_array().expect("clips").len(), 1);
    let drawn = frame["shapes"].as_array().expect("drawn");
    assert_eq!(drawn.len(), 1, "only the masked shape draws");
    assert_eq!(
        drawn[0][7],
        serde_json::json!([0]),
        "and it names the region"
    );
}

#[test]
fn nothing_in_a_name_can_end_the_script_or_the_page() {
    let layer = layer(
        "L",
        vec![span(0, 1, Content::Shape(rect(0.0, 5.0, Rgba8::WHITE)))],
    );
    let options = Html5Options {
        title: "</title></head><script>alert(1)</script> & \"q\" 'x' \u{1}".to_owned(),
        ..Html5Options::default()
    };
    let published =
        publish_html5(&document(vec![scene(vec![layer])], vec![]), &options).expect("publish");
    assert!(!published.html.contains("<script>alert"));
    assert!(
        published
            .html
            .contains("&lt;/title&gt;&lt;/head&gt;&lt;script&gt;alert(1)")
    );
    assert!(published.html.contains("&amp; &quot;q&quot; &#39;x&#39;"));
    let (before_data, _) = published
        .html
        .split_once("<script id=\"flare-data\"")
        .expect("data");
    assert_eq!(
        before_data.matches("<script").count(),
        0,
        "nothing opens a script early"
    );
    assert_eq!(
        published.html.matches("</script>").count(),
        2,
        "the data and the player"
    );
}

#[test]
fn the_publish_settings_choose_looping_centering_and_scaling() {
    let held = layer(
        "L",
        vec![span(0, 2, Content::Shape(rect(0.0, 5.0, Rgba8::WHITE)))],
    );
    let doc = document(vec![scene(vec![held])], vec![]);
    let plain = publish_html5(
        &doc,
        &Html5Options {
            loop_timeline: false,
            center_stage: false,
            responsive: true,
            ..Html5Options::default()
        },
    )
    .expect("publish");
    let d = data(&plain.html);
    assert_eq!(
        (d["loop"].as_bool(), d["responsive"].as_bool()),
        (Some(false), Some(true))
    );
    assert!(!plain.html.contains("body{display:flex"));
    let default = publish_html5(&doc, &Html5Options::default()).expect("publish");
    let d = data(&default.html);
    assert_eq!(
        (d["loop"].as_bool(), d["responsive"].as_bool()),
        (Some(true), Some(false))
    );
    assert!(
        default
            .html
            .contains("body{display:flex;justify-content:center")
    );
}

#[test]
fn scenes_play_one_after_another_with_a_warning_and_too_long_a_movie_is_refused() {
    let one = |x: f64| {
        scene(vec![layer(
            "L",
            vec![span(0, 3, Content::Shape(rect(x, 5.0, Rgba8::WHITE)))],
        )])
    };
    let doc = document(vec![one(0.0), one(40.0)], vec![]);
    let published = publish_html5(&doc, &Html5Options::default()).expect("publish");
    assert_eq!(published.frames, 6);
    assert!(
        published
            .diagnostics
            .iter()
            .any(|d| d.message.contains("one after another"))
    );
    let long = layer(
        "L",
        vec![span(
            0,
            100_001,
            Content::Shape(rect(0.0, 5.0, Rgba8::WHITE)),
        )],
    );
    let result = publish_html5(
        &document(vec![scene(vec![long])], vec![]),
        &Html5Options::default(),
    );
    assert!(matches!(result, Err(PublishError::TooManyFrames(100_001))));
}

#[test]
fn the_page_is_written_whole_and_never_over_an_existing_file() {
    let held = layer(
        "L",
        vec![span(0, 2, Content::Shape(rect(0.0, 5.0, Rgba8::WHITE)))],
    );
    let doc = document(vec![scene(vec![held])], vec![]);
    let dir = tempfile::tempdir().expect("dir");
    let out = dir.path().join("movie.html");
    let published = publish_html5_to_file(&doc, &Html5Options::default(), &out).expect("write");
    assert_eq!(std::fs::read_to_string(&out).expect("read"), published.html);
    let again = publish_html5_to_file(&doc, &Html5Options::default(), &out);
    assert!(again.is_err());
    assert_eq!(
        std::fs::read_dir(dir.path()).expect("ls").count(),
        1,
        "no temp files"
    );
}

mod sound {
    use super::*;
    use flare_core::SoundId;
    use flare_document::{Sound, SoundCue};

    const WAV: &[u8] = b"RIFF\x24\x00\x00\x00WAVEfmt \x10\x00\x00\x00";

    fn sound(name: &str, bytes: &[u8]) -> Sound {
        Sound::new(SoundId::new(), name, bytes.to_vec(), 8000, 1, 8000).expect("sound")
    }

    fn scene_with(cues: Vec<SoundCue>, frames: u32) -> Scene {
        let layer = layer(
            "L",
            vec![span(
                0,
                frames,
                Content::Shape(rect(0.0, 5.0, Rgba8::WHITE)),
            )],
        )
        .with_cues(cues)
        .expect("cues");
        scene(vec![layer])
    }

    #[test]
    fn a_cued_sound_is_embedded_once_with_its_type_and_the_page_asks_for_a_click() {
        let heard = sound("beep", WAV);
        let unused = sound("never", WAV);
        let id = heard.id();
        let cues = vec![
            SoundCue {
                start: 1,
                sound: id,
            },
            SoundCue {
                start: 3,
                sound: id,
            },
        ];
        let doc = document(vec![scene_with(cues, 6)], vec![])
            .with_sounds(vec![heard, unused])
            .expect("sounds");
        let published = publish_html5(&doc, &Html5Options::default()).expect("publish");
        let d = data(&published.html);
        let sounds = d["sounds"].as_array().expect("sounds");
        assert_eq!(sounds.len(), 1, "a sound nothing cues is not carried");
        assert_eq!(sounds[0]["mime"], "audio/wav");
        assert_eq!(sounds[0]["name"], "beep");
        assert_eq!(d["cues"], serde_json::json!([[1, 0], [3, 0]]));
        assert!(published.html.contains("<div id=\"start\""));
        assert!(
            published
                .diagnostics
                .iter()
                .any(|m| m.message.contains("waits for a click"))
        );
    }

    #[test]
    fn cues_in_later_scenes_count_from_the_start_of_the_whole_movie() {
        let a = sound("a", WAV);
        let id = a.id();
        let first = scene_with(vec![], 5);
        let second = scene_with(
            vec![SoundCue {
                start: 2,
                sound: id,
            }],
            4,
        );
        let doc = document(vec![first, second], vec![])
            .with_sounds(vec![a])
            .expect("sounds");
        let d = data(
            &publish_html5(&doc, &Html5Options::default())
                .expect("publish")
                .html,
        );
        assert_eq!(d["cues"], serde_json::json!([[7, 0]]));
    }

    #[test]
    fn a_format_a_browser_may_not_play_is_left_out_with_a_warning_and_a_silent_movie_has_no_prompt()
    {
        let odd = sound("odd", b"MThd not audio at all");
        let id = odd.id();
        let doc = document(
            vec![scene_with(
                vec![SoundCue {
                    start: 0,
                    sound: id,
                }],
                3,
            )],
            vec![],
        )
        .with_sounds(vec![odd])
        .expect("sounds");
        let published = publish_html5(&doc, &Html5Options::default()).expect("publish");
        let d = data(&published.html);
        assert!(d["sounds"].as_array().expect("sounds").is_empty());
        assert!(d["cues"].as_array().expect("cues").is_empty());
        assert!(
            published
                .diagnostics
                .iter()
                .any(|m| m.message.contains("\"odd\""))
        );
        assert!(
            !published
                .diagnostics
                .iter()
                .any(|m| m.message.contains("waits for a click")),
            "nothing to wait for"
        );
    }

    #[test]
    fn each_common_format_is_told_by_how_it_begins() {
        let mime = |bytes: &[u8]| {
            let s = sound("s", bytes);
            let id = s.id();
            let doc = document(
                vec![scene_with(
                    vec![SoundCue {
                        start: 0,
                        sound: id,
                    }],
                    2,
                )],
                vec![],
            )
            .with_sounds(vec![s])
            .expect("sounds");
            let d = data(
                &publish_html5(&doc, &Html5Options::default())
                    .expect("publish")
                    .html,
            );
            d["sounds"]
                .get(0)
                .map(|s| s["mime"].as_str().expect("mime").to_owned())
        };
        assert_eq!(mime(b"ID3\x04\x00 mp3").as_deref(), Some("audio/mpeg"));
        assert_eq!(
            mime(&[0xFF, 0xFB, 0x90, 0x00]).as_deref(),
            Some("audio/mpeg")
        );
        assert_eq!(mime(b"OggS\x00\x02").as_deref(), Some("audio/ogg"));
        assert_eq!(mime(b"fLaC\x00\x00").as_deref(), Some("audio/flac"));
        assert_eq!(
            mime(b"RIFF\x00\x00\x00\x00AVI LIST").as_deref(),
            None,
            "RIFF that is not WAVE"
        );
    }
}
