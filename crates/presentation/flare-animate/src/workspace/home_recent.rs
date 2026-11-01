//! What the Home page knows about each recent project besides its name: when the file was last
//! changed and how big it is, found off the UI thread (a project on a network drive can take
//! seconds to answer), and the search over the list.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

/// What is known of one file. `None` for both is a file that is not there any more.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct FileInfo {
    pub(super) modified: Option<SystemTime>,
    pub(super) size: Option<u64>,
}

impl FileInfo {
    pub(super) fn read(path: &Path) -> Self {
        match std::fs::metadata(path) {
            Ok(meta) => Self {
                modified: meta.modified().ok(),
                size: Some(meta.len()),
            },
            Err(_) => Self::default(),
        }
    }
}

/// Answers gathered by background threads, shared with the page that asked.
#[derive(Clone, Default)]
pub(super) struct RecentInfo {
    known: Arc<Mutex<HashMap<PathBuf, FileInfo>>>,
    asked: HashSet<PathBuf>,
}

impl RecentInfo {
    pub(super) fn get(&self, path: &Path) -> Option<FileInfo> {
        self.known.lock().ok()?.get(path).copied()
    }

    /// Reads every path not asked about yet on a thread of its own, and asks `wake` for another
    /// frame when it is done. Returns the thread so a test can wait for it.
    pub(super) fn ask(
        &mut self,
        paths: &[PathBuf],
        wake: impl FnOnce() + Send + 'static,
    ) -> Option<std::thread::JoinHandle<()>> {
        let fresh: Vec<PathBuf> = paths
            .iter()
            .filter(|p| self.asked.insert((*p).clone()))
            .cloned()
            .collect();
        if fresh.is_empty() {
            return None;
        }
        let known = Arc::clone(&self.known);
        std::thread::Builder::new()
            .name("flare-recent-info".into())
            .spawn(move || {
                for path in fresh {
                    let info = FileInfo::read(&path);
                    if let Ok(mut known) = known.lock() {
                        known.insert(path, info);
                    }
                }
                wake();
            })
            .ok()
    }

    /// Counts `paths` as asked about without reading them, so a test can look at the page while
    /// nothing is known yet; a real thread might answer before the page is drawn.
    #[cfg(test)]
    pub(super) fn pretend_asked(&mut self, paths: &[PathBuf]) {
        self.asked.extend(paths.iter().cloned());
    }

    /// Sets what is known of `path`, so a test can sort on facts a file system would not give it.
    #[cfg(test)]
    pub(super) fn set(&self, path: &Path, info: FileInfo) {
        if let Ok(mut known) = self.known.lock() {
            known.insert(path.to_owned(), info);
        }
    }

    /// Forgets what was read, so the next look at the list reads it again.
    pub(super) fn refresh(&mut self) {
        self.asked.clear();
        if let Ok(mut known) = self.known.lock() {
            known.clear();
        }
    }
}

/// Whether a project whose file is called `name`, in `folder`, answers `query`: every word of
/// the query is somewhere in the name or the folder, in any case. An empty query matches all.
pub(super) fn matches(name: &str, folder: &str, query: &str) -> bool {
    let haystack = format!("{name} {folder}").to_lowercase();
    query
        .split_whitespace()
        .all(|word| haystack.contains(&word.to_lowercase()))
}

pub(super) fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// How long ago `then` was, as "5 minutes ago" and on up through days, months and years. It is
/// never a date, so no time zone comes into it. A time in the future (a clock that was set back)
/// reads as just now.
pub(super) fn ago(then: SystemTime, now: SystemTime) -> String {
    const DAY: u64 = 86_400;
    let ago = now.duration_since(then).unwrap_or(Duration::ZERO).as_secs();
    let plural = |n: u64, unit: &str| format!("{n} {unit}{}", if n == 1 { "" } else { "s" });
    let text = match ago {
        0..=59 => return "just now".to_owned(),
        60..=3599 => plural(ago / 60, "minute"),
        3600..=86_399 => plural(ago / 3600, "hour"),
        86_400..=2_591_999 => plural(ago / DAY, "day"),
        2_592_000..=31_535_999 => plural(ago / (30 * DAY), "month"),
        _ => plural(ago / (365 * DAY), "year"),
    };
    format!("{text} ago")
}

/// What the recent list is ordered by. `Opened` is the order the projects were last opened or
/// saved in, which is the order the list is kept in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum SortBy {
    #[default]
    Opened,
    Name,
    Modified,
    Size,
    Kind,
}

/// The order of the recent list: by what, and whether it runs from the largest or the last.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Sorting {
    pub(super) by: SortBy,
    pub(super) descending: bool,
}

impl Sorting {
    /// What clicking the heading of `column` does: that column ascending, then descending, then
    /// back to the order the projects were opened in.
    pub(super) fn click(self, column: SortBy) -> Self {
        match (self.by == column, self.descending) {
            (false, _) => Self {
                by: column,
                descending: false,
            },
            (true, false) => Self {
                by: column,
                descending: true,
            },
            (true, true) => Self::default(),
        }
    }
}

/// What kind of file a recent project is, going by its name.
pub(super) fn kind(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("flare") => "Flare project",
        Some("fla") => "Animate document",
        Some("xfl") => "Animate project",
        _ => "File",
    }
}

/// Puts `paths` in the order `sorting` asks for. A file whose size or time is not known (still
/// being read, or gone) goes last whichever way the column runs, and ties keep the order the
/// projects were opened in.
pub(super) fn sort(paths: &mut [PathBuf], sorting: Sorting, info: &RecentInfo) {
    use std::cmp::Ordering;
    let name = |p: &Path| {
        p.file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    };
    let order = |ordering: Ordering| {
        if sorting.descending {
            ordering.reverse()
        } else {
            ordering
        }
    };
    // Unknown values are compared outside the direction, so they never come first.
    fn known<T: Ord>(a: Option<T>, b: Option<T>, order: impl Fn(Ordering) -> Ordering) -> Ordering {
        match (a, b) {
            (Some(a), Some(b)) => order(a.cmp(&b)),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
    }
    match sorting.by {
        SortBy::Opened => {}
        SortBy::Name => paths.sort_by(|a, b| order(name(a).cmp(&name(b)))),
        SortBy::Kind => paths
            .sort_by(|a, b| order(kind(a).cmp(kind(b))).then_with(|| order(name(a).cmp(&name(b))))),
        SortBy::Modified => paths.sort_by(|a, b| {
            known(
                info.get(a).and_then(|i| i.modified),
                info.get(b).and_then(|i| i.modified),
                order,
            )
        }),
        SortBy::Size => paths.sort_by(|a, b| {
            known(
                info.get(a).and_then(|i| i.size),
                info.get(b).and_then(|i| i.size),
                order,
            )
        }),
    }
}
