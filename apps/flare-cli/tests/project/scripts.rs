//! `flare scripts` on a movie shaped like an old Flash 3D game: its classes listed and written
//! out, its strings listed, and a server's name patched into a new movie.

use super::{json, run};
use flare_as3::fixtures::{HOST, MAIN_CLASS, movie};

fn game(dir: &std::path::Path) -> String {
    let path = dir.join("game.swf");
    std::fs::write(&path, movie(swf::Compression::Zlib).expect("fixture")).expect("write");
    path.to_string_lossy().into_owned()
}

#[test]
fn the_classes_are_listed_and_one_is_written_out_with_its_code() {
    let dir = tempfile::tempdir().expect("dir");
    let game = game(dir.path());
    let list = json(&["scripts", "list", &game, "--find", "tanks"]);
    assert_eq!(list["matched"], 1);
    assert_eq!(list["classes"][0]["full_name"], MAIN_CLASS);
    assert_eq!(list["classes"][0]["extends"], "flash.display.Sprite");

    let class = json(&["scripts", "class", &game, "Main", "--code"]);
    let text = class["text"].as_str().expect("text");
    assert!(
        text.contains("public class Main extends flash.display.Sprite"),
        "{text}"
    );
    assert!(text.contains(&format!("pushstring \"{HOST}\"")), "{text}");

    let out = dir.path().join("Main.as");
    let written = json(&[
        "scripts",
        "class",
        &game,
        MAIN_CLASS,
        "--out",
        &out.to_string_lossy(),
    ]);
    assert!(written.get("text").is_none());
    let file = std::fs::read_to_string(&out).expect("the .as file");
    assert!(file.contains("package alternativa.tanks {"), "{file}");
}

#[test]
fn a_server_name_is_patched_into_a_new_movie() {
    let dir = tempfile::tempdir().expect("dir");
    let game = game(dir.path());
    let strings = json(&["scripts", "strings", &game, "--find", HOST, "--text"]);
    assert_eq!(strings["matched"], 3, "{strings}");

    let out = dir.path().join("patched.swf");
    let out = out.to_string_lossy();
    let patched = json(&[
        "scripts",
        "patch",
        &game,
        "--out",
        &out,
        "--from",
        HOST,
        "--to",
        "play.example.org",
    ]);
    assert_eq!(patched["changes"][0]["blocks"], serde_json::json!([0, 1]));
    let after = json(&["scripts", "strings", &out, "--find", "play.example.org"]);
    assert_eq!(after["matched"], 2);
    let before = json(&["scripts", "strings", &game, "--find", "play.example.org"]);
    assert_eq!(before["matched"], 0, "the original is not touched");
}

#[test]
fn a_patch_that_cannot_be_made_says_why_and_writes_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let game = game(dir.path());
    let out = dir.path().join("patched.swf");
    let out_text = out.to_string_lossy().into_owned();
    let refused = |args: &[&str], why: &str| {
        let output = run(args);
        assert!(!output.status.success(), "{args:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(why), "{why:?} not in {stderr}");
        assert!(!out.exists(), "nothing written for {args:?}");
    };
    let base = ["scripts", "patch", &game, "--out", &out_text];
    refused(
        &[&base[..], &["--from", "connect", "--to", "join"]].concat(),
        "a name the code links by",
    );
    refused(
        &[&base[..], &["--from", "nowhere", "--to", "x"]].concat(),
        "no string \"nowhere\"",
    );
    refused(
        &[
            &base[..],
            &["--from", HOST, "--from", "connect", "--to", "x"],
        ]
        .concat(),
        "each --from needs one --to",
    );
    // Renaming is allowed when asked for; then the file is there and is not written over.
    json(
        &[
            &base[..],
            &["--from", "connect", "--to", "join", "--rename-names"],
        ]
        .concat(),
    );
    let written = std::fs::read(&out).expect("patched");
    let again = run(&[&base[..], &["--from", HOST, "--to", "x"]].concat());
    assert!(!again.status.success());
    assert!(String::from_utf8_lossy(&again.stderr).contains("already exists"));
    assert_eq!(std::fs::read(&out).expect("still there"), written);
}
