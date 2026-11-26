//! `flare bitmap-compression`: the wiring of `flare_tools::library::set_bitmap_compression` to
//! the command line. What a photo is and how it is stored are tested in the document and the
//! storage; here the options, the saved project and the refusals.

use super::*;

/// A project with one 120x80 bitmap of a red rectangle, made by the command line itself.
fn with_bitmap(file: &str) {
    json(&["new", file]);
    json(&[
        "draw-rect",
        file,
        "--rect",
        "0",
        "0",
        "30",
        "20",
        "--color",
        "ff0000",
    ]);
    json(&["convert-to-bitmap", file, "--scale", "4"]);
}

fn quality(file: &str) -> Option<u8> {
    let doc = flare_storage::load(Path::new(file)).expect("load");
    doc.bitmaps()[0].photo().map(|p| p.quality())
}

#[test]
fn bitmaps_become_photos_at_a_quality_and_go_back_to_lossless() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    with_bitmap(&file);
    assert_eq!(quality(&file), None, "a converted drawing starts lossless");
    let out = json(&["bitmap-compression", &file, "photo"]);
    assert_eq!(out["changed"], 1);
    assert!(out["bitmap_bytes_after"].as_u64() < out["bitmap_bytes_before"].as_u64());
    assert_eq!(quality(&file), Some(92));
    let pixel = flare_storage::load(Path::new(&file))
        .expect("load")
        .bitmaps()[0]
        .pixels()[..3]
        .to_vec();
    assert!(
        pixel[0] > 200 && pixel[1] < 40 && pixel[2] < 40,
        "still red: {pixel:?}"
    );
    json(&["bitmap-compression", &file, "photo", "--quality", "50"]);
    assert_eq!(quality(&file), Some(50));
    let again = json(&["bitmap-compression", &file, "photo", "--quality", "50"]);
    assert_eq!(
        (again["changed"].as_u64(), again["unchanged"].as_u64()),
        (Some(0), Some(1))
    );
    json(&["bitmap-compression", &file, "lossless"]);
    assert_eq!(quality(&file), None);
}

#[test]
fn a_refusal_leaves_the_file_as_it_was() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    with_bitmap(&file);
    let before = std::fs::read(&file).expect("read");
    let unknown = "6f1c2d3e-4a5b-4c6d-8e7f-001122334455";
    for (args, says) in [
        (
            vec!["lossless", "--quality", "80"],
            "--quality is for photo",
        ),
        (vec!["photo", "--quality", "0"], "quality is from 1 to 100"),
        (vec!["photo", "--bitmap", unknown], "no bitmap"),
    ] {
        let mut all = vec!["bitmap-compression", file.as_str()];
        all.extend(args);
        let output = run(&all);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(says), "{stderr}");
        assert_eq!(std::fs::read(&file).expect("read"), before);
    }
}
