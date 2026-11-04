#![allow(clippy::expect_used)]
//! Scene reader tests on tiny synthetic `.tnz` documents written for these tests.

use flare_toonz::{Column, FrameId, ToonzError, parse_scene};

const MINIMAL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<tnz version="71.0">
  <generator>synthetic</generator>
  <properties>
    <outputs>
      <output name="main">
        <range>0 2</range>
        <fps>24</fps>
      </output>
    </outputs>
    <cameras><camera/></cameras>
  </properties>
  <levelSet>
    <levels>
      <level id="1" name="A" type="vector"><path>+drawings/A.pli</path></level>
      <level id="2" name="B" type="raster"><path>+drawings/B.tlv</path></level>
      <level id="3" name="S" type="sound"><path>+sounds/S.wav</path></level>
    </levels>
  </levelSet>
  <xsheet>
    <columns>
      <levelColumn>
        <cells>
          <cell>0 2 1 1 0</cell>
          <cell>2 1 2 3a 0</cell>
        </cells>
      </levelColumn>
      <soundColumn>
        <SoundCells>0 10 0 3</SoundCells>
      </soundColumn>
      <fxColumn/>
    </columns>
  </xsheet>
</tnz>"#;

#[test]
fn reads_summary_of_a_minimal_scene() {
    let scene = parse_scene(MINIMAL).expect("minimal scene parses");
    assert_eq!(scene.version.as_deref(), Some("71.0"));
    assert_eq!(scene.generator.as_deref(), Some("synthetic"));
    assert_eq!(scene.frame_rate, Some(24.0));
    assert_eq!(scene.camera_count, 1);
    assert_eq!(scene.levels.len(), 3);
    assert_eq!(scene.levels[0].name, "A");
    assert_eq!(scene.levels[0].kind, "vector");
    assert_eq!(scene.levels[0].path.as_deref(), Some("+drawings/A.pli"));
    assert_eq!(scene.frame_count(), 3);
}

#[test]
fn reads_cell_runs_with_letters_and_holds() {
    let scene = parse_scene(MINIMAL).expect("parses");
    let Column::Level(runs) = &scene.columns[0] else {
        panic!("first column is a level column");
    };
    assert_eq!(runs.len(), 2);
    assert_eq!((runs[0].row, runs[0].count, runs[0].level), (0, 2, 1));
    assert_eq!(
        runs[0].frame,
        FrameId {
            number: 1,
            letter: None
        }
    );
    assert_eq!(
        runs[1].frame,
        FrameId {
            number: 3,
            letter: Some('a')
        }
    );
}

#[test]
fn reads_sound_column_and_keeps_unknown_columns() {
    let scene = parse_scene(MINIMAL).expect("parses");
    assert!(matches!(&scene.columns[1], Column::Sound(s) if s.level == 3 && s.end_offset == 10));
    assert!(matches!(&scene.columns[2], Column::Other { tag } if tag == "fxColumn"));
}

#[test]
fn increment_moves_the_frame_number() {
    let text = MINIMAL.replace("<cell>0 2 1 1 0</cell>", "<cell>0 2 1 1 1</cell>");
    let scene = parse_scene(&text).expect("parses");
    let Column::Level(runs) = &scene.columns[0] else {
        panic!("level column");
    };
    assert_eq!(runs[0].increment, 1);
    assert_eq!(runs[0].count, 2);
}

#[test]
fn main_output_is_preferred_for_frame_rate() {
    let text = MINIMAL.replace(
        "<output name=\"main\">",
        "<output name=\"preview\"><fps>12</fps></output><output name=\"main\">",
    );
    assert_eq!(parse_scene(&text).expect("parses").frame_rate, Some(24.0));
}

#[test]
fn refuses_a_root_that_is_not_tnz() {
    let err = parse_scene("<palette/>").expect_err("wrong root");
    assert!(matches!(err, ToonzError::UnknownFormat(_)));
}

#[test]
fn refuses_broken_xml() {
    assert!(matches!(
        parse_scene("<tnz><levelSet></tnz>"),
        Err(ToonzError::Xml(_))
    ));
    assert!(parse_scene("<tnz>").is_err(), "unclosed root is refused");
}

#[test]
fn refuses_a_cell_that_refers_to_an_undeclared_level() {
    let text = MINIMAL.replace("<cell>0 2 1 1 0</cell>", "<cell>0 2 9 1 0</cell>");
    let err = parse_scene(&text).expect_err("level 9 is not declared");
    assert!(err.to_string().contains("undeclared level 9"), "{err}");
}

#[test]
fn refuses_overlapping_runs_in_one_column() {
    let text = MINIMAL.replace("<cell>2 1 2 3a 0</cell>", "<cell>1 1 2 3a 0</cell>");
    assert!(parse_scene(&text).is_err());
}

#[test]
fn refuses_a_lettered_frame_held_over_rows() {
    let text = MINIMAL.replace("<cell>2 1 2 3a 0</cell>", "<cell>2 2 2 3a 0</cell>");
    assert!(parse_scene(&text).is_err());
}

#[test]
fn refuses_bad_cell_fields() {
    for bad in [
        "<cell>0 2 1 1</cell>",
        "<cell>0 2 1 1 0 9</cell>",
        "<cell>x 2 1 1 0</cell>",
        "<cell>0 0 1 1 0</cell>",
        "<cell>0 2 1 1z 0</cell>",
        "<cell>0 2 1 -1 0</cell>",
    ] {
        let text = MINIMAL.replace("<cell>0 2 1 1 0</cell>", bad);
        assert!(parse_scene(&text).is_err(), "accepted {bad}");
    }
}

#[test]
fn refuses_a_row_past_the_xsheet_limit() {
    let text = MINIMAL.replace("<cell>0 2 1 1 0</cell>", "<cell>999999 2 1 1 0</cell>");
    assert!(matches!(
        parse_scene(&text),
        Err(ToonzError::Limit {
            what: "xsheet rows",
            ..
        })
    ));
}

#[test]
fn refuses_fps_out_of_range_and_duplicate_level_ids() {
    let fps = MINIMAL.replace("<fps>24</fps>", "<fps>0</fps>");
    assert!(parse_scene(&fps).is_err());
    let nan = MINIMAL.replace("<fps>24</fps>", "<fps>NaN</fps>");
    assert!(parse_scene(&nan).is_err());
    let dup = MINIMAL.replace("id=\"2\"", "id=\"1\"");
    assert!(parse_scene(&dup).is_err());
}

#[test]
fn refuses_nesting_past_the_depth_limit() {
    let open = "<x>".repeat(70);
    let close = "</x>".repeat(70);
    let text = format!("<tnz>{open}{close}</tnz>");
    assert!(matches!(
        parse_scene(&text),
        Err(ToonzError::Limit {
            what: "element depth",
            ..
        })
    ));
}

#[test]
fn refuses_a_file_over_the_size_limit() {
    let big = "a".repeat(flare_toonz::MAX_INPUT_BYTES + 1);
    assert!(matches!(
        parse_scene(&big),
        Err(ToonzError::Limit {
            what: "file size",
            ..
        })
    ));
}
