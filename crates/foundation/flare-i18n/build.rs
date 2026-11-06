//! Finds the message files, so adding one needs no change to the code: every `*.ftl` file in
//! `localizations/en/` is a section, and every other language must have the same file or the
//! build stops (a missing `include_str!` is the error), so no language can be left behind.

use std::error::Error;
use std::fmt::Write as _;
use std::path::PathBuf;

/// The folders of the languages, which are also their tags (`Language::code`).
const LANGUAGES: [&str; 11] = [
    "en", "de", "es", "fr", "it", "pt-BR", "cs", "ja", "ko", "ru", "zh-CN",
];

fn main() -> Result<(), Box<dyn Error>> {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?)
        .join("../../../localizations")
        .canonicalize()?;
    println!("cargo:rerun-if-changed={}", root.display());
    let mut names: Vec<String> = std::fs::read_dir(root.join("en"))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.strip_suffix(".ftl").map(str::to_owned)
        })
        .collect();
    names.sort();

    // Every key English defines, sorted, so a key can be checked for while the program is built.
    let mut keys: Vec<String> = Vec::new();
    for name in &names {
        let text = std::fs::read_to_string(root.join("en").join(format!("{name}.ftl")))?;
        for line in text.lines() {
            if let Some((key, _)) = line.split_once(" =")
                && !key.is_empty()
                && key
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                keys.push(key.to_owned());
            }
        }
    }
    keys.sort();
    keys.dedup();

    let mut out = String::new();
    writeln!(
        out,
        "/// Every message key English has, sorted by bytes.\npub const KEYS: [&str; {}] = {keys:?};\n",
        keys.len()
    )?;
    writeln!(
        out,
        "/// The sections of the interface that have files, in every language.\npub const SECTIONS: [&str; {}] = {names:?};\n",
        names.len()
    )?;
    writeln!(
        out,
        "fn language_sources(code: &str) -> [&'static str; SECTIONS.len()] {{\n    match code {{"
    )?;
    for language in LANGUAGES {
        write!(out, "        {language:?} => [")?;
        for name in &names {
            let path = root.join(language).join(format!("{name}.ftl"));
            write!(out, "include_str!({:?}), ", path.display().to_string())?;
        }
        writeln!(out, "],")?;
    }
    writeln!(out, "        _ => language_sources(\"en\"),\n    }}\n}}")?;
    std::fs::write(
        PathBuf::from(std::env::var("OUT_DIR")?).join("sections.rs"),
        out,
    )?;
    Ok(())
}
