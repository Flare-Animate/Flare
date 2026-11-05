use super::records::edit_values;
use super::*;
use flare_document::StrokeStyle;

// Strokes in the sample have a "cap" (they are not round), so that is where a style goes.
fn add_style(v: &mut serde_json::Value, style: &str) {
    match v {
        serde_json::Value::Object(map) => {
            if map.contains_key("cap") {
                map.insert("style".to_owned(), serde_json::json!(style));
            }
            map.values_mut().for_each(|v| add_style(v, style));
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(|v| add_style(v, style)),
        _ => {}
    }
}

#[test]
fn every_geometric_stroke_style_is_saved_and_loaded_back() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    let again = dir.path().join("b.flare");
    for style in StrokeStyle::ALL.into_iter().filter(|s| s.is_geometric()) {
        save(&sample(), &path).expect("save");
        edit_values(&path, &["scene"], |v| add_style(v, style.name()));
        let loaded = load(&path).expect("a known style loads");
        save(&loaded, &again).expect("save again");
        assert_eq!(
            load(&again).expect("load"),
            loaded,
            "{style} is written back"
        );
    }
}

#[test]
fn stroke_style_survives_and_an_unknown_one_is_refused() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    let again = dir.path().join("b.flare");
    let document = sample();
    save(&document, &path).expect("save");
    edit_values(&path, &["scene"], |v| add_style(v, "dotted"));
    let dotted = load(&path).expect("a known style loads");
    assert_ne!(dotted, document, "the style was read");
    save(&dotted, &again).expect("save again");
    assert_eq!(load(&again).expect("load"), dotted, "and written back");
    edit_values(&path, &["scene"], |v| add_style(v, "wavy"));
    assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
}
