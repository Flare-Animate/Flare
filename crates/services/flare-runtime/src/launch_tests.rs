#![allow(clippy::expect_used)]

use super::*;

#[cfg(unix)]
fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

fn no_env(_: &str) -> Option<OsString> {
    None
}

#[cfg(unix)]
#[test]
fn explicit_variable_wins_and_must_be_executable() {
    let dir = tempfile::tempdir().expect("dir");
    let app = script(dir.path(), "chosen", "exit 0");
    let found = find_desktop_with(
        |n| (n == "FLARE_DESKTOP").then(|| app.clone().into_os_string()),
        None,
    );
    assert_eq!(found, Some(app));
    let plain = dir.path().join("plain");
    std::fs::write(&plain, "x").expect("write");
    let refused = find_desktop_with(
        |n| (n == "FLARE_DESKTOP").then(|| plain.clone().into_os_string()),
        None,
    );
    assert_eq!(
        refused, None,
        "a named file that cannot run is not guessed around"
    );
}

#[cfg(unix)]
#[test]
fn the_app_next_to_the_running_program_is_found() {
    let dir = tempfile::tempdir().expect("dir");
    let app = script(dir.path(), APP, "exit 0");
    let cli = dir.path().join("flare");
    assert_eq!(
        find_in(no_env, Some(&cli), Path::new("/nonexistent")),
        Some(app)
    );
}

#[cfg(unix)]
#[test]
fn path_is_searched_but_relative_entries_are_not() {
    let dir = tempfile::tempdir().expect("dir");
    let app = script(dir.path(), APP, "exit 0");
    let path = std::env::join_paths([Path::new("relative"), dir.path()]).expect("join");
    let found = find_in(
        |n| (n == "PATH").then(|| path.clone()),
        None,
        Path::new("/nonexistent"),
    );
    assert_eq!(found, Some(app));
}

#[test]
fn nothing_found_is_none() {
    assert_eq!(find_in(no_env, None, Path::new("/nonexistent")), None);
}

#[cfg(unix)]
#[test]
fn the_files_reach_the_app_as_absolute_paths() {
    let dir = tempfile::tempdir().expect("dir");
    let out = dir.path().join("args.txt");
    let app = script(
        dir.path(),
        "app",
        &format!("printf '%s\\n' \"$@\" > '{}'", out.display()),
    );
    open_desktop(&app, &[PathBuf::from("a.flare"), dir.path().join("b.fla")]).expect("start");
    for _ in 0..200 {
        if std::fs::read_to_string(&out).is_ok_and(|s| s.lines().count() == 2) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    let seen = std::fs::read_to_string(&out).expect("the app ran");
    let lines: Vec<&str> = seen.lines().collect();
    assert!(Path::new(lines[0]).is_absolute() && lines[0].ends_with("a.flare"));
    assert_eq!(lines[1], dir.path().join("b.fla").to_string_lossy());
}

#[test]
fn too_many_files_and_a_missing_app_are_refused() {
    let files = vec![PathBuf::from("x.flare"); MAX_OPEN_FILES + 1];
    let err = open_desktop(Path::new("/nope"), &files).expect_err("too many");
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    let err = open_desktop(Path::new("/nonexistent/flare-desktop"), &[]).expect_err("missing");
    assert_eq!(err.kind(), io::ErrorKind::NotFound);
}

#[cfg(target_os = "macos")]
#[test]
fn the_installed_app_bundle_is_found_by_the_name_of_its_program() {
    let dir = tempfile::tempdir().expect("dir");
    let macos = dir.path().join("Flare.app/Contents/MacOS");
    std::fs::create_dir_all(&macos).expect("dirs");
    // A bundle with only a loose-style program name is not how Flare is packaged.
    script(&macos, APP, "exit 0");
    assert_eq!(find_in(no_env, None, dir.path()), None);
    let app = script(&macos, "flare", "exit 0");
    assert_eq!(find_in(no_env, None, dir.path()), Some(app));
}
