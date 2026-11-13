//! `flare layer show-all | lock-others | hide-others | duplicate`, as the layer menu has them.

use super::*;

fn layers(file: &str) -> Vec<Value> {
    json(&["info", file])["scenes"][0]["layers"]
        .as_array()
        .expect("layers")
        .clone()
}

fn flag(file: &str, key: &str) -> Vec<bool> {
    layers(file)
        .iter()
        .map(|l| l[key].as_bool().expect("flag"))
        .collect()
}

#[test]
fn hide_others_lock_others_show_all_and_duplicate_change_the_project() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    json(&["add-layer", &file, "B"]);
    json(&["add-layer", &file, "C"]);
    let ids: Vec<String> = layers(&file)
        .iter()
        .map(|l| l["id"].as_str().expect("id").to_owned())
        .collect();
    assert_eq!(ids.len(), 3);

    json(&["layer", "hide-others", &file, "--layer", &ids[1]]);
    assert_eq!(flag(&file, "visible"), vec![false, true, false]);
    json(&["layer", "show-all", &file]);
    assert_eq!(flag(&file, "visible"), vec![true, true, true]);
    json(&["layer", "lock-others", &file, "--layer", &ids[2]]);
    assert_eq!(flag(&file, "locked"), vec![true, true, false]);

    let out = json(&["layer", "duplicate", &file, "--layer", &ids[1]]);
    let copy = out["copy"].as_str().expect("the copy's id").to_owned();
    let after = layers(&file);
    assert_eq!(after.len(), 4);
    assert_eq!(after[1]["id"], copy.as_str());
    assert_eq!(after[1]["name"], "B copy");

    // An unknown layer is refused and nothing is written.
    let before = std::fs::read(&file).expect("read");
    let bad = run(&[
        "layer",
        "hide-others",
        &file,
        "--layer",
        "00000000-0000-0000-0000-000000000000",
    ]);
    assert!(!bad.status.success());
    assert_eq!(std::fs::read(&file).expect("read"), before);
}

#[test]
fn symbol_from_layers_moves_the_layers_into_a_new_symbol() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    json(&[
        "draw-rect",
        &file,
        "--rect",
        "10",
        "10",
        "20",
        "20",
        "--duration",
        "6",
    ]);
    json(&["add-layer", &file, "Top"]);
    let ids: Vec<String> = layers(&file)
        .iter()
        .map(|l| l["id"].as_str().expect("id").to_owned())
        .collect();
    let before = json(&["eval", &file, "--frame", "0"]);
    let out = json(&[
        "symbol",
        "from-layers",
        &file,
        "--layer",
        &ids[0],
        "--name",
        "Flower",
    ]);
    assert_eq!(out["kind"], "graphic");
    let after = layers(&file);
    let names: Vec<&str> = after
        .iter()
        .map(|l| l["name"].as_str().expect("name"))
        .collect();
    assert_eq!(names, vec!["Flower", "Top"]);
    assert_eq!(after[0]["id"], out["layer"]);
    let info = json(&["info", &file]);
    assert!(
        info["symbols"]
            .as_array()
            .expect("symbols")
            .iter()
            .any(|s| s["name"] == "Flower")
    );
    // The same rectangle is still drawn, now inside the instance.
    let placed = json(&["eval", &file, "--frame", "0"]);
    assert_eq!(placed["placed"].as_array().expect("placed").len(), 1);
    assert_ne!(placed, before);

    // The name is taken now: refused, nothing written.
    let written = std::fs::read(&file).expect("read");
    let again = run(&[
        "symbol",
        "from-layers",
        &file,
        "--layer",
        &ids[1],
        "--name",
        "Flower",
    ]);
    assert!(!again.status.success());
    assert!(String::from_utf8_lossy(&again.stderr).contains("already has a symbol"));
    assert_eq!(std::fs::read(&file).expect("read"), written);
}

#[test]
fn layer_opacity_makes_a_layer_see_through_and_bad_values_are_refused() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    let id = layers(&file)[0]["id"].as_str().expect("id").to_owned();
    assert_eq!(layers(&file)[0]["opacity"], 100);
    let out = json(&["layer", "opacity", &file, "--layer", &id, "--percent", "40"]);
    assert_eq!(out["percent"], 40);
    assert_eq!(layers(&file)[0]["opacity"], 40);
    let written = std::fs::read(&file).expect("read");
    for bad in [
        vec!["layer", "opacity", &file, "--percent", "101"],
        vec!["layer", "opacity", &file],
    ] {
        assert!(!run(&bad).status.success(), "{bad:?}");
    }
    assert_eq!(
        std::fs::read(&file).expect("read"),
        written,
        "nothing written"
    );
}
