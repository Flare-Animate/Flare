#![allow(clippy::expect_used)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use flare_core::{DocumentPoint, Rgba8, ShapeId};
use flare_document::TextSource;

use crate::{Done, Runtime, TextJob, add_google_fonts, text_shape};

/// A small Google font (Squada One, SIL Open Font License, licence text beside it).
const SQUADA: &[u8] =
    include_bytes!("../../../foundation/flare-text/data/test/SquadaOne-Regular.ttf");

fn source(family: &str) -> TextSource {
    TextSource::new(
        "Flare",
        family,
        40.0,
        false,
        false,
        DocumentPoint::new(0.0, 0.0).expect("point"),
    )
    .expect("source")
}

fn set(id: u64, family: &str) -> TextJob {
    TextJob::Set {
        id,
        source: source(family),
        shape: ShapeId::new(),
        fill: Rgba8::opaque(0, 0, 0),
    }
}

fn add(fonts: Result<Vec<Vec<u8>>, String>) -> TextJob {
    TextJob::AddFonts {
        family: "Squada One".into(),
        fonts,
    }
}

/// What came back, in short: `wanted:<families>`, `added`, `refused`, `text:<id>`, `preview:<id>`.
fn answers(runtime: &mut Runtime) -> Vec<String> {
    runtime
        .poll()
        .into_iter()
        .map(|done| match done {
            Done::FontsWanted(families) => format!("wanted:{}", families.join("+")),
            Done::FontsAdded { result: Ok(()), .. } => "added".into(),
            Done::FontsAdded { result: Err(_), .. } => "refused".into(),
            Done::Text { id, result } => {
                assert!(result.is_ok(), "{result:?}");
                format!("text:{id}")
            }
            Done::TextPreview { id, .. } => format!("preview:{id}"),
            _ => "other".into(),
        })
        .collect()
}

#[test]
fn text_in_a_missing_google_font_waits_for_it_and_is_then_set_in_it() {
    let mut runtime = Runtime::inline();
    runtime.submit_text(set(1, "Squada One, sans-serif"));
    assert_eq!(answers(&mut runtime), ["wanted:Squada One"]);
    runtime.submit_text(add(Ok(vec![SQUADA.to_vec()])));
    let done = runtime.poll();
    assert!(matches!(&done[0], Done::FontsAdded { result: Ok(()), .. }));
    let Some(Done::Text {
        id: 1,
        result: Ok(shape),
    }) = done.get(1)
    else {
        panic!("the waiting text was not set");
    };
    let fallback = text_shape(
        &source("sans-serif"),
        ShapeId::new(),
        Rgba8::opaque(0, 0, 0),
        &Arc::default(),
    )
    .expect("fallback");
    assert_ne!(
        shape.path(),
        fallback.path(),
        "not set in the downloaded font"
    );
    runtime.submit_text(set(2, "Squada One"));
    assert_eq!(
        answers(&mut runtime),
        ["text:2"],
        "asked for a family it has"
    );
}

#[test]
fn a_family_that_cannot_be_had_falls_back_and_is_not_asked_for_again() {
    let mut runtime = Runtime::inline();
    runtime.submit_text(set(1, "Squada One"));
    assert_eq!(answers(&mut runtime), ["wanted:Squada One"]);
    runtime.submit_text(add(Err("offline".into())));
    assert_eq!(answers(&mut runtime), ["refused", "text:1"]);
    runtime.submit_text(set(2, "Squada One"));
    assert_eq!(answers(&mut runtime), ["text:2"]);
    // Bytes that are not the family are refused the same way.
    runtime.submit_text(TextJob::AddFonts {
        family: "Abel".into(),
        fonts: Ok(vec![SQUADA.to_vec()]),
    });
    assert_eq!(answers(&mut runtime), ["refused"]);
}

#[test]
fn newer_text_replaces_text_that_waits_and_previews_do_not_wait() {
    let mut runtime = Runtime::inline();
    runtime.submit_text(set(1, "Squada One"));
    runtime.submit_text(set(2, "sans-serif"));
    assert_eq!(answers(&mut runtime), ["wanted:Squada One", "text:2"]);
    runtime.submit_text(TextJob::Preview {
        id: 3,
        text: "Fla".into(),
        style: flare_text::TextStyle {
            family: "Squada One".into(),
            size: 40.0,
            bold: false,
            italic: false,
        },
        origin: DocumentPoint::new(0.0, 0.0).expect("point"),
        cursor: 3,
        shape: ShapeId::new(),
        fill: Rgba8::opaque(0, 0, 0),
    });
    assert_eq!(answers(&mut runtime), ["preview:3", "wanted:Squada One"]);
    runtime.submit_text(add(Ok(vec![SQUADA.to_vec()])));
    assert_eq!(
        answers(&mut runtime),
        ["added"],
        "the replaced text came back, or the settled preview was answered twice"
    );
}

#[test]
fn the_text_worker_waits_for_fonts_too() {
    let mut runtime = Runtime::inline();
    runtime.start_threads(Arc::new(|| {}));
    let wait_for = |runtime: &mut Runtime, count: usize| {
        let (start, mut got) = (Instant::now(), Vec::new());
        while got.len() < count && start.elapsed() < Duration::from_secs(60) {
            got.extend(answers(runtime));
            std::thread::sleep(Duration::from_millis(5));
        }
        got
    };
    runtime.submit_text(set(1, "Squada One"));
    assert_eq!(wait_for(&mut runtime, 1), ["wanted:Squada One"]);
    runtime.submit_text(add(Ok(vec![SQUADA.to_vec()])));
    assert_eq!(wait_for(&mut runtime, 2), ["added", "text:1"]);
}

#[test]
fn a_program_with_no_window_downloads_first_and_fails_by_name() {
    let fonts = Arc::default();
    let error = add_google_fonts("Squada One", &fonts, |_| Err("offline".into()))
        .expect_err("cannot be had");
    assert!(
        error.contains("Squada One") && error.contains("offline"),
        "{error}"
    );
    let error =
        add_google_fonts("Abel", &fonts, |_| Ok(vec![SQUADA.to_vec()])).expect_err("not Abel");
    assert!(error.contains("Abel"), "{error}");
    let asked = std::cell::Cell::new(0);
    let fetch = |name: &str| {
        asked.set(asked.get() + 1);
        assert_eq!(name, "Squada One");
        Ok(vec![SQUADA.to_vec()])
    };
    add_google_fonts("Squada One, sans-serif", &fonts, fetch).expect("downloads");
    add_google_fonts("Squada One", &fonts, fetch).expect("already here");
    add_google_fonts("sans-serif, Squada One", &fonts, fetch).expect("not needed");
    assert_eq!(asked.get(), 1);
    assert!(crate::font_families(&fonts).contains(&"Squada One".to_owned()));
    assert!(!crate::google_font_families(&fonts).contains(&"Squada One"));
    assert!(crate::google_font_families(&fonts).len() > 1500);
}
