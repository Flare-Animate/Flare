//! Port of TFilePath standard-mode string logic (flare/sources/common/tsystem/tfilepath.cpp).
//! Seq numbers only for non-ffmpeg types; underscore format allowed (m_underscoreFormatAllowed).

const FFMPEG: [&str; 8] = ["gif", "mp4", "webm", "mov", "avi", "mkv", "m4v", "webp"];

fn last_slash(p: &str) -> Option<usize> {
    p.rfind(|c| c == '/' || c == '\\')
}
fn file(p: &str) -> &str {
    last_slash(p).map_or(p, |i| &p[i + 1..])
}

/// getType: extension (after last '.'), "" if none.
pub fn get_type(p: &str) -> String {
    let f = file(p);
    match f.rfind('.') {
        Some(i) if f != ".." => f[i + 1..].to_string(),
        _ => String::new(),
    }
}
pub fn is_ffmpeg_type(p: &str) -> bool {
    FFMPEG.contains(&get_type(p).to_lowercase().as_str())
}

/// (name, sep, digits) when file is a numbered frame, e.g. a.0001.png / a_0001.png.
fn seq(p: &str) -> Option<(String, char, String)> {
    if is_ffmpeg_type(p) {
        return None;
    }
    let f = file(p);
    let j = f.rfind('.')?;
    let stem = &f[..j];
    let i = stem.rfind('.').or_else(|| stem.rfind('_'))?;
    let d = &stem[i + 1..];
    if d.is_empty() || !d.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((
        stem[..i].to_string(),
        stem.as_bytes()[i] as char,
        d.to_string(),
    ))
}

pub fn get_name(p: &str) -> String {
    if let Some((n, _, _)) = seq(p) {
        return n;
    }
    let f = file(p);
    match f.rfind('.') {
        Some(i) if f != ".." => f[..i].to_string(),
        _ => f.to_string(),
    }
}
/// "pluto.0001.png" -> "pluto..png" (digits erased, separator kept).
pub fn get_level_name(p: &str) -> String {
    match seq(p) {
        Some((n, s, _)) => format!("{n}{s}.{}", get_type(p)),
        None => file(p).to_string(),
    }
}
pub fn get_dots(p: &str) -> String {
    if is_ffmpeg_type(p) {
        return ".".into();
    }
    if get_type(p).is_empty() {
        return String::new();
    }
    (if seq(p).is_some() { ".." } else { "." }).into()
}
pub fn get_frame(p: &str) -> i64 {
    seq(p).and_then(|(_, _, d)| d.parse().ok()).unwrap_or(-1)
}
pub fn get_parent_dir(p: &str) -> String {
    match last_slash(p) {
        None => {
            let b = p.as_bytes();
            if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
                p[..2].into()
            } else {
                String::new()
            }
        }
        Some(0) => "/".into(),
        Some(i) => p[..i].into(),
    }
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn basics() {
        assert_eq!(get_level_name("/pippo/pluto.0001.gif"), "pluto.0001.gif"); // ffmpeg type: no seq
        let p = "/pippo/pluto.0001.png";
        assert_eq!(get_level_name(p), "pluto..png");
        assert_eq!(
            (get_name(p).as_str(), get_frame(p), get_dots(p).as_str()),
            ("pluto", 1, "..")
        );
        assert_eq!(get_name("a_12.tif"), "a");
        assert_eq!(get_level_name("prova..tif"), "prova..tif");
        assert_eq!(get_parent_dir("C:\\x\\a.png"), "C:\\x");
        assert_eq!(get_parent_dir("C:a"), "C:");
        assert_eq!(get_parent_dir("/a"), "/");
        assert_eq!(
            (get_dots("a.png"), get_type("noext"), get_frame("a.png")),
            (".".to_string(), String::new(), -1)
        );
    }
}
