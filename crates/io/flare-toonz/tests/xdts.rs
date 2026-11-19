#![allow(clippy::expect_used)]
//! XDTS timing-sheet tests on small scenes built in these tests and sheet text written by hand.

use flare_toonz::{
    Cell, CellRun, Column, FrameId, IDENTIFIER, LevelDecl, MAX_XDTS_ROWS, Scene, TimingSheet,
    ToonzError, Track, apply, export, parse,
};

fn level(id: u32, name: &str) -> LevelDecl {
    LevelDecl {
        id,
        name: name.to_owned(),
        kind: "vector".to_owned(),
        path: None,
    }
}

fn run(row: u32, count: u32, level: u32, number: u32, letter: Option<char>) -> CellRun {
    CellRun {
        row,
        count,
        level,
        frame: FrameId { number, letter },
        increment: 0,
    }
}

fn cell(frame: u32, text: Option<&str>) -> Cell {
    Cell {
        frame,
        cell: text.map(str::to_owned),
    }
}

/// Column A holds drawing 1 for three rows and drawing 2 for two rows after a blank row; column B
/// shows the lettered drawing 3a on row 2 only.
fn two_column_scene() -> Scene {
    Scene {
        levels: vec![level(1, "A"), level(2, "B")],
        columns: vec![
            Column::Level(vec![run(0, 3, 1, 1, None), run(4, 2, 1, 2, None)]),
            Column::Level(vec![run(2, 1, 2, 3, Some('a'))]),
        ],
        ..Scene::default()
    }
}

#[test]
fn export_writes_a_track_per_column_with_the_exposure_changes() {
    let text = export(&two_column_scene());
    assert_eq!(text.lines().next(), Some(IDENTIFIER));
    assert!(text.contains("\"version\": 5"), "{text}");
    let sheet = parse(&text).expect("the export parses");
    assert_eq!(sheet.name, "Scene 1");
    assert_eq!(sheet.duration, 6);
    assert_eq!(
        sheet.tracks,
        vec![
            Track {
                name: "A".to_owned(),
                cells: vec![cell(0, Some("1")), cell(3, None), cell(4, Some("2"))],
            },
            Track {
                name: "B".to_owned(),
                cells: vec![cell(2, Some("3a")), cell(3, None)],
            },
        ]
    );
}

#[test]
fn a_round_trip_rebuilds_the_cell_runs() {
    let sheet = parse(&export(&two_column_scene())).expect("the export parses");
    let mut target = Scene {
        levels: vec![level(1, "A"), level(2, "B")],
        columns: vec![Column::Level(Vec::new()), Column::Level(Vec::new())],
        ..Scene::default()
    };
    apply(&sheet, &mut target);
    assert_eq!(
        target.columns,
        vec![
            Column::Level(vec![run(0, 3, 1, 1, None), run(4, 2, 1, 2, None)]),
            Column::Level(vec![run(2, 1, 2, 3, Some('a'))]),
        ]
    );
}

#[test]
fn apply_leaves_unmatched_columns_and_tracks_alone() {
    let mut scene = Scene {
        levels: vec![level(1, "A")],
        columns: vec![
            Column::Level(vec![run(0, 2, 1, 7, None)]),
            Column::Other {
                tag: "fxColumn".to_owned(),
            },
        ],
        ..Scene::default()
    };
    let sheet = TimingSheet {
        name: "S".to_owned(),
        duration: 3,
        tracks: vec![
            Track {
                name: "Z".to_owned(),
                cells: vec![cell(0, Some("9"))],
            },
            Track {
                name: "A".to_owned(),
                cells: vec![cell(1, Some("2"))],
            },
        ],
    };
    apply(&sheet, &mut scene);
    assert_eq!(
        scene.columns[0],
        Column::Level(vec![run(1, 2, 1, 2, None)]),
        "only the named level column changes, and its own level id is kept"
    );
    assert_eq!(
        scene.columns[1],
        Column::Other {
            tag: "fxColumn".to_owned()
        },
        "a column that is not a level column is untouched"
    );
}

#[test]
fn refuses_a_file_without_the_identifier_line() {
    let err = parse("exchangeDigitalTimeSheet Old Data\n{\"timeTables\": []}")
        .expect_err("the identifier does not match");
    assert!(matches!(err, ToonzError::UnknownFormat(_)), "{err}");
    assert!(err.to_string().contains("XDTS"), "{err}");
}

#[test]
fn refuses_broken_json() {
    let text = format!("{IDENTIFIER}\n{{\"timeTables\": [}}");
    let err = parse(&text).expect_err("the JSON is broken");
    assert!(
        matches!(err, ToonzError::Malformed { file: "xdts", .. }),
        "{err}"
    );
    assert!(err.to_string().contains("JSON"), "{err}");
}

#[test]
fn refuses_a_duration_past_the_limit() {
    let duration = MAX_XDTS_ROWS + 1;
    let text = format!(
        "{IDENTIFIER}\n{{\"header\": {{}}, \"version\": 5, \"timeTables\": [{{\"duration\": \
         {duration}, \"name\": \"S\", \"timeTableHeaders\": [{{\"fieldId\": 0, \"names\": []}}], \
         \"fields\": [{{\"fieldId\": 0, \"tracks\": []}}]}}]}}"
    );
    let err = parse(&text).expect_err("the duration is over the limit");
    assert!(
        matches!(err, ToonzError::Limit { what: "timing sheet rows", max } if max == MAX_XDTS_ROWS as usize),
        "{err}"
    );
}

#[test]
fn refuses_a_drawing_number_past_the_frame_limit() {
    let text = format!(
        "{IDENTIFIER}\n{{\"header\": {{}}, \"version\": 5, \"timeTables\": [{{\"duration\": 4, \
         \"name\": \"S\", \"timeTableHeaders\": [{{\"fieldId\": 0, \"names\": [\"A\"]}}], \
         \"fields\": [{{\"fieldId\": 0, \"tracks\": [{{\"trackNo\": 0, \"frames\": [{{\"frame\": \
         0, \"cell\": \"1000001\"}}]}}]}}]}}]}}"
    );
    let err = parse(&text).expect_err("the drawing number is over the limit");
    assert!(
        matches!(
            err,
            ToonzError::Limit {
                what: "frame number",
                max: 1_000_000
            }
        ),
        "{err}"
    );
}

#[test]
fn refuses_a_cell_that_is_not_a_drawing() {
    let text = format!(
        "{IDENTIFIER}\n{{\"header\": {{}}, \"version\": 5, \"timeTables\": [{{\"duration\": 4, \
         \"name\": \"S\", \"timeTableHeaders\": [{{\"fieldId\": 0, \"names\": [\"A\"]}}], \
         \"fields\": [{{\"fieldId\": 0, \"tracks\": [{{\"trackNo\": 0, \"frames\": [{{\"frame\": \
         0, \"cell\": \"SYMBOL_TICK_1\"}}]}}]}}]}}]}}"
    );
    let err = parse(&text).expect_err("a tick is not a drawing");
    assert!(
        matches!(err, ToonzError::Malformed { file: "xdts", .. }),
        "{err}"
    );
}
