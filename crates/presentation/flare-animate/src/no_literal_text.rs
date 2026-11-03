//! A tooltip or a message that is not drawn is not seen by the drawing test, so this reads the
//! source: a widget or a message given a plain English string, instead of one from a message
//! file, is a word that will not be translated.
#![allow(clippy::expect_used)]

use std::path::Path;

/// The calls that put their string argument in front of a person.
const SHOWING: &[&str] = &[
    ".on_hover_text(",
    ".label(",
    ".heading(",
    ".button(",
    ".small_button(",
    ".selectable_label(",
    ".checkbox(",
    ".menu_button(",
    ".collapsing(",
    ".radio_value(",
    ".report(",
    "RichText::new(",
    "egui::Window::new(",
    "icon_button(",
];

/// Strings that are not words of the interface.
fn is_kept(text: &str) -> bool {
    let letters = text.chars().filter(|c| c.is_alphabetic()).count();
    letters < 2
        || matches!(
            text,
            "Flare" | "px" | "fps" | "KB" | "MB" | "GB" | "Hex" | "XFL" | "FLA" | "SWF"
        )
        || text.starts_with("http")
        // An escape such as \u{25cf} is a symbol, and its source text has letters in it.
        || text.starts_with("\\u{")
}

fn sources(dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).expect("read dir").flatten() {
        let path = entry.path();
        let name = path.display().to_string();
        if path.is_dir() {
            if !name.contains("/tests") && !name.contains("\\tests") {
                sources(&path, out);
            }
        } else if name.ends_with(".rs")
            && !name.ends_with("no_literal_text.rs")
            && !name.ends_with("_tests.rs")
            && !name.ends_with("tests.rs")
        {
            out.push((name, std::fs::read_to_string(&path).expect("read source")));
        }
    }
}

#[test]
fn no_widget_or_message_is_given_a_plain_english_string() {
    let mut files = Vec::new();
    sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    assert!(files.len() > 20, "found the sources");
    let mut found = Vec::new();
    for (file, text) in files {
        // Comments are cut out first; a call may put its argument on the next line.
        let code: String = text
            .lines()
            .map(|line| line.split("//").next().unwrap_or(line))
            .collect::<Vec<_>>()
            .join("\n");
        for call in SHOWING {
            let mut from = 0;
            while let Some(at) = code[from..].find(call) {
                let start = from + at + call.len();
                from = start;
                let argument = code[start..].trim_start();
                // Only a literal as the first argument: a string held in a variable or built
                // by `self.tr` is fine.
                if let Some(inner) = argument.strip_prefix('"')
                    && let Some(end) = inner.find('"')
                    && !is_kept(&inner[..end])
                {
                    let line = code[..start].matches('\n').count() + 1;
                    found.push(format!("{file}:{line}: {call}\"{}\"", &inner[..end]));
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "{} plain English strings are given to the interface:\n{}",
        found.len(),
        found.join("\n")
    );
}
