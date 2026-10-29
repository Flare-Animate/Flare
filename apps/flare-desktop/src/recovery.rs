//! Where recovery files live and the startup prompt that offers them.

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::SystemTime;

use flare_storage::RecoveryEntry;
use flare_ui::Workspace;

/// The per-user recovery directory, given an environment lookup (injectable for tests).
pub fn recovery_dir_with(env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let var = |name: &str| env(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    let base = if cfg!(target_os = "macos") {
        var("HOME")?.join("Library").join("Application Support")
    } else if cfg!(target_os = "windows") {
        var("APPDATA")?
    } else {
        var("XDG_DATA_HOME").or_else(|| var("HOME").map(|h| h.join(".local").join("share")))?
    };
    Some(base.join("Flare").join("recovery"))
}

pub fn recovery_dir() -> Option<PathBuf> {
    recovery_dir_with(|name| std::env::var_os(name))
}

/// A short, human description of how old a recovery file is.
pub fn describe_age(modified: SystemTime, now: SystemTime) -> String {
    let secs = now.duration_since(modified).map_or(0, |d| d.as_secs());
    match secs {
        0..=59 => "less than a minute ago".to_owned(),
        60..=3599 => format!("{} minutes ago", secs / 60),
        3600..=86_399 => format!("{} hours ago", secs / 3600),
        _ => format!("{} days ago", secs / 86_400),
    }
}

/// Offers the newest recovery file. Yes recovers it, No discards it, Cancel keeps it for
/// next time. Only one document is open at a time, so older files wait their turn.
pub fn offer_recovery(workspace: &mut Workspace, newest: &RecoveryEntry) {
    let answer = rfd::MessageDialog::new()
        .set_title("Recover unsaved work?")
        .set_description(format!(
            "Flare found unsaved work from {}.\n\nYes: recover it\nNo: discard it\nCancel: decide next time",
            describe_age(newest.modified, SystemTime::now())
        ))
        .set_buttons(rfd::MessageButtons::YesNoCancel)
        .show();
    match answer {
        rfd::MessageDialogResult::Yes => workspace.recover(&newest.path),
        rfd::MessageDialogResult::No => {
            // The file may already be gone (another instance, or a clean save); that is fine.
            if let Err(error) = std::fs::remove_file(&newest.path)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                eprintln!("could not discard {}: {error}", newest.path.display());
            }
        }
        _ => {}
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| OsString::from(*v))
        }
    }

    #[test]
    fn the_directory_follows_the_platform_convention() {
        let dir = recovery_dir_with(env(&[
            ("HOME", "/home/u"),
            ("XDG_DATA_HOME", "/data"),
            ("APPDATA", "C:\\Users\\u\\AppData\\Roaming"),
        ]))
        .expect("dir");
        assert!(dir.ends_with("Flare/recovery") || dir.ends_with("Flare\\recovery"));
        if cfg!(target_os = "macos") {
            assert_eq!(
                dir,
                PathBuf::from("/home/u/Library/Application Support/Flare/recovery")
            );
        } else if cfg!(target_os = "linux") {
            assert_eq!(dir, PathBuf::from("/data/Flare/recovery"));
        }
    }

    #[test]
    fn missing_or_empty_variables_give_no_directory_rather_than_a_relative_path() {
        assert_eq!(recovery_dir_with(env(&[])), None);
        assert_eq!(
            recovery_dir_with(env(&[("HOME", ""), ("APPDATA", ""), ("XDG_DATA_HOME", "")])),
            None
        );
    }

    #[test]
    fn ages_read_naturally() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let ago = |s| describe_age(now - Duration::from_secs(s), now);
        assert_eq!(ago(5), "less than a minute ago");
        assert_eq!(ago(120), "2 minutes ago");
        assert_eq!(ago(7200), "2 hours ago");
        assert_eq!(ago(172_800), "2 days ago");
        assert_eq!(
            describe_age(now + Duration::from_secs(9), now),
            "less than a minute ago"
        );
    }
}
