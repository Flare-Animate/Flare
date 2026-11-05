//! Keeps the status table, the `TODO(viewspace:<id>)` comments and the document in step.

use super::*;

const SOURCES: &[(&str, &str)] = &[
    ("clothing", include_str!("clothing.rs")),
    ("confidence", include_str!("confidence.rs")),
    ("edit", include_str!("edit.rs")),
    ("export", include_str!("export.rs")),
    ("expression", include_str!("expression.rs")),
    ("features", include_str!("features.rs")),
    ("fields", include_str!("fields.rs")),
    ("head", include_str!("head.rs")),
    ("interp", include_str!("interp.rs")),
    ("keyframe", include_str!("keyframe.rs")),
    ("magic", include_str!("magic.rs")),
    ("mesh", include_str!("mesh.rs")),
    ("mod", include_str!("mod.rs")),
    ("neighbours", include_str!("neighbours.rs")),
    ("project", include_str!("project.rs")),
    ("quality", include_str!("quality.rs")),
];

const DOC: &str = include_str!("../../../../../../docs/experimental/VIEWSPACE.md");

fn todo_ids() -> Vec<String> {
    let mut ids = Vec::new();
    for (_, src) in SOURCES {
        for line in src.lines() {
            if let Some(rest) = line.trim_start().strip_prefix("// TODO(viewspace:") {
                ids.extend(rest.split(')').next().map(str::to_string));
            }
        }
    }
    ids
}

#[test]
fn ids_are_unique() {
    let mut seen = std::collections::BTreeSet::new();
    for s in STATUS {
        assert!(seen.insert(s.id), "duplicate id {}", s.id);
    }
}

#[test]
fn every_stub_has_exactly_one_todo_and_nothing_else_does() {
    let todos = todo_ids();
    for s in STATUS {
        let n = todos.iter().filter(|t| *t == s.id).count();
        // ui-tab lives in the module doc, which the scan above does not see as a TODO comment.
        let expected = usize::from(!s.done && s.id != "ui-tab");
        assert_eq!(n, expected, "{} (done: {})", s.id, s.done);
    }
    for t in &todos {
        assert!(
            STATUS.iter().any(|s| s.id == t),
            "TODO id {t} is not in STATUS"
        );
    }
}

#[test]
fn the_document_lists_every_id_with_its_status() {
    for s in STATUS {
        let row = DOC
            .lines()
            .find(|l| l.starts_with(&format!("| `{}` |", s.id)));
        let Some(row) = row else {
            panic!("{} is missing from docs/experimental/VIEWSPACE.md", s.id)
        };
        assert!(
            row.contains(if s.done { "done" } else { "stub" }),
            "wrong status for {}: {row}",
            s.id
        );
    }
}

#[test]
fn every_source_file_is_short() {
    for (name, src) in SOURCES {
        assert!(
            src.lines().count() < 500,
            "{name}.rs has {} lines",
            src.lines().count()
        );
    }
}

#[test]
fn errors_display() {
    assert_eq!(
        ViewSpaceError::NotImplemented("x").to_string(),
        "not implemented: x"
    );
    assert!(
        ViewSpaceError::Limit { what: "w", max: 3 }
            .to_string()
            .contains("at most 3")
    );
}
