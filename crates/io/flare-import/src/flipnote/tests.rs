#![allow(clippy::expect_used)]
//! Flipnote imports end to end: magic detection, the refusals, and the converted document.

use super::ppm::synth::{SynthFrame, encode, wrap};
use super::ppm::{HEIGHT, WIDTH};
use crate::xfl::{ImportError, Severity};

fn blank() -> Vec<u8> {
    vec![0; WIDTH * HEIGHT]
}

fn two_frame_file() -> Vec<u8> {
    let mut first = blank();
    first[0] = 1;
    let key = SynthFrame {
        keyframe: true,
        inverted: false,
        pens: [1, 2],
        shift: None,
        layers: [first.clone(), blank()],
    };
    let second = SynthFrame {
        keyframe: false,
        inverted: false,
        pens: [1, 2],
        shift: None,
        layers: [first.clone(), blank()],
    };
    let zero = [blank(), blank()];
    let f0 = encode(&key, &zero, None);
    let f1 = encode(&second, &[first.clone(), blank()], None);
    wrap(&[f0, f1], &[0, 0], 2, &[], 0)
}

#[test]
fn ppm_magic_is_detected_and_kwz_magic_too() {
    assert!(super::looks_like_ppm(b"PARA\0\0"));
    assert!(!super::looks_like_ppm(b"PK\x03\x04"));
    assert!(super::looks_like_kwz(b"KFH\x14\xcc"));
    assert!(!super::looks_like_kwz(b"KF"));
    assert!(!super::looks_like_kwz(b"PARA"));
}

#[test]
fn a_ppm_imports_as_two_layers_and_paper_over_its_frames() {
    let imported = crate::import_bytes(two_frame_file()).expect("imports");
    let document = imported.document;
    let scene = &document.scenes()[0];
    assert_eq!(scene.timeline().frame_count(), 2);
    let layers = scene.timeline().layers();
    assert_eq!(layers.len(), 3, "layer 1, layer 2 and paper");
    assert_eq!(layers[0].name(), "Layer 1");
    // The second frame repeats the first picture: one span covers both frames.
    assert_eq!(layers[0].spans().len(), 1);
    assert_eq!(layers[0].spans()[0].duration().get(), 2);
    assert_eq!(layers[2].name(), "Paper");
    assert!(
        imported
            .diagnostics
            .iter()
            .any(|d| d.message.contains("2 frames at 12 a second"))
    );
}

#[test]
fn a_hidden_layer_imports_hidden() {
    let mut file = two_frame_file();
    // The flags word: bit 4 hides layer 1.
    file[0x6A6..0x6A8].copy_from_slice(&0x10_u16.to_le_bytes());
    // The metadata is only read when the version's high nibble is set.
    file[14] = 0x10;
    let imported = crate::import_bytes(file).expect("imports");
    let layers = imported.document.scenes()[0].timeline().layers().to_vec();
    assert!(!layers[0].is_visible());
    assert!(layers[1].is_visible());
}

#[test]
fn a_kwz_is_refused_by_name() {
    let err = crate::import_bytes(b"KFH\x14 and more".to_vec()).expect_err("refused");
    let message = err.to_string();
    assert!(message.contains("KWZ"), "{message}");
}

#[test]
fn a_non_ppm_named_ppm_is_refused_with_the_magic_in_the_message() {
    let err = super::import("picture.ppm", b"P6\n2 2\n255\n").expect_err("refused");
    assert!(
        matches!(err, ImportError::Foreign(ref m) if m.contains("PARA")),
        "{err}"
    );
}

#[test]
fn an_import_names_the_file_in_its_errors() {
    let mut file = two_frame_file();
    file[0x6A0..0x6A2].copy_from_slice(&0_u16.to_le_bytes());
    let err = super::import("broken.ppm", &file).expect_err("refused");
    let message = err.to_string();
    assert!(message.starts_with("broken.ppm:"), "{message}");
    assert!(message.contains("no frames"), "{message}");
}

#[test]
fn an_unimported_sound_track_is_reported_and_the_signature_note_is_there() {
    let key = SynthFrame {
        keyframe: true,
        inverted: false,
        pens: [1, 2],
        shift: None,
        layers: [blank(), blank()],
    };
    let frame = encode(&key, &[blank(), blank()], None);
    let file = wrap(&[frame], &[0], 2, &[0, 0, 0, 0, 0x11, 0x22], 0);
    let imported = super::import("sound.ppm", &file).expect("imports");
    assert!(imported.diagnostics.iter().any(
        |d| d.severity == Severity::Unsupported && d.message.contains("sound is not imported")
    ));
    assert!(
        imported
            .diagnostics
            .iter()
            .any(|d| d.message.contains("signature is not checked"))
    );
}

#[test]
fn a_ppm_file_imports_by_path_by_its_extension() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("picture.ppm");
    std::fs::write(&path, two_frame_file()).expect("write");
    let imported = crate::import_path(&path).expect("imports");
    assert_eq!(imported.document.scenes()[0].timeline().frame_count(), 2);
    let not_ppm = dir.path().join("plain.ppm");
    std::fs::write(&not_ppm, b"P6\n1 1\n255\n\0\0\0").expect("write");
    let err = crate::import_path(&not_ppm).expect_err("a netpbm image is not a Flipnote");
    assert!(err.to_string().contains("PARA"), "{err}");
}
