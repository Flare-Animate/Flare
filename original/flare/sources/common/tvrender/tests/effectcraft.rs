//! Motion Blur filter and EffectCraft effects (git dependency).

use ac_core::*;
use ac_render::render_frame;

fn px(doc: &Document, frame: u32, x: u32, y: u32) -> [u8; 4] {
    let p = render_frame(doc, frame, 1.0).pixel(x, y).unwrap().demultiply();
    [p.red(), p.green(), p.blue(), p.alpha()]
}

const RED: Color = Color::rgb(255, 0, 0);

/// A 20×20 red square that slides 50 px per frame to the right (classic tween,
/// frames 0 → 4), with `filters` on it.
fn sliding(filters: Vec<Filter>) -> Document {
    let mut d = Document { width: 300.0, height: 200.0, ..Default::default() };
    let mut a = Element::shape("r", Shape::new(Path::rect(0.0, 0.0, 20.0, 20.0), Some(RED), None));
    a.matrix = Matrix::translate(0.0, 90.0);
    a.filters = filters;
    let mut b = a.clone();
    b.matrix = Matrix::translate(200.0, 90.0);
    let mut l = Layer::new("square", 0);
    let mut k0 = Keyframe::with(vec![a]);
    k0.tween = Tween::Classic { ease: 0.0, rotate_turns: 0 };
    l.keyframes.insert(0, k0);
    l.keyframes.insert(4, Keyframe::with(vec![b]));
    l.end = 5;
    d.scene.layers = vec![l];
    d
}

#[test]
fn motion_blur_smears_along_the_real_motion() {
    let sharp = sliding(vec![]);
    let blurred = sliding(vec![Filter::MotionBlur { shutter: 360.0, samples: 16 }]);
    // frame 2: the square sits at x 100..120; a frame earlier it was at 50..70
    assert_eq!(px(&sharp, 2, 75, 100), [255, 255, 255, 255], "nothing behind a sharp square");
    let behind = px(&blurred, 2, 75, 100);
    assert!(behind[1] < 250 && behind[1] > 60, "streak behind the square: {behind:?}");
    // the leading edge thins out; above/below the path stays clean
    assert_eq!(px(&blurred, 2, 75, 60), [255, 255, 255, 255]);
    let front = px(&blurred, 2, 118, 100);
    assert!(front[1] > 100, "the leading edge is only covered for part of the shutter: {front:?}");
    // frame 0 has no previous frame: sharp
    assert_eq!(px(&blurred, 0, 10, 100), px(&sharp, 0, 10, 100));
    // the camera view carries the motion too (zoomed render keeps the streak)
    let big = render_frame(&blurred, 2, 2.0).pixel(150, 200).unwrap().demultiply();
    assert!(big.green() < 250, "streak at 2× zoom");
}

#[test]
fn motion_blur_survives_save_and_tweens_its_shutter() {
    let d = sliding(vec![Filter::MotionBlur { shutter: 180.0, samples: 8 }]);
    let back = Document::from_json(&d.to_json()).unwrap();
    assert_eq!(back.scene.layers[0].keyframes[&0].elements[0].filters, vec![Filter::MotionBlur { shutter: 180.0, samples: 8 }]);
    let a = Filter::MotionBlur { shutter: 0.0, samples: 8 };
    let b = Filter::MotionBlur { shutter: 360.0, samples: 8 };
    assert_eq!(a.lerp(&b, 0.25), Filter::MotionBlur { shutter: 90.0, samples: 8 });
}

#[test]
fn effectcraft_effects_run_from_their_registry() {
    let cat = ac_render::effectcraft::catalog();
    assert!(cat.len() > 100, "EffectCraft catalogue: {}", cat.len());
    let dir = ac_render::effectcraft::info("ec.blur.directional").expect("Directional Blur");
    assert_eq!(dir.name, "Directional Blur");
    assert!(dir.params.iter().any(|p| p.id == "length"));

    // a still square with a horizontal Directional Blur of 40 px
    let mut d = Document { width: 200.0, height: 200.0, ..Default::default() };
    let mut sq = Element::shape("r", Shape::new(Path::rect(80.0, 80.0, 40.0, 40.0), Some(RED), None));
    let plain = px(&{
        d.scene.layers = vec![{
            let mut l = Layer::new("a", 0);
            l.keyframes.insert(0, Keyframe::with(vec![sq.clone()]));
            l
        }];
        d.clone()
    }, 0, 70, 100);
    assert_eq!(plain, [255, 255, 255, 255]);
    sq.filters = vec![Filter::Effect { id: "ec.blur.directional".into(), params: vec![("direction".into(), EffectValue::Num(90.0)), ("length".into(), EffectValue::Num(40.0))] }];
    d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements = vec![sq.clone()];
    let left = px(&d, 0, 70, 100);
    assert!(left[1] < 250, "blur spreads left of the square: {left:?}");
    assert_eq!(px(&d, 0, 100, 60), [255, 255, 255, 255], "but not vertically");

    // round trip, and an unknown effect leaves the object as it is
    let back = Document::from_json(&d.to_json()).unwrap();
    assert_eq!(back.scene.layers[0].keyframes[&0].elements[0].filters, sq.filters);
    sq.filters = vec![Filter::Effect { id: "no.such.effect".into(), params: vec![] }];
    d.scene.layers[0].keyframes.get_mut(&0).unwrap().elements = vec![sq];
    assert_eq!(px(&d, 0, 100, 100), [255, 0, 0, 255]);
    assert_eq!(px(&d, 0, 70, 100), [255, 255, 255, 255]);
}

#[test]
fn html5_publishes_motion_blur_samples() {
    let d = sliding(vec![Filter::MotionBlur { shutter: 180.0, samples: 6 }]);
    let files = ac_render::html5::publish(&d, &ac_render::html5::Html5Opts::default());
    let js = files.iter().find(|f| f.0 == "movie.js").map(|f| String::from_utf8(f.1.clone()).unwrap()).unwrap();
    assert!(js.contains("\"mb\":{\"m\":["), "motion blur in the published movie");
    assert!(js.contains("\"n\":6"));
}
