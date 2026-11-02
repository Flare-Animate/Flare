//! The Test Movie mechanism, exercised with a stand-in player (a shell script that records
//! what it was given). Real Ruffle has not been run.
#![allow(clippy::expect_used)]
#![cfg(unix)]

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{Content, Document, FrameSpan, Layer, Scene, Shape, Timeline};
use flare_ruffle::{TestMovieError, find_ruffle_with, launch_test_movie};

fn document() -> Document {
    let shape = Shape::rect(
        ShapeId::new(),
        0.0,
        0.0,
        10.0,
        10.0,
        Rgba8::opaque(255, 0, 0),
    )
    .expect("rect");
    let span = FrameSpan::new(
        0,
        std::num::NonZeroU32::new(3).expect("nonzero"),
        Content::Shape(shape),
    );
    let layer = Layer::new(LayerId::new(), "L", vec![span]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    let rate = FrameRate::new(24, 1).expect("rate");
    Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("doc")
}

/// A player script in `dir` that records its arguments and a copy of the SWF next to itself,
/// then exits with `code`.
fn recorder(dir: &Path, code: i32) -> PathBuf {
    let path = dir.join("ruffle");
    let script = format!(
        "#!/bin/sh\nhere=$(dirname \"$0\")\nprintf '%s\\n' \"$#\" \"$1\" \"$(pwd -P)\" > \"$here/args.txt\"\ncp \"$1\" \"$here/copy.swf\"\nexit {code}\n"
    );
    fs::write(&path, script).expect("write script");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

#[test]
fn the_player_gets_one_argument_a_valid_swf_and_its_exit_status_comes_back() {
    let dir = tempfile::tempdir().expect("dir");
    let player = recorder(dir.path(), 3);
    let movie = launch_test_movie(&document(), &player).expect("launch");
    let swf = movie.swf_path().to_owned();
    assert!(swf.ends_with("movie.swf"));
    // Resolve symlinks now: the directory is gone once the movie ends.
    let physical_dir = swf
        .parent()
        .expect("dir")
        .canonicalize()
        .expect("dir exists while running");
    let status = movie.wait().expect("wait");
    assert_eq!(
        status.code(),
        Some(3),
        "the player's exit status is reported"
    );

    let args = fs::read_to_string(dir.path().join("args.txt")).expect("args");
    let lines: Vec<&str> = args.lines().collect();
    assert_eq!(lines[0], "1", "exactly one argument");
    assert_eq!(Path::new(lines[1]), swf);
    assert_eq!(
        Path::new(lines[2]),
        physical_dir,
        "the player starts inside the private directory"
    );

    // The file the player saw is a real SWF.
    let copy = fs::read(dir.path().join("copy.swf")).expect("copy");
    let buf = swf::decompress_swf(&copy[..]).expect("decompress");
    let parsed = swf::parse_swf(&buf).expect("the SWF the player received parses");
    assert_eq!(parsed.header.num_frames(), 3);

    assert!(
        !swf.exists(),
        "the temporary files are removed once the movie ends"
    );
    assert!(!swf.parent().expect("dir").exists());
}

#[test]
fn nothing_is_interpreted_by_a_shell() {
    // A directory name full of shell syntax: it must be used literally.
    let dir = tempfile::tempdir().expect("dir");
    let nasty = dir
        .path()
        .join("a b $(touch pwned) ; `touch pwned2` && 'x'");
    fs::create_dir(&nasty).expect("mkdir");
    let player = recorder(&nasty, 0);
    launch_test_movie(&document(), &player)
        .expect("launch")
        .wait()
        .expect("wait");
    assert!(
        nasty.join("copy.swf").is_file(),
        "the player ran despite the awkward path"
    );
    for evidence in [
        dir.path().join("pwned"),
        nasty.join("pwned"),
        dir.path().join("pwned2"),
        nasty.join("pwned2"),
    ] {
        assert!(
            !evidence.exists(),
            "{evidence:?}: a shell executed part of the path"
        );
    }
}

#[test]
fn unusable_players_are_refused_before_anything_is_published() {
    let dir = tempfile::tempdir().expect("dir");
    let missing = dir.path().join("nope");
    assert!(matches!(
        launch_test_movie(&document(), &missing),
        Err(TestMovieError::NotExecutable(_))
    ));
    assert!(
        matches!(
            launch_test_movie(&document(), dir.path()),
            Err(TestMovieError::NotExecutable(_))
        ),
        "a directory"
    );
    let plain = dir.path().join("plain");
    fs::write(&plain, "x").expect("write");
    fs::set_permissions(&plain, fs::Permissions::from_mode(0o644)).expect("chmod");
    assert!(
        matches!(
            launch_test_movie(&document(), &plain),
            Err(TestMovieError::NotExecutable(_))
        ),
        "no execute bit"
    );
}

#[test]
fn a_running_player_can_be_stopped_and_dropping_stops_it_and_cleans_up() {
    let dir = tempfile::tempdir().expect("dir");
    let player = dir.path().join("slow");
    fs::write(&player, "#!/bin/sh\nsleep 30\n").expect("write");
    fs::set_permissions(&player, fs::Permissions::from_mode(0o755)).expect("chmod");

    let mut movie = launch_test_movie(&document(), &player).expect("launch");
    assert!(movie.try_wait().expect("poll").is_none(), "still running");
    movie.kill().expect("kill");
    assert!(movie.try_wait().expect("poll").is_some(), "stopped");
    movie.kill().expect("killing twice is fine");

    let started = Instant::now();
    let movie = launch_test_movie(&document(), &player).expect("launch again");
    let swf = movie.swf_path().to_owned();
    drop(movie);
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "dropping did not wait for the player"
    );
    assert!(!swf.exists(), "dropping removes the files");
}

fn env_with<'a>(pairs: &'a [(&'a str, OsString)]) -> impl Fn(&str) -> Option<OsString> + 'a {
    move |name| {
        pairs
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v.clone())
    }
}

#[test]
fn ruffle_is_found_through_the_explicit_variable_then_the_path() {
    let dir = tempfile::tempdir().expect("dir");
    let bin = dir.path().join("bin");
    fs::create_dir(&bin).expect("mkdir");
    let on_path = recorder(&bin, 0);
    let elsewhere = recorder(dir.path(), 0);

    let search = std::env::join_paths([bin.clone()]).expect("path");
    assert_eq!(
        find_ruffle_with(env_with(&[("PATH", search.clone())])),
        Some(on_path.clone())
    );
    // The explicit variable wins, and is honoured only if it names an executable file.
    let both = [
        ("PATH", search.clone()),
        ("FLARE_RUFFLE", elsewhere.clone().into_os_string()),
    ];
    assert_eq!(find_ruffle_with(env_with(&both)), Some(elsewhere));
    let bogus = [
        ("PATH", search),
        ("FLARE_RUFFLE", dir.path().join("missing").into_os_string()),
    ];
    assert_eq!(
        find_ruffle_with(env_with(&bogus)),
        None,
        "a bad explicit path is not silently replaced"
    );
    assert_eq!(find_ruffle_with(env_with(&[])), None);
}

#[test]
fn only_executable_files_on_absolute_path_entries_count() {
    let dir = tempfile::tempdir().expect("dir");
    let plain = dir.path().join("ruffle");
    fs::write(&plain, "x").expect("write");
    fs::set_permissions(&plain, fs::Permissions::from_mode(0o644)).expect("chmod");
    let search = std::env::join_paths([dir.path()]).expect("path");
    assert_eq!(
        find_ruffle_with(env_with(&[("PATH", search)])),
        None,
        "not executable"
    );
    // A relative entry (such as the current directory) is never searched.
    assert_eq!(
        find_ruffle_with(env_with(&[("PATH", OsString::from(".:bin"))])),
        None
    );
}
