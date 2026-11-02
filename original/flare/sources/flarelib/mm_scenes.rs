//! The Scene panel's commands: duplicate, delete, rename and reorder scenes. Insert > Scene is
//! `Document::insert_scene`.

use crate::Result;
use crate::doc::Document;

/// Duplicate Scene: a copy of scene `i`, "<name> copy", right after it. Returns where it went.
pub fn duplicate(doc: &mut Document, i: usize) -> usize {
    let mut copy = doc.scenes[i].clone();
    let base = format!("{} copy", copy.name);
    let taken = |name: &String| doc.scenes.iter().any(|s| &s.name == name);
    copy.name = (1..).map(|n| if n == 1 { base.clone() } else { format!("{base} {n}") }).find(|n| !taken(n)).unwrap_or(base);
    doc.scenes.insert(i + 1, copy);
    i + 1
}

/// Deletes scene `i`. A document keeps one scene at least.
pub fn delete(doc: &mut Document, i: usize) -> Result<()> {
    if doc.scenes.len() < 2 {
        return Err("A document needs one scene at least.".into());
    }
    doc.scenes.remove(i);
    Ok(())
}

/// Renames scene `i`. A name is needed, and one no other scene has.
pub fn rename(doc: &mut Document, i: usize, name: &str) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err("A scene needs a name.".into());
    }
    if doc.scenes.iter().enumerate().any(|(j, s)| j != i && s.name == name) {
        return Err(format!("There is already a scene named \"{name}\".").into());
    }
    doc.scenes[i].name = name.to_string();
    Ok(())
}

/// Moves scene `from` into the gap before scene `gap` (`scenes.len()` is after the last), as a drag
/// in the Scene panel drops it. Returns where it went.
pub fn move_to_gap(doc: &mut Document, from: usize, gap: usize) -> usize {
    let scene = doc.scenes.remove(from);
    let at = if gap > from { gap - 1 } else { gap }.min(doc.scenes.len());
    doc.scenes.insert(at, scene);
    at
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(doc: &Document) -> Vec<&str> {
        doc.scenes.iter().map(|s| s.name.as_str()).collect()
    }

    #[test]
    fn duplicate_delete_rename_and_reorder() {
        let mut doc = Document::new(550.0, 400.0, 24.0);
        assert!(delete(&mut doc, 0).is_err(), "the last scene stays");
        doc.scenes[0].layers[0].frames[0].duration = 5;
        assert_eq!(duplicate(&mut doc, 0), 1);
        assert_eq!(duplicate(&mut doc, 0), 1);
        assert_eq!(names(&doc), ["Scene 1", "Scene 1 copy 2", "Scene 1 copy"]);
        assert_eq!(doc.scenes[2].frame_count(), 5, "a copy holds the same frames");

        assert!(rename(&mut doc, 1, "Scene 1").is_err() && rename(&mut doc, 1, "  ").is_err());
        rename(&mut doc, 1, " Intro ").unwrap();
        rename(&mut doc, 1, "Intro").unwrap();
        assert_eq!(names(&doc), ["Scene 1", "Intro", "Scene 1 copy"]);

        // Dragged into the gap after the last, then back to the top.
        assert_eq!(move_to_gap(&mut doc, 0, 3), 2);
        assert_eq!(names(&doc), ["Intro", "Scene 1 copy", "Scene 1"]);
        assert_eq!(move_to_gap(&mut doc, 2, 0), 0);
        assert_eq!(move_to_gap(&mut doc, 1, 1), 1, "its own gap leaves it be");
        assert_eq!(names(&doc), ["Scene 1", "Intro", "Scene 1 copy"]);

        delete(&mut doc, 1).unwrap();
        assert_eq!(names(&doc), ["Scene 1", "Scene 1 copy"]);
    }
}
