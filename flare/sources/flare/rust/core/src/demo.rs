//! Built-in demo document ("Bouncing Day"): nested symbols, a movie clip with
//! its own timeline, eased classic tweens and a shape tween.

use crate::geom::{pt, Matrix, Path};
use crate::model::*;
use std::f32::consts::PI;

fn shape(name: &str, path: Path, fill: Option<Color>, stroke: Option<Stroke>) -> Element {
    Element::shape(name, Shape { path, fill, stroke, ..Default::default() })
}

fn kf(els: Vec<Element>, tween: Tween) -> Keyframe {
    Keyframe { tween, ..Keyframe::with(els) }
}

pub fn demo_document() -> Document {
    let mut doc = Document { background: Color::rgb(0xBF, 0xE3, 0xFF), ..Default::default() };
    let total = 48;

    // ---- Ball (graphic symbol)
    let mut ball_tl = Timeline::new("Ball");
    ball_tl.layers[0].keyframes.insert(
        0,
        Keyframe::with(vec![
            shape("body", Path::ellipse(0.0, 0.0, 22.0, 22.0), Some(Color::rgb(0xE8, 0x3A, 0x3A)), Some(Stroke { color: Color::rgb(0x7A, 0x12, 0x12), width: 2.0 })),
            shape("shine", Path::ellipse(-7.0, -8.0, 7.0, 5.0), Some(Color { r: 255, g: 255, b: 255, a: 170 }), None),
        ]),
    );
    let ball = doc.add_symbol("Ball", SymbolKind::Graphic, ball_tl);

    // ---- Sun (movie clip: rays spin on its own timeline)
    let mut rays = Vec::new();
    for i in 0..8 {
        let a = i as f32 * PI / 4.0;
        let (s, c) = a.sin_cos();
        let p = |r: f32, da: f32| {
            let (s2, c2) = (a + da).sin_cos();
            let _ = (s, c);
            pt(r * c2, r * s2)
        };
        rays.push(shape("ray", Path::polygon(&[p(40.0, -0.18), p(62.0, 0.0), p(40.0, 0.18)]), Some(Color::rgb(0xFF, 0xB3, 0x00)), None));
    }
    let mut sun_tl = Timeline::new("Sun");
    sun_tl.layers[0].name = "core".into();
    sun_tl.layers[0].keyframes.insert(0, Keyframe::with(vec![shape("core", Path::ellipse(0.0, 0.0, 34.0, 34.0), Some(Color::rgb(0xFF, 0xD5, 0x3D)), Some(Stroke { color: Color::rgb(0xFF, 0xB3, 0x00), width: 3.0 }))]));
    sun_tl.layers[0].end = 24;
    let rays_tl = {
        let mut t = Timeline::new("rays");
        t.layers[0].keyframes.insert(0, Keyframe::with(rays));
        t
    };
    let rays_sym = doc.add_symbol("Sun Rays", SymbolKind::Graphic, rays_tl);
    let mut ray_layer = Layer::new("rays", 1);
    ray_layer.keyframes.insert(0, kf(vec![Element::instance("rays", rays_sym, Matrix::IDENTITY)], Tween::Classic { ease: 0.0, rotate_turns: 0 }));
    ray_layer.keyframes.insert(23, Keyframe::with(vec![Element::instance("rays", rays_sym, Matrix::rotate(PI / 4.0 * 0.96))]));
    ray_layer.end = 24;
    sun_tl.layers.push(ray_layer);
    let sun = doc.add_symbol("Sun", SymbolKind::MovieClip, sun_tl);

    // ---- Cloud (graphic)
    let mut cloud_tl = Timeline::new("Cloud");
    let white = Some(Color::rgb(255, 255, 255));
    cloud_tl.layers[0].keyframes.insert(
        0,
        Keyframe::with(vec![
            shape("puff", Path::ellipse(-30.0, 6.0, 30.0, 20.0), white, None),
            shape("puff", Path::ellipse(0.0, -8.0, 34.0, 28.0), white, None),
            shape("puff", Path::ellipse(32.0, 6.0, 28.0, 19.0), white, None),
        ]),
    );
    let cloud = doc.add_symbol("Cloud", SymbolKind::Graphic, cloud_tl);

    // ---- Scene layers (index 0 = top)
    let mut scene = Timeline { name: "Scene 1".into(), layers: vec![] };

    // Ball: eased bounce (ease-in falling, ease-out rising) with squash on contact.
    let mut ball_layer = Layer::new("Ball", 0);
    let ground = 318.0;
    let pos = [(50.0, 120.0), (150.0, ground), (250.0, 150.0), (350.0, ground), (440.0, 180.0), (500.0, ground)];
    let frames = [0u32, 9, 18, 27, 36, 47];
    for (i, (&f, &(x, y))) in frames.iter().zip(pos.iter()).enumerate() {
        let on_ground = y == ground;
        let m = if on_ground {
            Matrix::translate(x, y + 5.0).concat(&Matrix::scale(1.25, 0.75))
        } else {
            Matrix::translate(x, y)
        };
        let ease = if on_ground { 100.0 } else { -100.0 };
        let tween = if i + 1 < frames.len() { Tween::Classic { ease, rotate_turns: 0 } } else { Tween::None };
        ball_layer.keyframes.insert(f, kf(vec![Element::instance("Ball", ball, m)], tween));
    }
    ball_layer.end = total;
    scene.layers.push(ball_layer);

    // Morph: shape tween square → star → circle.
    let mut morph = Layer::new("Morph", 1);
    let center = (95.0, 235.0);
    morph.keyframes.insert(0, kf(vec![shape("morph", Path::rect(center.0 - 30.0, center.1 - 30.0, 60.0, 60.0), Some(Color::rgb(0x2D, 0x6C, 0xDF)), Some(Stroke { color: Color::rgb(0x12, 0x2E, 0x6B), width: 2.0 }))], Tween::Shape { ease: 0.0 }));
    morph.keyframes.insert(20, kf(vec![shape("morph", Path::star(center.0, center.1, 42.0, 18.0, 5), Some(Color::rgb(0xFF, 0xC4, 0x1F)), Some(Stroke { color: Color::rgb(0x9A, 0x6A, 0x00), width: 2.0 }))], Tween::Shape { ease: 0.0 }));
    morph.keyframes.insert(40, kf(vec![shape("morph", Path::ellipse(center.0, center.1, 32.0, 32.0), Some(Color::rgb(0x9B, 0x4D, 0xD8)), Some(Stroke { color: Color::rgb(0x4A, 0x1C, 0x70), width: 2.0 }))], Tween::None));
    morph.end = total;
    scene.layers.push(morph);

    let mut sun_layer = Layer::new("Sun", 2);
    sun_layer.keyframes.insert(0, Keyframe::with(vec![Element::instance("Sun", sun, Matrix::translate(470.0, 75.0))]));
    sun_layer.end = total;
    scene.layers.push(sun_layer);

    // Clouds drift (classic tween with alpha fade).
    let mut clouds = Layer::new("Clouds", 3);
    let c1 = Element::instance("Cloud", cloud, Matrix::translate(90.0, 70.0));
    let mut c2 = Element::instance("Cloud", cloud, Matrix::translate(300.0, 110.0).concat(&Matrix::scale(0.7, 0.7)));
    c2.alpha = 0.8;
    let c1b = Element::instance("Cloud", cloud, Matrix::translate(170.0, 70.0));
    let mut c2b = Element::instance("Cloud", cloud, Matrix::translate(250.0, 110.0).concat(&Matrix::scale(0.7, 0.7)));
    c2b.alpha = 0.45;
    clouds.keyframes.insert(0, kf(vec![c1, c2], Tween::Classic { ease: 0.0, rotate_turns: 0 }));
    clouds.keyframes.insert(47, Keyframe::with(vec![c1b, c2b]));
    clouds.end = total;
    scene.layers.push(clouds);

    let mut ground_l = Layer::new("Ground", 4);
    ground_l.locked = true;
    ground_l.keyframes.insert(
        0,
        Keyframe::with(vec![
            shape("hill", Path::ellipse(420.0, 360.0, 220.0, 70.0), Some(Color::rgb(0x5C, 0xB8, 0x5C)), None),
            shape("ground", Path::rect(0.0, 340.0, 550.0, 60.0), Some(Color::rgb(0x3E, 0x9A, 0x47)), None),
        ]),
    );
    ground_l.end = total;
    scene.layers.push(ground_l);

    doc.scene = scene;
    doc
}
