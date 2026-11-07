#![allow(clippy::expect_used)]
//! Entity handling in `.tnz` XML. OpenToonz writes a bare `&` in names and reads it back, so the
//! reader keeps one as a literal; a malformed document is still refused.

use flare_toonz::{ToonzError, parse_scene};

#[test]
fn a_bare_ampersand_in_text_and_attributes_is_kept() {
    let scene = parse_scene(
        r#"<tnz version="71.0"><generator>Tom & Jerry</generator>
           <properties><outputs><output name="Lempel-Ziv & Welch"/></outputs></properties></tnz>"#,
    )
    .expect("a bare ampersand is read as a literal");
    assert_eq!(scene.generator.as_deref(), Some("Tom & Jerry"));
}

#[test]
fn valid_entities_still_decode() {
    let scene = parse_scene(
        r#"<tnz version="71.0"><generator>a &amp; b &lt;c&gt; &#65;&#x42;</generator></tnz>"#,
    )
    .expect("entities decode");
    assert_eq!(scene.generator.as_deref(), Some("a & b <c> AB"));
}

#[test]
fn an_unknown_entity_name_is_kept_as_written() {
    let scene = parse_scene(r#"<tnz version="71.0"><generator>a &nbsp; b</generator></tnz>"#)
        .expect("an unknown entity is not an error");
    assert_eq!(scene.generator.as_deref(), Some("a &nbsp; b"));
}

#[test]
fn a_bare_ampersand_does_not_hide_a_broken_tag() {
    assert!(
        matches!(
            parse_scene(r#"<tnz version="71.0"><generator>Tom & Jerry</tnz>"#),
            Err(ToonzError::Xml(_))
        ),
        "a mismatched end tag is still refused"
    );
}
