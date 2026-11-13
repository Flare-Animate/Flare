//! The QuickTime movie: the MP4 with QuickTime's brand.

use std::io::Cursor;

use super::*;

/// The `ftyp` box a QuickTime movie starts with: 24 bytes, brand `qt  `, minor version 0x200, and
/// `qt  ` then `isom` among the compatible brands.
const QT_FTYP: [u8; 24] = *b"\0\0\0\x18ftypqt  \0\0\x02\0qt  isom";

#[test]
fn a_mov_is_the_mp4_byte_for_byte_but_for_its_quicktime_brand() {
    let dir = tempfile::tempdir().expect("dir");
    let document = with_sound(30, 0, tone_wav(4000));
    let (mp4, mov) = (dir.path().join("a.mp4"), dir.path().join("a.mov"));
    write_mp4(&document, &mp4).expect("mp4");
    write_mov(&document, &mov).expect("mov");
    let (mp4, mov) = (
        std::fs::read(&mp4).expect("read"),
        std::fs::read(&mov).expect("read"),
    );
    assert_eq!(mov[..24], QT_FTYP);
    assert_ne!(mp4[..24], QT_FTYP, "the MP4 keeps its own brand");
    assert_eq!(mov.len(), mp4.len());
    assert!(
        mov[24..] == mp4[24..],
        "the picture and the sound are the same"
    );
    let (names, tracks) = boxes(&mov);
    assert_eq!(names.first().map(String::as_str), Some("ftyp"));
    assert_eq!(tracks, 2, "a picture and a sound track");
}

#[test]
fn a_mov_never_replaces_a_file_and_leaves_nothing_behind_when_it_cannot_be_made() {
    let dir = tempfile::tempdir().expect("dir");
    let target = dir.path().join("a.mov");
    std::fs::write(&target, b"mine").expect("write");
    assert!(
        write_mov(&animation(3, (20, 20)), &target)
            .expect_err("exists")
            .contains("already exists")
    );
    assert_eq!(std::fs::read(&target).expect("read"), b"mine");
    let broken = dir.path().join("broken.mov");
    assert!(write_mov(&with_sound(6, 0, vec![1, 2, 3, 4, 5, 6, 7, 8]), &broken).is_err());
    assert!(!broken.exists());
    assert_eq!(std::fs::read_dir(dir.path()).expect("ls").count(), 1);
}

#[test]
fn only_a_file_that_starts_with_a_file_type_box_is_rebranded() {
    for bytes in [
        Vec::new(),
        b"\0\0\0\x18moovqt  \0\0\x02\0qt  isom".to_vec(),
        // A box longer than the file.
        b"\0\0\0\x40ftypisom\0\0\x02\0isom".to_vec(),
        // A size that does not end on a whole brand.
        b"\0\0\0\x17ftypisom\0\0\x02\0isommp4".to_vec(),
    ] {
        let mut file = Cursor::new(bytes.clone());
        assert!(quicktime::rebrand(&mut file).is_err(), "{bytes:?}");
        assert_eq!(file.into_inner(), bytes, "left as it was");
    }
    let mut file = Cursor::new(b"\0\0\0\x18ftypisom\0\0\x02\0isommp41rest".to_vec());
    quicktime::rebrand(&mut file).expect("rebranded");
    let bytes = file.into_inner();
    assert_eq!(bytes[..24], QT_FTYP);
    assert_eq!(&bytes[24..], b"rest", "nothing after the box moves");
}

/// With ffprobe on the machine (this checks nothing, and says so, where there is none): a
/// stranger's reader takes the file for a QuickTime movie with both its streams.
#[test]
fn an_independent_reader_sees_a_quicktime_movie_when_one_is_installed() {
    use std::process::Command;
    if !Command::new("ffprobe")
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        eprintln!("no ffprobe here: the independent check was not run");
        return;
    }
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("film.mov");
    write_mov(&with_sound(24, 0, tone_wav(4000)), &path).expect("encoded");
    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format_tags=major_brand:stream=codec_name",
            "-of",
            "default=nw=1",
        ])
        .arg(&path)
        .output()
        .expect("probe");
    let said = String::from_utf8_lossy(&probe.stdout).into_owned();
    for wanted in ["major_brand=qt", "codec_name=h264", "codec_name=aac"] {
        assert!(said.contains(wanted), "{wanted}: {said}");
    }
}
