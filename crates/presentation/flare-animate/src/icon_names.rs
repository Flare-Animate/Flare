//! A typo in an icon name would show up only as a missing picture, so every quoted string in
//! this crate that looks like an icon name must name one.
#![allow(clippy::expect_used)]

use std::collections::HashSet;
use std::path::Path;

fn sources(dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).expect("read dir").flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") && !path.ends_with("icon_names.rs") {
            let text = std::fs::read_to_string(&path).expect("read source");
            out.push((path.display().to_string(), text));
        }
    }
}

#[test]
fn every_icon_name_written_in_the_ui_code_exists() {
    // Icon names are kebab-case strings that start like an icon's name ("tool-", "layer-"...).
    let families: HashSet<&str> = flare_ui::icons::names()
        .filter_map(|name| name.split('-').next())
        .collect();
    let mut files = Vec::new();
    sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    assert!(files.len() > 5, "found the sources");
    for (file, text) in files {
        for quoted in text.split('"').skip(1).step_by(2) {
            let looks_like_icon = quoted.contains('-')
                && quoted
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                && quoted
                    .split('-')
                    .next()
                    .is_some_and(|f| families.contains(f));
            if looks_like_icon {
                assert!(
                    flare_ui::icons::exists(quoted),
                    "{file} names an icon that does not exist: {quoted}"
                );
            }
        }
    }
}
