//! Motion object tweens: the curves of an `AnimationCore` are sampled into one keyframe per frame,
//! and what is not imported is reported.
#![allow(clippy::expect_used)]

use std::collections::HashMap;

use flare_document::{Content, Tween};
use flare_import::{FileSource, Severity, import_xfl};

#[derive(Default)]
struct Files(HashMap<String, String>);

impl FileSource for Files {
    fn read(&self, name: &str) -> std::io::Result<Option<Vec<u8>>> {
        Ok(self.0.get(name).map(|s| s.clone().into_bytes()))
    }
}

/// Animate writes the matrix of an instance like this.
fn instance(a: f64, b: f64, c: f64, d: f64, tx: f64, ty: f64) -> String {
    format!(
        r#"<DOMSymbolInstance libraryItemName="S"><matrix><Matrix a="{a}" b="{b}" c="{c}" d="{d}" tx="{tx}" ty="{ty}"/></matrix></DOMSymbolInstance>"#
    )
}

/// One property with its keys; each key is `(ticks, value, next handle, previous handle)`.
fn property(id: &str, keys: &[(u32, f64, Option<&str>, Option<&str>)]) -> String {
    let keys: String = keys
        .iter()
        .map(|(t, v, next, previous)| {
            format!(
                r#"<Keyframe anchor="0,{v}" next="{}" previous="{}" roving="0" timevalue="{t}"/>"#,
                next.map_or(format!("0,{v}"), str::to_owned),
                previous.map_or(format!("0,{v}"), str::to_owned),
            )
        })
        .collect();
    format!(
        r#"<Property enabled="1" id="{id}" ignoreTimeMap="0" readonly="0" visible="1">{keys}</Property>"#
    )
}

