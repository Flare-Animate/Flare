//! A Flash movie's ActionScript 3 through the MCP server: classes listed and written out,
//! strings listed, and a server's name patched into a new movie in the export directory.
#![allow(clippy::expect_used)]

mod common;

use common::{Fixture, fixture};
use flare_as3::fixtures::{HOST, MAIN_CLASS, movie};
use serde_json::json;

async fn with_game(write: bool) -> Fixture {
    let f = fixture(write, true).await;
    let exports = f.dir.path().join("exports");
    std::fs::create_dir_all(&exports).expect("exports");
    std::fs::write(
        exports.join("game.swf"),
        movie(swf::Compression::Zlib).expect("fixture"),
    )
    .expect("write");
    f
}

#[tokio::test]
async fn the_classes_are_listed_and_one_is_written_out() {
    let f = with_game(false).await;
    let list = f
        .ok(
            "movie_scripts",
            json!({ "source": "game.swf", "find": "tanks" }),
        )
        .await;
    assert_eq!(list["matched"], 1);
    assert_eq!(list["classes"][0]["full_name"], MAIN_CLASS);

    let class = f
        .ok(
            "movie_script_class",
            json!({ "source": "game.swf", "class": MAIN_CLASS, "member": "connect", "code": true }),
        )
        .await;
    let text = class["text"].as_str().expect("text");
    assert!(text.contains("public function connect(host:String, port:int = 5190):void"));
    assert!(text.contains("callpropvoid trace, 1"), "{text}");
}

#[tokio::test]
async fn a_server_name_is_patched_even_by_a_read_only_server() {
    // A patch writes a new movie into the export directory, as an export does; the project is
    // not touched, so read access is enough.
    let f = with_game(false).await;
    let revision = f.revision().await;
    let strings = f
        .ok(
            "movie_script_strings",
            json!({ "source": "game.swf", "find": HOST, "text_only": true }),
        )
        .await;
    assert_eq!(strings["matched"], 3);

    let patched = f
        .ok(
            "patch_movie_strings",
            json!({
                "source": "game.swf",
                "name": "patched",
                "replacements": [{ "from": HOST, "to": "play.example.org" }],
            }),
        )
        .await;
    assert_eq!(patched["changes"][0]["blocks"], json!([0, 1]));
    assert!(f.dir.path().join("exports/patched.swf").is_file());
    let after = f
        .ok(
            "movie_script_strings",
            json!({ "source": "patched.swf", "find": "play.example.org" }),
        )
        .await;
    assert_eq!(after["matched"], 2);
    assert_eq!(f.revision().await, revision, "the project is unchanged");
}

#[tokio::test]
async fn a_patch_that_cannot_be_made_is_refused_and_writes_nothing() {
    let f = with_game(true).await;
    let patch = |name: &str, from: &str| {
        json!({
            "source": "game.swf",
            "name": name,
            "replacements": [{ "from": from, "to": "x" }],
        })
    };
    let (failed, message) = f.tool("patch_movie_strings", patch("a", "connect")).await;
    assert!(failed && message.to_string().contains("a name the code links by"));
    let (failed, message) = f.tool("patch_movie_strings", patch("b", "nowhere")).await;
    assert!(
        failed && message.to_string().contains("no string"),
        "{message}"
    );
    let (failed, message) = f.tool("patch_movie_strings", patch("game", HOST)).await;
    assert!(
        failed && message.to_string().contains("already exists"),
        "{message}"
    );
    let (failed, _) = f
        .tool("movie_scripts", json!({ "source": "../outside.swf" }))
        .await;
    assert!(failed, "a name leaving the export directory is refused");
    for name in ["a.swf", "b.swf"] {
        assert!(!f.dir.path().join("exports").join(name).exists(), "{name}");
    }
}
