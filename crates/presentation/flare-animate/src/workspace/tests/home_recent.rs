//! The modified time, size and search on the Home page's recent projects.

use super::*;
use crate::workspace::home_recent::{FileInfo, RecentInfo, format_modified, format_size, matches};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

#[test]
fn sizes_read_in_the_unit_that_fits() {
    assert_eq!(format_size(0), "0 B");
    assert_eq!(format_size(1023), "1023 B");
    assert_eq!(format_size(1024), "1.0 KB");
    assert_eq!(format_size(1536), "1.5 KB");
    assert_eq!(format_size(5 * 1024 * 1024), "5.0 MB");
    assert_eq!(format_size(300 * 1024 * 1024), "300 MB");
    assert_eq!(format_size(u64::MAX), "16777216 TB");
}

#[test]
fn modified_times_read_as_how_long_ago_and_then_as_a_date() {
    let now = at(1_800_000_000);
    let ago = |seconds: u64| format_modified(at(1_800_000_000 - seconds), now);
    assert_eq!(ago(5), "Modified just now");
    assert_eq!(ago(60), "Modified 1 minute ago");
    assert_eq!(ago(2 * 3600), "Modified 2 hours ago");
    assert_eq!(ago(3 * 86_400), "Modified 3 days ago");
    // 2027-01-15 08:00:00 UTC is 1_800_000_000; a month before it:
    assert_eq!(ago(30 * 86_400), "Modified 16 Dec 2026");
    assert_eq!(format_modified(at(0), now), "Modified 1 Jan 1970");
    assert_eq!(
        format_modified(at(951_782_400), now),
        "Modified 29 Feb 2000",
        "a leap day"
    );
}

#[test]
fn a_time_in_the_future_reads_as_just_now() {
    let now = at(1_000);
    assert_eq!(format_modified(at(9_000_000), now), "Modified just now");
}

#[test]
fn search_needs_every_word_in_the_name_or_the_folder_in_any_case() {
    assert!(matches("Test1.flare", "/Users/me/Downloads", ""));
    assert!(matches("Test1.flare", "/Users/me/Downloads", "test"));
    assert!(matches(
        "Test1.flare",
        "/Users/me/Downloads",
        "DOWNLOADS test1"
    ));
    assert!(!matches("Test1.flare", "/Users/me/Downloads", "test2"));
    assert!(!matches(
        "Test1.flare",
        "/Users/me/Downloads",
        "test nothing"
    ));
    assert!(matches("Test1.flare", "/Users/me/Downloads", "   "));
}

#[test]
fn a_file_is_read_for_its_size_and_a_missing_one_says_so() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    std::fs::write(&path, vec![0u8; 2048]).expect("write");
    let info = FileInfo::read(&path);
    assert_eq!(info.size, Some(2048));
    assert!(info.modified.is_some());
    assert!(info.describe(SystemTime::now()).ends_with("2.0 KB"));
    let gone = FileInfo::read(&dir.path().join("none.flare"));
    assert_eq!(gone, FileInfo::default());
    assert_eq!(gone.describe(SystemTime::now()), "Not found");
}

#[test]
fn files_are_read_once_off_the_calling_thread_and_again_after_a_refresh() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    std::fs::write(&path, b"x").expect("write");
    let mut info = RecentInfo::default();
    assert_eq!(info.get(&path), None, "nothing is known before the answer");
    let paths = vec![path.clone()];
    let worker = info.ask(&paths, || {}).expect("a thread was started");
    worker.join().expect("thread");
    assert_eq!(info.get(&path).and_then(|i| i.size), Some(1));
    assert!(info.ask(&paths, || {}).is_none(), "asked about only once");
    info.refresh();
    assert_eq!(info.get(&path), None);
    info.ask(&paths, || {})
        .expect("again")
        .join()
        .expect("thread");
    assert!(info.get(&path).is_some());
}

/// All the text a pass drew.
fn texts(ws: &mut Workspace, ctx: &egui::Context, time: f64) -> Vec<String> {
    let mut out = ctx.run_ui(input(time, vec![]), |ui| ws.show(ui));
    out.textures_delta.clear();
    out.shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => Some(t.galley.text().to_owned()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_cards_show_when_the_file_changed_and_how_big_it_is() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("Real.flare");
    std::fs::write(&path, vec![0u8; 3000]).expect("write");
    let mut ws = workspace();
    ws.note_recent(&path);
    ws.note_recent(&dir.path().join("Missing.flare"));
    ws.set_home(true);
    let ctx = egui::Context::default();
    assert!(
        texts(&mut ws, &ctx, 0.0).iter().any(|t| t == "Reading…"),
        "before the answer the card says it is reading"
    );
    let recent = ws.recent_files().to_vec();
    assert!(
        ws.recent_info.ask(&recent, || {}).is_none(),
        "the page already asked"
    );
    for _ in 0..200 {
        if ws.recent_info.get(&path).is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let drawn = texts(&mut ws, &ctx, 0.1);
    assert!(
        drawn
            .iter()
            .any(|t| t.contains("Modified") && t.contains("2.9 KB")),
        "{drawn:?}"
    );
    assert!(drawn.iter().any(|t| t == "Not found"), "{drawn:?}");
}

#[test]
fn the_search_narrows_the_cards_and_says_when_nothing_matches() {
    let mut ws = workspace();
    ws.note_recent(std::path::Path::new("/a/walk-cycle.flare"));
    ws.note_recent(std::path::Path::new("/a/logo.flare"));
    ws.set_home(true);
    let ctx = egui::Context::default();
    let all = texts(&mut ws, &ctx, 0.0);
    assert!(all.iter().any(|t| t == "walk-cycle.flare") && all.iter().any(|t| t == "logo.flare"));
    ws.home_search = "WALK".to_owned();
    let some = texts(&mut ws, &ctx, 0.1);
    assert!(some.iter().any(|t| t == "walk-cycle.flare"));
    assert!(!some.iter().any(|t| t == "logo.flare"));
    ws.home_search = "zzz".to_owned();
    let none = texts(&mut ws, &ctx, 0.2);
    assert!(
        none.iter().any(|t| t.contains("No recent project matches")),
        "{none:?}"
    );
    assert_eq!(ws.recent_files().len(), 2, "searching forgets nothing");
}