/// A motion object with the given containers, which hold properties.
fn motion(containers: &[(&str, String)]) -> String {
    let body: String = containers
        .iter()
        .map(|(id, props)| format!(r#"<PropertyContainer id="{id}">{props}</PropertyContainer>"#))
        .collect();
    format!(
        r#"<motionObjectXML><AnimationCore TimeScale="24000" Version="1" duration="3000"><TimeMap strength="0" type="Quadratic"/><PropertyContainer id="headContainer">{body}</PropertyContainer></AnimationCore></motionObjectXML>"#
    )
}

/// A tween of `duration` frames from `start` (with its curves) and the end frame after it.
fn tween_frames(duration: u32, start: &str, curves: &str) -> String {
    format!(
        r#"<DOMFrame index="0" duration="{duration}" tweenType="motion object"><elements>{start}</elements>{curves}</DOMFrame>
           <DOMFrame index="{duration}" duration="1"><elements>{start}</elements></DOMFrame>"#
    )
}

fn import(frames: &str) -> flare_import::Imported {
    let xml = format!(
        r#"<DOMDocument frameRate="24" width="100" height="100"><symbols><Include href="S.xml"/></symbols>
        <timelines><DOMTimeline name="Main"><layers><DOMLayer name="L"><frames>{frames}</frames></DOMLayer></layers></DOMTimeline></timelines></DOMDocument>"#
    );
    let mut files = Files::default();
    files.0.insert("DOMDocument.xml".to_owned(), xml);
    files.0.insert(
        "LIBRARY/S.xml".to_owned(),
        r#"<DOMSymbolItem name="S" symbolType="graphic"><timeline><DOMTimeline name="S"/></timeline></DOMSymbolItem>"#
            .to_owned(),
    );
    import_xfl(&files).expect("import")
}

/// Every span of the first layer: its start, its tween and its instance's matrix.
fn spans(imported: &flare_import::Imported) -> Vec<(u32, Tween, flare_core::Transform2D)> {
    imported.document.scenes()[0].timeline().layers()[0]
        .spans()
        .iter()
        .filter_map(|span| match span.content() {
            Content::Instance(made) => Some((span.start(), span.tween(), made.transform())),
            _ => None,
        })
        .collect()
}

fn warnings(imported: &flare_import::Imported) -> Vec<String> {
    imported
        .diagnostics
        .iter()
        .filter(|d| d.severity != Severity::Info)
        .map(|d| d.message.clone())
        .collect()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn a_moving_motion_object_is_one_keyframe_per_frame_at_its_curve() {
    let start = instance(1.0, 0.0, 0.0, 1.0, 10.0, 0.0);
    let curves = motion(&[(
        "Basic_Motion",
        property(
            "Motion_X",
            &[(0, 0.0, None, None), (3000, 30.0, None, None)],
        ),
    )]);
    let imported = import(&tween_frames(3, &start, &curves));
    let spans = spans(&imported);
    assert_eq!(spans.len(), 4, "three sampled frames and the end frame");
    for (i, (start_frame, tween, transform)) in spans.iter().take(3).enumerate() {
        assert_eq!(*start_frame, i as u32);
        assert_eq!(*tween, Tween::None);
        assert!(near(
            transform.as_affine().translation.x,
            10.0 + 10.0 * i as f64
        ));
    }
    assert!(warnings(&imported).is_empty(), "{:?}", warnings(&imported));
}

#[test]
fn the_handles_of_a_key_bend_the_curve() {
    let start = instance(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    // Flat leaving the first key, so the first frame moves less than a straight line would.
    let curves = motion(&[(
        "Basic_Motion",
        property(
            "Motion_X",
            &[
                (0, 0.0, Some("2000,0"), None),
                (3000, 30.0, None, Some("1000,30")),
            ],
        ),
    )]);
    let imported = import(&tween_frames(3, &start, &curves));
    let first = spans(&imported)[1].2.as_affine().translation.x;
    assert!(first < 10.0 - 0.5, "ease-in expected, found {first}");
}

#[test]
fn rotation_and_scale_act_on_the_matrix_from_its_start() {
    let start = instance(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    let curves = motion(&[(
        "Transformation",
        property(
            "Rotation_Z",
            &[(0, 0.0, None, None), (3000, 90.0, None, None)],
        ) + &property(
            "Scale_X",
            &[(0, 100.0, None, None), (3000, 200.0, None, None)],
        ),
    )]);
    let imported = import(&tween_frames(3, &start, &curves));
    let at_two = spans(&imported)[2].2.as_affine();
    // Two of three frames in: 60 degrees, and the x scale at 100 + 100 * 2/3 percent.
    let angle = at_two
        .matrix2
        .x_axis
        .y
        .atan2(at_two.matrix2.x_axis.x)
        .to_degrees();
    assert!((angle - 60.0).abs() < 1e-6, "angle {angle}");
    assert!(near(
        at_two.matrix2.x_axis.length(),
        100.0 / 100.0 + 2.0 / 3.0
    ));
}

#[test]
fn the_first_frame_is_the_instance_as_it_was_placed() {
    // A 90 degree instance, so the start is not the identity.
    let start = instance(0.0, 1.0, -1.0, 0.0, 10.0, 20.0);
    let curves = motion(&[(
        "Basic_Motion",
        property("Motion_Y", &[(0, 0.0, None, None), (3000, 5.0, None, None)]),
    )]);
    let imported = import(&tween_frames(3, &start, &curves));
    let first = spans(&imported)[0].2.as_affine();
    let expected = flare_core::Transform2D::new(glamx::DAffine2::from_cols_array(&[
        0.0, 1.0, -1.0, 0.0, 10.0, 20.0,
    ]))
    .expect("finite")
    .as_affine();
    assert!(near(first.translation.x, expected.translation.x));
    assert!(near(first.translation.y, expected.translation.y));
    assert!(near(first.matrix2.x_axis.x, expected.matrix2.x_axis.x));
    assert!(near(first.matrix2.x_axis.y, expected.matrix2.x_axis.y));
    assert!(near(first.matrix2.y_axis.x, expected.matrix2.y_axis.x));
    assert!(near(first.matrix2.y_axis.y, expected.matrix2.y_axis.y));
}

#[test]
fn a_colour_curve_is_reported_and_the_motion_still_samples() {
    let start = instance(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    let curves = motion(&[
        (
            "Basic_Motion",
            property(
                "Motion_X",
                &[(0, 0.0, None, None), (3000, 30.0, None, None)],
            ),
        ),
        (
            "Colors",
            property(
                "Alpha_Amount",
                &[(0, 100.0, None, None), (3000, 0.0, None, None)],
            ),
        ),
    ]);
    let imported = import(&tween_frames(3, &start, &curves));
    assert_eq!(spans(&imported).len(), 4);
    let notes = warnings(&imported).join("\n");
    assert!(notes.contains("colour curves are not imported"), "{notes}");
}

#[test]
fn a_time_map_is_reported_and_the_keys_keep_their_own_timing() {
    let start = instance(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    let curves = motion(&[(
        "Basic_Motion",
        property(
            "Motion_X",
            &[(0, 0.0, None, None), (3000, 30.0, None, None)],
        ),
    )])
    .replace(
        r#"<TimeMap strength="0" type="Quadratic"/>"#,
        r#"<TimeMap strength="60" type="BounceIn"/>"#,
    );
    let imported = import(&tween_frames(3, &start, &curves));
    assert_eq!(spans(&imported).len(), 4);
    let notes = warnings(&imported).join("\n");
    assert!(
        notes.contains("the time map BounceIn is not imported"),
        "{notes}"
    );
}

#[test]
fn a_quadratic_time_map_eases_the_whole_motion() {
    let start = instance(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    // A straight line from 0 to 30 over three frames, eased out with the full strength.
    let curves = motion(&[(
        "Basic_Motion",
        property(
            "Motion_X",
            &[(0, 0.0, None, None), (3000, 30.0, None, None)],
        ),
    )])
    .replace(
        r#"<TimeMap strength="0" type="Quadratic"/>"#,
        r#"<TimeMap strength="100" type="Quadratic"/>"#,
    );
    let imported = import(&tween_frames(3, &start, &curves));
    let middle = spans(&imported)[1].2.as_affine().translation.x;
    // 1 - (2/3)^2 of the way, not a third.
    assert!(
        (middle - 30.0 * (1.0 - (2.0_f64 / 3.0).powi(2))).abs() < 1e-6,
        "{middle}"
    );
    assert!(warnings(&imported).is_empty(), "{:?}", warnings(&imported));
}

#[test]
fn rotation_turns_about_the_transformation_point() {
    // The instance's point (10, 0) is where the motion turns it, so it stays where it is.
    let start = r#"<DOMSymbolInstance libraryItemName="S"><matrix><Matrix tx="0" ty="0"/></matrix>
        <transformationPoint><Point x="10" y="0"/></transformationPoint></DOMSymbolInstance>"#;
    let curves = motion(&[(
        "Transformation",
        property(
            "Rotation_Z",
            &[(0, 0.0, None, None), (3000, 90.0, None, None)],
        ),
    )]);
    let imported = import(&tween_frames(3, start, &curves));
    let at_two = spans(&imported)[2].2.as_affine();
    let pivot = at_two.transform_point2(glamx::DVec2::new(10.0, 0.0));
    assert!(near(pivot.x, 10.0) && near(pivot.y, 0.0), "{pivot:?}");
    // The origin, ten pixels off the pivot, has moved.
    let origin = at_two.transform_point2(glamx::DVec2::ZERO);
    assert!(!near(origin.x, 0.0) || !near(origin.y, 0.0), "{origin:?}");
}

#[test]
fn a_motion_object_without_curves_stays_a_straight_tween_with_a_reason() {
    let start = instance(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    let imported = import(&tween_frames(3, &start, ""));
    let spans = spans(&imported);
    assert_eq!(spans[0].1, Tween::Linear);
    assert!(
        warnings(&imported)
            .join("\n")
            .contains("motion object without curves")
    );
}

#[test]
fn a_curve_that_cannot_be_built_is_reported_and_the_tween_stays_linear() {
    let start = instance(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    // Two keys at the same time cannot make a curve.
    let curves = motion(&[(
        "Basic_Motion",
        property("Motion_X", &[(0, 0.0, None, None), (0, 30.0, None, None)]),
    )]);
    let imported = import(&tween_frames(3, &start, &curves));
    assert_eq!(spans(&imported)[0].1, Tween::Linear);
    let notes = warnings(&imported).join("\n");
    assert!(notes.contains("could not be read"), "{notes}");
}

#[test]
fn a_motion_object_longer_than_the_sampling_bound_stays_linear() {
    let start = instance(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    let curves = motion(&[(
        "Basic_Motion",
        property(
            "Motion_X",
            &[(0, 0.0, None, None), (3000, 30.0, None, None)],
        ),
    )]);
    // One more than the sampling bound (4096 frames) in motion/mod.rs.
    let too_long = 4097;
    let imported = import(&tween_frames(too_long, &start, &curves));
    assert_eq!(spans(&imported)[0].1, Tween::Linear);
    assert!(warnings(&imported).join("\n").contains("longer than"));
}

#[test]
fn a_key_with_a_value_that_is_not_a_number_is_ignored_not_fatal() {
    let start = instance(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    let bad = r#"<Keyframe anchor="0,nope" next="0,0" previous="0,0" roving="0" timevalue="0"/>"#;
    let curves = motion(&[(
        "Basic_Motion",
        format!(
            r#"<Property enabled="1" id="Motion_X">{bad}<Keyframe anchor="0,0" next="0,0" previous="0,0" roving="0" timevalue="3000"/></Property>"#
        ),
    )]);
    let imported = import(&tween_frames(3, &start, &curves));
    assert_eq!(spans(&imported).len(), 4);
}
