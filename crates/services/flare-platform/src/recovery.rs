//! The startup question about work left behind by a crash.

use std::time::SystemTime;

use flare_storage::RecoveryEntry;

/// What the person chose to do with a recovery file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Recover,
    /// Throw the file away.
    Discard,
    /// Keep it and ask again next time.
    Later,
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

/// Offers the newest recovery file. Only one document is open at a time, so older files wait
/// their turn. Choosing to discard deletes the file here.
pub fn offer(newest: &RecoveryEntry) -> Choice {
    let answer = rfd::MessageDialog::new()
        .set_title("Recover unsaved work?")
        .set_description(format!(
            "Flare found unsaved work from {}.\n\nYes: recover it\nNo: discard it\nCancel: decide next time",
            describe_age(newest.modified, SystemTime::now())
        ))
        .set_buttons(rfd::MessageButtons::YesNoCancel)
        .show();
    match answer {
        rfd::MessageDialogResult::Yes => Choice::Recover,
        rfd::MessageDialogResult::No => {
            // The file may already be gone (another instance, or a clean save); that is fine.
            if let Err(error) = std::fs::remove_file(&newest.path)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                eprintln!("could not discard {}: {error}", newest.path.display());
            }
            Choice::Discard
        }
        _ => Choice::Later,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

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
