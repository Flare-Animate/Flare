//! A movie's classes, their declarations and their code, read from the fixture game.
#![allow(clippy::expect_used)]

use flare_as3::fixtures::{HOST, MAIN_CLASS, SERVER_URL, movie};
use flare_as3::{
    MemberKind, Movie, OutlineQuery, ScriptError, SourceOptions, class_source, outline,
};
use swf::Compression;

fn game() -> Movie {
    Movie::read(&movie(Compression::Zlib).expect("fixture")).expect("read")
}

#[test]
fn every_class_is_listed_with_its_package_base_and_members() {
    let found = outline(&game(), &OutlineQuery::default());
    assert_eq!(found.blocks.len(), 2);
    assert_eq!(found.blocks[0].name, "frame1");
    assert_eq!(found.matched, 2);
    let main = &found.classes[0];
    assert_eq!(main.full_name, MAIN_CLASS);
    assert_eq!(main.package, "alternativa.tanks");
    assert_eq!(main.extends.as_deref(), Some("flash.display.Sprite"));
    assert!(!main.dynamic && !main.interface);

    let server = main
        .members
        .iter()
        .find(|m| m.name == "SERVER")
        .expect("SERVER");
    assert_eq!(server.kind, MemberKind::Const);
    assert!(server.is_static);
    assert_eq!(server.type_name.as_deref(), Some("String"));
    assert_eq!(server.value.as_deref(), Some(&*format!("\"{SERVER_URL}\"")));

    let lives = main
        .members
        .iter()
        .find(|m| m.name == "lives")
        .expect("lives");
    assert_eq!(
        (lives.kind, lives.visibility.as_str()),
        (MemberKind::Var, "private")
    );
    assert_eq!(lives.value.as_deref(), Some("3"));

    let connect = main
        .members
        .iter()
        .find(|m| m.name == "connect")
        .and_then(|m| m.signature.as_ref())
        .expect("connect");
    assert_eq!(connect.returns, "void");
    assert_eq!(connect.params[0].name.as_deref(), Some("host"));
    assert_eq!(connect.params[0].type_name, "String");
    assert_eq!(connect.params[1].default.as_deref(), Some("5190"));
    assert!(connect.code_bytes > 0);

    let helper = &found.classes[1];
    assert_eq!((helper.block, helper.full_name.as_str()), (1, "Helper"));
    assert!(helper.is_final);
    assert_eq!(helper.extends, None, "Object is not named as a base");
}

#[test]
fn a_search_and_a_limit_narrow_the_list_but_count_every_match() {
    let movie = game();
    let found = outline(
        &movie,
        &OutlineQuery {
            find: Some("TANKS".into()),
            limit: None,
        },
    );
    assert_eq!(found.matched, 1);
    assert_eq!(found.classes[0].full_name, MAIN_CLASS);

    let limited = outline(
        &movie,
        &OutlineQuery {
            find: None,
            limit: Some(1),
        },
    );
    assert_eq!((limited.matched, limited.classes.len()), (2, 1));
}

#[test]
fn a_class_is_written_out_as_its_declaration() {
    let source = class_source(&game(), MAIN_CLASS, &SourceOptions::default()).expect("source");
    let text = &source.text;
    assert!(!source.truncated);
    for line in [
        "package alternativa.tanks {",
        "    public class Main extends flash.display.Sprite {",
        &format!("        public static const SERVER:String = \"{SERVER_URL}\";"),
        "        private var lives:int = 3;",
        "        public function Main() { /* ",
        "        public function connect(host:String, port:int = 5190):void { /* ",
    ] {
        assert!(text.contains(line), "{line:?} missing from\n{text}");
    }
    assert!(
        !text.contains("pushstring"),
        "bodies are only sized\n{text}"
    );
}

#[test]
fn with_code_each_body_is_listed_with_names_and_branch_targets() {
    let options = SourceOptions {
        code: true,
        member: Some("connect".into()),
    };
    let source = class_source(&game(), "Main", &options).expect("found by its short name");
    let text = &source.text;
    assert!(text.contains(&format!("pushstring \"{HOST}\"")), "{text}");
    assert!(text.contains("callpropvoid trace, 1"), "{text}");
    assert!(!text.contains("lives"), "only the member asked for\n{text}");
    // The branch skips the call and lands on the last instruction.
    let target = text
        .lines()
        .find_map(|l| l.split("ifstrictne L").nth(1))
        .expect("a branch");
    let last = text
        .lines()
        .rfind(|l| l.contains("returnvoid"))
        .expect("returnvoid");
    assert!(
        last.contains(&format!("// L{target} ")),
        "{target} in\n{text}"
    );
}

#[test]
fn an_unknown_class_or_member_is_named_in_the_refusal() {
    let movie = game();
    assert_eq!(
        class_source(&movie, "Nope", &SourceOptions::default()),
        Err(ScriptError::NoClass("Nope".into()))
    );
    let options = SourceOptions {
        code: false,
        member: Some("fly".into()),
    };
    assert!(matches!(
        class_source(&movie, MAIN_CLASS, &options),
        Err(ScriptError::NoMember { member, .. }) if member == "fly"
    ));
}
