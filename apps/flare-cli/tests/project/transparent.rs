//! `flare export --transparent`: every format with an alpha channel leaves the stage colour out,
//! and the ones without refuse it and write nothing.

use super::*;

/// A 20x20 white stage with a red square in its top-left quarter, made by the command line.
fn square(dir: &Path) -> String {
    let file = path(dir);
    json(&["new", &file, "--width", "20", "--height", "20"]);
    json(&[
        "draw-rect",
        &file,
        "--rect",
        "0",
        "0",
        "10",
        "10",
        "--color",
        "FF0000",
    ]);
    file
}

fn out(dir: &Path, name: &str) -> String {
    dir.join(name).to_string_lossy().into_owned()
}

#[test]
fn a_transparent_gif_apng_and_webp_leave_the_stage_out() {
    let dir = tempfile::tempdir().expect("dir");
    let file = square(dir.path());

    let gif = out(dir.path(), "a.gif");
    json(&["export", &file, &gif, "--format", "gif", "--transparent"]);
    // The Graphic Control Extension (0x21 0xF9, size 4) flags a see-through colour.
    let bytes = std::fs::read(&gif).expect("gif");
    assert!(
        bytes
            .windows(4)
            .any(|w| w[..3] == [0x21, 0xF9, 0x04] && w[3] & 1 == 1),
        "the GIF has a see-through colour"
    );

    let apng = out(dir.path(), "a.png");
    json(&["export", &file, &apng, "--format", "apng", "--transparent"]);
    let image = read_png(&apng);
    assert_eq!(pixel(&image, 5, 5), [255, 0, 0, 255]);
    assert_eq!(pixel(&image, 15, 15)[3], 0);

    let webp = out(dir.path(), "a.webp");
    json(&[
        "export",
        &file,
        &webp,
        "--format",
        "animated-webp",
        "--transparent",
    ]);
    assert!(std::fs::read(&webp).expect("webp").starts_with(b"RIFF"));
}

#[test]
fn a_transparent_svg_and_psd_leave_the_stage_out() {
    let dir = tempfile::tempdir().expect("dir");
    let file = square(dir.path());
    let (clear, opaque) = (out(dir.path(), "c.svg"), out(dir.path(), "o.svg"));
    json(&["export", &file, &clear, "--format", "svg", "--transparent"]);
    json(&["export", &file, &opaque, "--format", "svg"]);
    let clear = std::fs::read_to_string(&clear).expect("svg");
    let opaque = std::fs::read_to_string(&opaque).expect("svg");
    // The white stage is a rectangle painted #ffffff; the transparent picture has none.
    assert!(opaque.to_ascii_lowercase().contains("#ffffff"), "{opaque}");
    assert!(!clear.to_ascii_lowercase().contains("#ffffff"), "{clear}");

    let psd = out(dir.path(), "a.psd");
    json(&["export", &file, &psd, "--format", "psd", "--transparent"]);
    assert!(std::fs::read(&psd).expect("psd").starts_with(b"8BPS"));
}

#[test]
fn the_formats_without_alpha_refuse_transparent_and_write_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let file = square(dir.path());
    for (format, name) in [("mp4", "a.mp4"), ("avi", "a.avi"), ("jpeg", "a.jpg")] {
        let target = out(dir.path(), name);
        let refused = run(&[
            "export",
            &file,
            &target,
            "--format",
            format,
            "--transparent",
        ]);
        assert!(!refused.status.success(), "{format}");
        let said = String::from_utf8_lossy(&refused.stderr);
        assert!(said.contains("transparent"), "{format}: {said}");
        assert!(!Path::new(&target).exists(), "{format}: nothing is written");
    }
}
