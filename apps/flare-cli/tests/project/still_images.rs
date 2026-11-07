use super::*;

/// A 10x10 stage with a red square, made by the command line itself.
fn red_square(dir: &Path) -> String {
    let file = path(dir);
    json(&["new", &file, "--width", "10", "--height", "10"]);
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

#[test]
fn export_writes_jpeg_bmp_and_tga_at_the_stage_size() {
    let dir = tempfile::tempdir().expect("dir");
    let file = red_square(dir.path());

    let jpeg = dir.path().join("frame.jpg").to_string_lossy().into_owned();
    let info = json(&[
        "export",
        &file,
        &jpeg,
        "--format",
        "jpeg",
        "--quality",
        "95",
    ]);
    assert_eq!(info["width"], 10);
    assert_eq!(info["height"], 10);
    let bytes = std::fs::read(&jpeg).expect("jpeg");
    assert_eq!(
        &bytes[..3],
        &[0xFF, 0xD8, 0xFF],
        "JPEG start-of-image marker"
    );

    let bmp = dir.path().join("frame.bmp").to_string_lossy().into_owned();
    json(&["export", &file, &bmp, "--format", "bmp"]);
    assert_eq!(
        &std::fs::read(&bmp).expect("bmp")[..2],
        b"BM",
        "BMP signature"
    );

    let tga = dir.path().join("frame.tga").to_string_lossy().into_owned();
    json(&["export", &file, &tga, "--format", "tga"]);
    let bytes = std::fs::read(&tga).expect("tga");
    // The encoder may run-length compress the pixels (type 10); both are true-colour TGA.
    assert!(
        bytes[2] == 2 || bytes[2] == 10,
        "TGA true-colour image type, got {}",
        bytes[2]
    );
    assert_eq!(u16::from_le_bytes([bytes[12], bytes[13]]), 10, "TGA width");
    assert_eq!(u16::from_le_bytes([bytes[14], bytes[15]]), 10, "TGA height");
    assert_eq!(bytes[16], 32, "TGA bits per pixel");
}

#[test]
fn a_jpeg_refuses_transparency_and_a_quality_outside_its_range_and_writes_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let file = red_square(dir.path());

    let transparent = dir.path().join("clear.jpg").to_string_lossy().into_owned();
    let output = run(&[
        "export",
        &file,
        &transparent,
        "--format",
        "jpeg",
        "--transparent",
    ]);
    assert!(!output.status.success(), "a JPEG cannot be transparent");
    assert!(!Path::new(&transparent).exists(), "nothing is written");

    for quality in ["0", "101"] {
        let out = dir.path().join("q.jpg").to_string_lossy().into_owned();
        let output = run(&[
            "export",
            &file,
            &out,
            "--format",
            "jpeg",
            "--quality",
            quality,
        ]);
        assert!(
            !output.status.success(),
            "quality {quality} is outside 1 to 100"
        );
        assert!(
            !Path::new(&out).exists(),
            "a refused quality writes nothing"
        );
    }
}

#[test]
fn quality_is_only_for_jpeg() {
    let dir = tempfile::tempdir().expect("dir");
    let file = red_square(dir.path());
    let out = dir.path().join("f.png").to_string_lossy().into_owned();
    let output = run(&["export", &file, &out, "--quality", "50"]);
    assert!(!output.status.success(), "--quality with PNG is refused");
    assert!(!Path::new(&out).exists());
}

/// Each new still format is written at the stage size, with the file's own signature.
#[test]
fn export_writes_the_new_still_formats_with_their_signatures() {
    let dir = tempfile::tempdir().expect("dir");
    let file = red_square(dir.path());
    type Signature = fn(&[u8]) -> bool;
    let cases: [(&str, &str, Signature); 7] = [
        ("tiff", "frame.tif", |b| b.starts_with(b"II*\0")),
        ("webp", "frame.webp", |b| {
            b.starts_with(b"RIFF") && b.get(8..12) == Some(b"WEBP")
        }),
        ("qoi", "frame.qoi", |b| b.starts_with(b"qoif")),
        ("pnm", "frame.pnm", |b| b.starts_with(b"P7")),
        ("ico", "frame.ico", |b| b.starts_with(&[0, 0, 1, 0])),
        ("exr", "frame.exr", |b| {
            b.starts_with(&[0x76, 0x2f, 0x31, 0x01])
        }),
        ("hdr", "frame.hdr", |b| b.starts_with(b"#?RADIANCE")),
    ];
    for (format, name, signature) in cases {
        let out = dir.path().join(name).to_string_lossy().into_owned();
        let info = json(&["export", &file, &out, "--format", format]);
        assert_eq!(
            (info["width"].as_u64(), info["height"].as_u64()),
            (Some(10), Some(10)),
            "{format}"
        );
        let bytes = std::fs::read(&out).expect(format);
        assert!(signature(&bytes), "{format}: wrong file signature");
    }
}

#[test]
fn hdr_refuses_transparency_and_ico_refuses_more_than_256_pixels_and_write_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let file = red_square(dir.path());
    let hdr = dir.path().join("t.hdr").to_string_lossy().into_owned();
    let refused = run(&["export", &file, &hdr, "--format", "hdr", "--transparent"]);
    assert!(
        !refused.status.success(),
        "HDR with --transparent is refused"
    );
    assert!(
        !std::path::Path::new(&hdr).exists(),
        "a refused export writes nothing"
    );

    let big = dir.path().join("big.flare").to_string_lossy().into_owned();
    json(&["new", &big, "--width", "300", "--height", "300"]);
    let ico = dir.path().join("big.ico").to_string_lossy().into_owned();
    let refused = run(&["export", &big, &ico, "--format", "ico"]);
    assert!(!refused.status.success(), "a 300 pixel ICO is refused");
    assert!(!std::path::Path::new(&ico).exists());
    let small = dir.path().join("small.ico").to_string_lossy().into_owned();
    json(&[
        "export", &big, &small, "--format", "ico", "--width", "64", "--height", "64",
    ]);
}
