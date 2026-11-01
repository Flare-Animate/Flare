use super::*;

#[cfg(unix)]
#[test]
fn open_starts_the_app_with_the_projects_as_absolute_paths() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("dir");
    let out = dir.path().join("args.txt");
    let app = dir.path().join("app.sh");
    std::fs::write(
        &app,
        format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\n", out.display()),
    )
    .expect("script");
    std::fs::set_permissions(&app, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let output = Command::new(env!("CARGO_BIN_EXE_flare"))
        .args(["open", "one.flare", "two.fla"])
        .env("FLARE_DESKTOP", &app)
        .current_dir(dir.path())
        .output()
        .expect("run cli");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let reply: Value = serde_json::from_slice(&output.stdout).expect("json");
    assert_eq!(reply["opened"], 2);
    for _ in 0..200 {
        if std::fs::read_to_string(&out).is_ok_and(|s| s.lines().count() == 2) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    let seen = std::fs::read_to_string(&out).expect("the app ran");
    let lines: Vec<&str> = seen.lines().collect();
    assert!(lines[0].ends_with("one.flare") && std::path::Path::new(lines[0]).is_absolute());
    assert!(lines[1].ends_with("two.fla"));
}

#[test]
fn open_names_the_problem_when_the_app_cannot_be_found() {
    let output = Command::new(env!("CARGO_BIN_EXE_flare"))
        .arg("open")
        .env("FLARE_DESKTOP", "/nonexistent/flare-desktop")
        .output()
        .expect("run cli");
    assert!(!output.status.success());
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(message.contains("FLARE_DESKTOP"), "{message}");
}
