#![allow(clippy::expect_used)]
//! Real Flipnote files, read from the directory named by `FLARE_FLIPNOTE_SAMPLES`. Nothing is
//! committed: the files are licence-unclear user work, so the test is skipped without the
//! variable. Each PPM must import and every frame must decode; each KWZ must be refused by name.

use std::path::PathBuf;

use flare_import::flipnote::ppm::{FrameDecoder, Ppm};

fn sample_dir() -> Option<PathBuf> {
    std::env::var_os("FLARE_FLIPNOTE_SAMPLES").map(PathBuf::from)
}

#[test]
fn every_real_ppm_decodes_every_frame() {
    let Some(dir) = sample_dir() else {
        eprintln!("FLARE_FLIPNOTE_SAMPLES is not set; skipping");
        return;
    };
    let mut seen = 0;
    for entry in std::fs::read_dir(&dir).expect("sample dir") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("ppm") {
            continue;
        }
        seen += 1;
        let bytes = std::fs::read(&path).expect("read");
        let ppm = Ppm::parse(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let mut dec = FrameDecoder::new();
        for i in 0..ppm.frame_count() {
            let info = ppm
                .decode_frame(&mut dec, i)
                .unwrap_or_else(|e| panic!("{} frame {i}: {e}", path.display()));
            // A frame never reads past the next frame in the file. Real files leave up to a dozen
            // unreferenced bytes after some frames (leftovers of Flipnote Studio's own buffer);
            // the pointer table, not the decoded length, bounds each frame.
            let start = ppm.frame_offset(i).expect("offset");
            let next = (0..ppm.frame_count())
                .filter_map(|j| ppm.frame_offset(j))
                .filter(|&o| o > start)
                .min()
                .unwrap_or(ppm.sound().effect_flags_at);
            assert!(
                info.end <= next,
                "{} frame {i} reads past the next frame",
                path.display()
            );
            assert!(
                next - info.end <= 16,
                "{} frame {i}: {} unread bytes",
                path.display(),
                next - info.end
            );
        }
        flare_import::import_path(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    }
    assert!(seen > 0, "no .ppm files in {}", dir.display());
}

#[test]
fn every_real_kwz_is_refused_by_name() {
    let Some(dir) = sample_dir() else {
        return;
    };
    for entry in std::fs::read_dir(&dir).expect("sample dir") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("kwz") {
            continue;
        }
        let bytes = std::fs::read(&path).expect("read");
        let err = flare_import::import_bytes(bytes).expect_err("refused");
        assert!(err.to_string().contains("KWZ"), "{}: {err}", path.display());
    }
}
