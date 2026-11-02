//! Opening a web page in the person's browser, for the Publish popup's share buttons.

use std::io;
use std::process::Command;

/// The longest address that is opened; the share pages are far shorter.
const MAX_URL: usize = 512;

/// The program and arguments that open `url` on this system, or `None` for an address that is not
/// a plain `https` one. Nothing a document or a file contains reaches here, but an address is
/// still checked before it is handed to the system, which would otherwise run what it names.
pub fn command_for(url: &str) -> Option<(&'static str, Vec<String>)> {
    let plain = url.len() <= MAX_URL
        && url.starts_with("https://")
        && url
            .chars()
            .all(|c| c.is_ascii_graphic() && c != '"' && c != '\'');
    if !plain {
        return None;
    }
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer.exe"
    } else {
        "xdg-open"
    };
    Some((program, vec![url.to_owned()]))
}

/// Opens `url` in the default browser. The browser is not waited for.
pub fn open_url(url: &str) -> io::Result<()> {
    let (program, args) = command_for(url).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "that address cannot be opened")
    })?;
    Command::new(program).args(args).spawn().map(|_| ())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_https_address_is_opened_with_the_systems_opener() {
        let (program, args) = command_for("https://x.com/intent/post?text=Made%20with%20Flare")
            .expect("https is fine");
        assert!(["open", "explorer.exe", "xdg-open"].contains(&program));
        assert_eq!(
            args,
            vec!["https://x.com/intent/post?text=Made%20with%20Flare"]
        );
    }

    #[test]
    fn anything_else_is_refused() {
        for bad in [
            "http://example.com",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https://example.com/a b",
            "https://example.com/\"; rm -rf ~",
            "https://example.com/\n",
            "",
        ] {
            assert!(command_for(bad).is_none(), "{bad:?}");
        }
        let long = format!("https://example.com/{}", "a".repeat(600));
        assert!(command_for(&long).is_none(), "too long");
        assert!(open_url("ftp://example.com").is_err());
    }
}
