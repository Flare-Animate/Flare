//! What a packager asks of the program it has built, which must be answered without a display.
#![allow(clippy::expect_used)]

use std::process::Command;

fn run(argument: &str) -> (bool, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_flare-desktop"))
        .arg(argument)
        .output()
        .expect("run flare-desktop");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

#[test]
fn it_says_its_version_and_leaves() {
    for flag in ["--version", "-V"] {
        let (ok, text) = run(flag);
        assert!(ok, "{flag}");
        assert_eq!(
            text.trim(),
            format!("flare-desktop {}", env!("CARGO_PKG_VERSION"))
        );
    }
}

#[test]
fn it_says_how_to_use_it_and_leaves() {
    for flag in ["--help", "-h"] {
        let (ok, text) = run(flag);
        assert!(ok, "{flag}");
        assert!(text.contains("Usage: flare-desktop [FILE]..."), "{text}");
    }
}
