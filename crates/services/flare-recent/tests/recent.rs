#![allow(clippy::expect_used)]

use flare_recent::{FileInfo, RecentInfo, SortBy, Sorting, ago, format_size, kind, matches, sort};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

fn listed(names: &[&str]) -> Vec<PathBuf> {
    names
        .iter()
        .map(|n| PathBuf::from(format!("/p/{n}")))
        .collect()
}

fn order(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|p| p.file_name().expect("name").to_string_lossy().into_owned())
        .collect()
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
fn modified_times_read_as_how_long_ago_up_through_months_and_years() {
    let now = at(1_800_000_000);
    let since = |seconds: u64| ago(at(1_800_000_000 - seconds), now);
    assert_eq!(since(5), "just now");
    assert_eq!(since(60), "1 minute ago");
    assert_eq!(since(2 * 3600), "2 hours ago");
    assert_eq!(since(86_400), "1 day ago");
    assert_eq!(since(3 * 86_400), "3 days ago");
    assert_eq!(since(29 * 86_400), "29 days ago");
    assert_eq!(since(30 * 86_400), "1 month ago");
    assert_eq!(since(364 * 86_400), "12 months ago");
    assert_eq!(since(365 * 86_400), "1 year ago");
    assert_eq!(since(3 * 365 * 86_400 + 5), "3 years ago");
    assert_eq!(ago(at(0), now), "57 years ago");
}

#[test]
fn a_time_in_the_future_reads_as_just_now() {
    assert_eq!(ago(at(9_000_000), at(1_000)), "just now");
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
    let gone = FileInfo::read(&dir.path().join("none.flare"));
    assert_eq!(gone, FileInfo::default());
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

#[test]
fn the_list_sorts_by_name_kind_date_and_size_in_both_directions() {
    let info = RecentInfo::default();
    let facts = [("b.flare", 20, 300), ("A.fla", 30, 100), ("c.xfl", 10, 200)];
    for (name, when, size) in facts {
        info.record(
            &PathBuf::from(format!("/p/{name}")),
            FileInfo {
                modified: Some(at(when)),
                size: Some(size),
            },
        );
    }
    let sorted = |by, descending| {
        let mut paths = listed(&["b.flare", "A.fla", "c.xfl"]);
        sort(&mut paths, Sorting { by, descending }, &info);
        order(&paths)
    };
    assert_eq!(sorted(SortBy::Opened, true), ["b.flare", "A.fla", "c.xfl"]);
    assert_eq!(sorted(SortBy::Name, false), ["A.fla", "b.flare", "c.xfl"]);
    assert_eq!(sorted(SortBy::Name, true), ["c.xfl", "b.flare", "A.fla"]);
    assert_eq!(
        sorted(SortBy::Modified, false),
        ["c.xfl", "b.flare", "A.fla"]
    );
    assert_eq!(
        sorted(SortBy::Modified, true),
        ["A.fla", "b.flare", "c.xfl"]
    );
    assert_eq!(sorted(SortBy::Size, false), ["A.fla", "c.xfl", "b.flare"]);
    assert_eq!(sorted(SortBy::Kind, false), ["A.fla", "c.xfl", "b.flare"]);
}

#[test]
fn what_is_not_known_yet_sorts_last_whichever_way_the_column_runs() {
    let info = RecentInfo::default();
    info.record(
        &PathBuf::from("/p/known.flare"),
        FileInfo {
            modified: Some(at(5)),
            size: Some(9),
        },
    );
    for descending in [false, true] {
        let mut paths = listed(&["pending.flare", "known.flare"]);
        let sorting = Sorting {
            by: SortBy::Size,
            descending,
        };
        sort(&mut paths, sorting, &info);
        assert_eq!(order(&paths), ["known.flare", "pending.flare"]);
    }
}

#[test]
fn clicking_a_heading_goes_up_then_down_then_back_to_the_order_opened() {
    let first = Sorting::default().click(SortBy::Size);
    assert_eq!((first.by, first.descending), (SortBy::Size, false));
    let second = first.click(SortBy::Size);
    assert_eq!((second.by, second.descending), (SortBy::Size, true));
    assert_eq!(second.click(SortBy::Size), Sorting::default());
    // Another heading starts over from the top.
    let other = second.click(SortBy::Name);
    assert_eq!((other.by, other.descending), (SortBy::Name, false));
}

#[test]
fn files_are_named_by_kind() {
    assert_eq!(kind(Path::new("a.FLARE")), "Flare project");
    assert_eq!(kind(Path::new("a.fla")), "Animate document");
    assert_eq!(kind(Path::new("a.xfl")), "Animate project");
    assert_eq!(kind(Path::new("a")), "File");
}
