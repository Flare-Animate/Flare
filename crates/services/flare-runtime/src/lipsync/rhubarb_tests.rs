#![allow(clippy::expect_used)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::rhubarb::choose_rhubarb;

const NAME: &str = if cfg!(windows) {
    "rhubarb.exe"
} else {
    "rhubarb"
};

fn program(dir: &Path) -> PathBuf {
    let path = dir.join(NAME);
    std::fs::write(&path, b"").expect("write");
    path
}

fn path_of(dirs: &[&Path]) -> Option<OsString> {
    Some(std::env::join_paths(dirs).expect("join"))
}

#[test]
fn the_program_named_in_the_variable_runs_ahead_of_the_others() {
    let (named, on_path) = (
        tempfile::tempdir().expect("dir"),
        tempfile::tempdir().expect("dir"),
    );
    let chosen = program(named.path());
    program(on_path.path());
    let found = choose_rhubarb(
        Some(chosen.clone().into()),
        None,
        path_of(&[on_path.path()]),
    );
    assert_eq!(found, Some(chosen));
}

#[test]
fn a_named_program_that_is_not_a_file_finds_none_rather_than_one_on_the_path() {
    let (named, on_path) = (
        tempfile::tempdir().expect("dir"),
        tempfile::tempdir().expect("dir"),
    );
    program(on_path.path());
    for missing in [named.path().to_path_buf(), named.path().join("gone")] {
        let found = choose_rhubarb(Some(missing.into()), None, path_of(&[on_path.path()]));
        assert_eq!(found, None);
    }
}

#[test]
fn with_nothing_named_rhubarb_beside_flare_comes_before_the_path() {
    let (beside, on_path) = (
        tempfile::tempdir().expect("dir"),
        tempfile::tempdir().expect("dir"),
    );
    let on_path_program = program(on_path.path());
    assert_eq!(
        choose_rhubarb(None, Some(beside.path()), path_of(&[on_path.path()])),
        Some(on_path_program.clone())
    );
    // An empty variable names nothing.
    assert_eq!(
        choose_rhubarb(Some(OsString::new()), None, path_of(&[on_path.path()])),
        Some(on_path_program)
    );
    // As its release archive unpacks, in a `rhubarb` folder beside Flare.
    let in_folder = beside.path().join("rhubarb");
    std::fs::create_dir(&in_folder).expect("dir");
    let unpacked = program(&in_folder);
    assert_eq!(
        choose_rhubarb(None, Some(beside.path()), path_of(&[on_path.path()])),
        Some(unpacked)
    );
    // Or right next to it.
    let other = tempfile::tempdir().expect("dir");
    let next_to = program(other.path());
    assert_eq!(
        choose_rhubarb(None, Some(other.path()), path_of(&[on_path.path()])),
        Some(next_to)
    );
}

#[test]
fn with_nothing_anywhere_there_is_no_rhubarb() {
    let empty = tempfile::tempdir().expect("dir");
    assert_eq!(
        choose_rhubarb(None, Some(empty.path()), path_of(&[empty.path()])),
        None
    );
}
