//! What the Home page knows about each recent project besides its name: when the file was last
//! changed and how big it is, found off the UI thread (a project on a network drive can take
//! seconds to answer), and the search over the list.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

    /// "Modified 2 hours ago · 1.4 MB", or what is missing.
    pub(super) fn describe(&self, now: SystemTime) -> String {
        match (self.modified, self.size) {
            (None, None) => "Not found".to_owned(),
            (Some(then), Some(size)) => {
                format!("{} · {}", format_modified(then, now), format_size(size))
            }
            (Some(then), None) => format_modified(then, now),
            (None, Some(size)) => format_size(size),
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

/// How long ago `then` was as "Modified 5 minutes ago", and as a date once it is a week old.
/// A time in the future (a clock that was set back) reads as just now.
pub(super) fn format_modified(then: SystemTime, now: SystemTime) -> String {
    let ago = now.duration_since(then).unwrap_or(Duration::ZERO).as_secs();
    let plural = |n: u64, unit: &str| format!("{n} {unit}{}", if n == 1 { "" } else { "s" });
    let text = match ago {
        0..=59 => "just now".to_owned(),
        60..=3599 => format!("{} ago", plural(ago / 60, "minute")),
        3600..=86_399 => format!("{} ago", plural(ago / 3600, "hour")),
        86_400..=604_799 => format!("{} ago", plural(ago / 86_400, "day")),
        _ => return format!("Modified {}", date(then)),
    };
    format!("Modified {text}")
}

/// "12 Mar 2026" in UTC: the standard library has no time zones, and a date only has to say
/// which day.
fn date(at: SystemTime) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let days = at
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400) as i64;
    // Civil date from a day count (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{day} {} {year}", MONTHS[(month - 1) as usize])
}
