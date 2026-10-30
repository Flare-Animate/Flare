//! Tweens as XFL names them: the kind (`tweenType`) and the easing (`acceleration`).
#![allow(clippy::expect_used)]

use std::collections::HashMap;

use flare_document::Tween;
use flare_import::{FileSource, Severity, import_xfl};

#[derive(Default)]
struct Files(HashMap<String, String>);

impl FileSource for Files {
    fn read(&self, name: &str) -> std::io::Result<Option<Vec<u8>>> {
        Ok(self.0.get(name).map(|s| s.clone().into_bytes()))
    }
}

const SQUARE: &str = r##"<DOMShape><fills><FillStyle index="1"><SolidColor color="#FF0000"/></FillStyle></fills>
<edges><Edge fillStyle1="1" edges="!0 0|200 0|200 200|0 200|0 0"/></edges></DOMShape>"##;

fn import(frames: &str) -> flare_import::Imported {
    let xml = format!(
        r#"<DOMDocument frameRate="24" width="100" height="100"><timelines><DOMTimeline name="S"><layers>
        <DOMLayer name="L"><frames>{frames}</frames></DOMLayer></layers></DOMTimeline></timelines></DOMDocument>"#
    );
    let mut files = Files::default();
    files.0.insert("DOMDocument.xml".to_owned(), xml);
    import_xfl(&files).expect("import")
}

fn spans(imported: &flare_import::Imported) -> Vec<(Tween, i8)> {
    imported.document.scenes()[0].timeline().layers()[0]
        .spans()
        .iter()
        .map(|s| (s.tween(), s.ease()))
        .collect()
}

fn warnings(imported: &flare_import::Imported) -> String {
    imported
        .diagnostics
        .iter()
        .filter(|d| d.severity != Severity::Info)
        .map(|d| d.message.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_shape_tween_and_its_ease_come_in() {
    let imported = import(&format!(
        r#"<DOMFrame index="0" duration="4" tweenType="shape" acceleration="40"><elements>{SQUARE}</elements></DOMFrame>
           <DOMFrame index="4" duration="1"><elements>{SQUARE}</elements></DOMFrame>"#
    ));
    assert_eq!(spans(&imported), [(Tween::Shape, 40), (Tween::None, 0)]);
    assert!(warnings(&imported).is_empty(), "{}", warnings(&imported));
}

#[test]
fn the_ease_of_a_motion_tween_is_kept_and_a_sloppy_one_is_brought_into_range() {
    let instance = r#"<DOMSymbolInstance libraryItemName="Missing"/>"#;
    let imported = import(&format!(
        r#"<DOMFrame index="0" duration="2" tweenType="motion" acceleration="-30"><elements>{instance}</elements></DOMFrame>"#
    ));
    // The symbol is not in the library, so the instance cannot be placed: the tween is dropped
    // with a reason rather than guessed at.
    assert_eq!(spans(&imported)[0].0, Tween::None);
    let shapes = |acceleration: &str| {
        import(&format!(
            r#"<DOMFrame index="0" duration="2" tweenType="shape" acceleration="{acceleration}"><elements>{SQUARE}</elements></DOMFrame>"#
        ))
    };
    assert_eq!(spans(&shapes("250"))[0], (Tween::Shape, 100));
    assert_eq!(spans(&shapes("-250"))[0], (Tween::Shape, -100));
    assert_eq!(spans(&shapes("12.6"))[0], (Tween::Shape, 13));
    assert_eq!(spans(&shapes("lots"))[0], (Tween::Shape, 0));
    assert_eq!(spans(&shapes("NaN"))[0], (Tween::Shape, 0));
}

#[test]
fn a_tween_with_nothing_to_tween_is_dropped_with_a_reason() {
    let imported =
        import(r#"<DOMFrame index="0" duration="2" tweenType="shape"><elements/></DOMFrame>"#);
    assert_eq!(spans(&imported)[0].0, Tween::None);
    assert!(warnings(&imported).contains("a shape tween needs a drawn shape"));
    let unknown = import(&format!(
        r#"<DOMFrame index="0" duration="2" tweenType="classic"><elements>{SQUARE}</elements></DOMFrame>"#
    ));
    assert_eq!(spans(&unknown)[0].0, Tween::None);
    assert!(warnings(&unknown).contains("classic tweens are not imported"));
}

#[test]
fn the_ease_of_a_motion_tween_between_instances_is_kept() {
    let main = r#"<DOMDocument frameRate="24" width="100" height="100"><symbols><Include href="Dot.xml"/></symbols>
      <timelines><DOMTimeline name="S"><layers><DOMLayer name="L"><frames>
      <DOMFrame index="0" duration="4" tweenType="motion" acceleration="-30"><elements>
        <DOMSymbolInstance libraryItemName="Dot"><matrix><Matrix tx="0" ty="0"/></matrix></DOMSymbolInstance></elements></DOMFrame>
      <DOMFrame index="4" duration="1"><elements>
        <DOMSymbolInstance libraryItemName="Dot"><matrix><Matrix tx="100" ty="0"/></matrix></DOMSymbolInstance></elements></DOMFrame>
      </frames></DOMLayer></layers></DOMTimeline></timelines></DOMDocument>"#;
    let dot = format!(
        r#"<DOMSymbolItem name="Dot" symbolType="graphic"><timeline><DOMTimeline name="Dot"><layers><DOMLayer name="L"><frames>
        <DOMFrame index="0"><elements>{SQUARE}</elements></DOMFrame></frames></DOMLayer></layers></DOMTimeline></timeline></DOMSymbolItem>"#
    );
    let mut files = Files::default();
    files
        .0
        .insert("DOMDocument.xml".to_owned(), main.to_owned());
    files.0.insert("LIBRARY/Dot.xml".to_owned(), dot);
    let imported = import_xfl(&files).expect("import");
    assert_eq!(spans(&imported), [(Tween::Linear, -30), (Tween::None, 0)]);
    assert!(
        !warnings(&imported).contains("easing"),
        "{}",
        warnings(&imported)
    );
}
