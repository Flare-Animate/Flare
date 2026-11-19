#![allow(clippy::expect_used)]

use std::cell::Cell;
use std::io::{Read, Write};
use std::net::TcpListener;

use super::*;

/// A small Google font (Squada One, SIL Open Font License, licence text beside it).
const SQUADA: &[u8] =
    include_bytes!("../../../foundation/flare-text/data/test/SquadaOne-Regular.ttf");

fn never(_: &str) -> Result<Vec<u8>, FetchError> {
    panic!("nothing should have been downloaded");
}

#[test]
fn a_family_is_downloaded_once_then_read_from_the_cache() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cache = Cache::new(dir.path().to_owned());
    let asked = Cell::new(0);
    let get = |url: &str| {
        asked.set(asked.get() + 1);
        assert!(
            url.ends_with("/ofl/squadaone/SquadaOne-Regular.ttf"),
            "{url}"
        );
        Ok(SQUADA.to_vec())
    };
    let (family, files) = fetch_family("squada one", Some(&cache), get).expect("downloads");
    assert_eq!(family.name(), "Squada One");
    assert_eq!(files, vec![SQUADA.to_vec()]);
    assert_eq!(asked.get(), 1);
    let (_, again) = fetch_family("Squada One", Some(&cache), never).expect("cached");
    assert_eq!(again, files);
}

#[test]
fn what_is_not_the_font_is_neither_kept_nor_returned() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cache = Cache::new(dir.path().to_owned());
    let error_page = |_: &str| Ok(b"<html>429 Too Many Requests</html>".to_vec());
    let error = fetch_family("Squada One", Some(&cache), error_page).expect_err("not a font");
    assert!(matches!(error, FetchError::NotTheFont { .. }), "{error}");
    let wrong = |_: &str| Ok(SQUADA.to_vec());
    let error = fetch_family("Abel", Some(&cache), wrong).expect_err("not Abel");
    assert!(error.to_string().contains("Squada One"), "{error}");
    let offline = |url: &str| {
        Err(FetchError::Download {
            url: url.into(),
            reason: "offline".into(),
        })
    };
    assert!(fetch_family("Squada One", Some(&cache), offline).is_err());
    assert!(matches!(
        fetch_family("Comic Sans MS", Some(&cache), never),
        Err(FetchError::Unknown { .. })
    ));
    let kept = std::fs::read_dir(dir.path())
        .map(|d| d.count())
        .unwrap_or(0);
    assert_eq!(kept, 0, "a failed download left files behind");
}

#[test]
fn a_damaged_cached_file_is_downloaded_again_and_older_commits_are_cleared() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cache = Cache::new(dir.path().to_owned());
    let family = flare_text::google::family_named("Squada One").expect("in the catalog");
    let path = dir.path().join(&family.cache_names()[0]);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
    std::fs::write(&path, &SQUADA[..100]).expect("write");
    let old = dir.path().join("0".repeat(40));
    std::fs::create_dir_all(old.join("ofl")).expect("old commit");
    let unrelated = dir.path().join("notes");
    std::fs::create_dir_all(&unrelated).expect("unrelated");
    assert!(cache.read(family).is_none(), "a cut-off file was read");
    let (_, files) =
        fetch_family("Squada One", Some(&cache), |_| Ok(SQUADA.to_vec())).expect("downloads");
    assert_eq!(files[0], SQUADA);
    assert_eq!(std::fs::read(&path).expect("kept"), SQUADA);
    assert!(!old.exists(), "an older commit's files were kept");
    assert!(
        unrelated.exists(),
        "something that is not a commit was removed"
    );
}

/// Serves one HTTP response on a local port and returns its address.
fn serve_once(response: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("address");
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request);
            let _ = stream.write_all(&response);
        }
    });
    format!("http://{address}/font.ttf")
}

#[test]
fn a_download_reads_the_body_and_refuses_errors_and_oversized_files() {
    let ok = serve_once(
        [
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                SQUADA.len()
            )
            .as_bytes(),
            SQUADA,
        ]
        .concat(),
    );
    assert_eq!(download(&ok).expect("downloads"), SQUADA);
    let missing = serve_once(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_vec());
    let error = download(&missing).expect_err("404");
    assert!(error.to_string().contains("404"), "{error}");
    let huge = flare_text::MAX_FONT_BYTES + 1;
    let mut response = format!("HTTP/1.1 200 OK\r\nContent-Length: {huge}\r\n\r\n").into_bytes();
    response.resize(response.len() + huge, 0);
    assert!(
        download(&serve_once(response)).is_err(),
        "an oversized file"
    );
    assert!(download("http://127.0.0.1:1/nothing-listens").is_err());
}

#[test]
fn a_downloader_reads_what_its_cache_has_without_the_network() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cache = Cache::new(dir.path().to_owned());
    fetch_family("Squada One", Some(&cache), |_| Ok(SQUADA.to_vec())).expect("kept");
    let downloader = Downloader::new(Some(cache));
    assert_eq!(
        downloader.fetch("Squada One").expect("cached"),
        vec![SQUADA.to_vec()]
    );
    let error = downloader.fetch("Comic Sans MS").expect_err("unknown");
    assert!(error.contains("Comic Sans MS"), "{error}");
    let offline = Downloader::new(None).with_transport(|url| {
        Err(FetchError::Download {
            url: url.into(),
            reason: "offline".into(),
        })
    });
    let error = offline.fetch("Squada One").expect_err("offline");
    assert!(error.contains("offline"), "{error}");
}
