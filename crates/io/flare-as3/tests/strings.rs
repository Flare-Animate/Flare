//! A movie's script strings: listed with their uses, and replaced without touching anything
//! else in the movie.
#![allow(clippy::expect_used)]

use flare_as3::fixtures::{HOST, MAIN_CLASS, SERVER_URL, movie};
use flare_as3::{
    Movie, OutlineQuery, Replacement, ScriptError, StringQuery, outline, patch_strings, strings,
};
use swf::avm2::types::AbcFile;
use swf::{Compression, Tag};

fn game() -> Movie {
    Movie::read(&movie(Compression::Zlib).expect("fixture")).expect("read")
}

fn replace(from: &str, to: &str) -> Replacement {
    Replacement {
        from: from.into(),
        to: to.into(),
    }
}

/// Every tag of a movie, with the ABC of each script block read.
fn tags_and_scripts(bytes: &[u8]) -> (Vec<String>, Vec<AbcFile>) {
    let buf = swf::decompress_swf(bytes).expect("decompress");
    let movie = swf::parse_swf(&buf).expect("parse");
    let mut tags = Vec::new();
    let mut scripts = Vec::new();
    for tag in &movie.tags {
        match tag {
            Tag::DoAbc2(abc) => {
                tags.push(format!("DoAbc2 {:?} {:?}", abc.flags, abc.name));
                scripts.push(swf::avm2::read::Reader::new(abc.data).read().expect("abc"));
            }
            Tag::DoAbc(data) => {
                tags.push("DoAbc".into());
                scripts.push(swf::avm2::read::Reader::new(data).read().expect("abc"));
            }
            other => tags.push(format!("{other:?}")),
        }
    }
    (tags, scripts)
}

#[test]
fn each_string_is_listed_with_what_uses_it() {
    let found = strings(&game(), &StringQuery::default());
    let host: Vec<_> = found.strings.iter().filter(|s| s.text == HOST).collect();
    assert_eq!(host.len(), 2, "once in each block");
    assert_eq!(
        (host[0].block, host[0].uses.text),
        (0, 1),
        "pushed by connect"
    );
    assert_eq!(
        (host[1].block, host[1].uses.text),
        (1, 1),
        "Helper.HOST's value"
    );
    assert_eq!(host[0].uses.name, 0);

    let connect = found
        .strings
        .iter()
        .find(|s| s.text == "connect")
        .expect("connect");
    assert_eq!((connect.uses.name, connect.uses.text), (1, 0));
    let host_param = found
        .strings
        .iter()
        .find(|s| s.text == "host")
        .expect("host");
    assert_eq!(host_param.uses.other, 1, "a parameter's name");
}

#[test]
fn a_listing_can_be_searched_kept_to_text_and_limited() {
    let movie = game();
    let found = strings(
        &movie,
        &StringQuery {
            find: Some("OLD.EX".into()),
            ..StringQuery::default()
        },
    );
    assert_eq!(found.matched, 3, "the host twice and the address");

    let text = strings(
        &movie,
        &StringQuery {
            text_only: true,
            ..StringQuery::default()
        },
    );
    let texts: Vec<&str> = text.strings.iter().map(|s| s.text.as_str()).collect();
    assert!(texts.contains(&SERVER_URL) && texts.contains(&HOST));
    assert!(!texts.contains(&"connect") && !texts.contains(&"Main"));

    let limited = strings(
        &movie,
        &StringQuery {
            limit: Some(2),
            ..StringQuery::default()
        },
    );
    assert_eq!(limited.strings.len(), 2);
    assert!(limited.matched > 2);
}

#[test]
fn a_patch_changes_the_strings_and_nothing_else() {
    for compression in [Compression::None, Compression::Zlib, Compression::Lzma] {
        let original = movie(compression).expect("fixture");
        let patched = patch_strings(
            &Movie::read(&original).expect("read"),
            &[
                replace(HOST, "play.example.org"),
                replace(SERVER_URL, "ws://a.b:1"),
            ],
            false,
        )
        .expect("patch");
        assert_eq!(patched.bytes[..3], original[..3], "{compression:?} is kept");
        assert_eq!(patched.changes[0].blocks, [0, 1]);
        assert_eq!(patched.changes[1].blocks, [0]);
        assert_eq!(
            patched.changes[1].uses.text, 2,
            "the constant and the trace"
        );

        let (before_tags, mut before) = tags_and_scripts(&original);
        let (after_tags, after) = tags_and_scripts(&patched.bytes);
        assert_eq!(before_tags, after_tags, "every tag is kept, in order");
        // Put the new strings into the old blocks: then they must be the same in every field.
        for abc in &mut before {
            for s in &mut abc.constant_pool.strings {
                if s == HOST.as_bytes() {
                    *s = b"play.example.org".to_vec();
                } else if s == SERVER_URL.as_bytes() {
                    *s = b"ws://a.b:1".to_vec();
                }
            }
        }
        assert_eq!(before, after, "{compression:?}");
    }
}

#[test]
fn a_patched_movie_reads_back_with_its_classes_whole() {
    let patched = patch_strings(&game(), &[replace(HOST, "h")], false).expect("patch");
    let movie = Movie::read(&patched.bytes).expect("read again");
    let classes = outline(&movie, &OutlineQuery::default());
    assert_eq!(classes.classes[0].full_name, MAIN_CLASS);
    let helper = &classes.classes[1].members[0];
    assert_eq!(helper.value.as_deref(), Some("\"h\""));
    let left = strings(
        &movie,
        &StringQuery {
            find: Some(HOST.into()),
            ..StringQuery::default()
        },
    );
    assert_eq!(
        left.matched, 1,
        "only the address still has the old host in it"
    );
}

#[test]
fn a_name_the_code_links_by_is_renamed_only_when_allowed() {
    let movie = game();
    assert_eq!(
        patch_strings(&movie, &[replace("connect", "join")], false),
        Err(ScriptError::IsName("connect".into()))
    );
    let renamed = patch_strings(&movie, &[replace("connect", "join")], true).expect("allowed");
    assert_eq!(renamed.changes[0].uses.name, 1);
    let classes = outline(
        &Movie::read(&renamed.bytes).expect("read"),
        &OutlineQuery::default(),
    );
    assert!(classes.classes[0].members.iter().any(|m| m.name == "join"));
}

#[test]
fn a_bad_patch_is_refused_whole() {
    let movie = game();
    let refused = [
        (vec![], ScriptError::EmptyPatch),
        (vec![replace("", "x")], ScriptError::EmptyFrom),
        (
            vec![replace(HOST, "a"), replace(HOST, "b")],
            ScriptError::Duplicate(HOST.into()),
        ),
        (
            vec![replace(HOST, "a"), replace("nowhere", "b")],
            ScriptError::NotFound("nowhere".into()),
        ),
        (
            vec![replace(
                HOST,
                &"x".repeat(flare_as3::MAX_REPLACEMENT_BYTES + 1),
            )],
            ScriptError::ReplacementTooLong(HOST.into()),
        ),
    ];
    for (patch, error) in refused {
        assert_eq!(patch_strings(&movie, &patch, false), Err(error));
    }
    let many: Vec<_> = (0..=flare_as3::MAX_REPLACEMENTS)
        .map(|i| replace(&i.to_string(), "x"))
        .collect();
    assert_eq!(
        patch_strings(&movie, &many, false),
        Err(ScriptError::TooManyReplacements)
    );
}
