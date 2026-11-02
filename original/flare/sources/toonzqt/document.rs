//! AnimationCraft owns document edits/history. Flare owns the workspace and file identity.
use ac_core::{
    Color, Document, Element, Keyframe, Layer, Matrix, Path, Shape, SymbolKind, Timeline, Tween,
};
use std::{
    io::{Read, Write},
    path::Path as FilePath,
};

pub const MAX_FRAMES: u32 = 10_000;

pub fn validate(doc: &Document) -> Result<(), String> {
    if !doc.width.is_finite()
        || !doc.height.is_finite()
        || !doc.fps.is_finite()
        || !(1.0..=4096.0).contains(&doc.width)
        || !(1.0..=4096.0).contains(&doc.height)
        || !(1.0..=120.0).contains(&doc.fps)
        || doc.library.len() > 256
        || doc.more_scenes.len() > 64
    {
        return Err("error-document".into());
    }
    for tl in doc.scenes().chain(doc.library.iter().map(|s| &s.timeline)) {
        if tl.layers.is_empty() || tl.layers.len() > 256 {
            return Err("error-document".into());
        }
        for (index, layer) in tl.layers.iter().enumerate() {
            if layer.end == 0
                || ![100, 200, 300].contains(&layer.row_height)
                || layer.end > MAX_FRAMES
                || layer.keyframes.len() > MAX_FRAMES as usize
                || layer.keyframes.keys().any(|f| *f >= layer.end)
                || layer.parent.is_some_and(|p| p >= index)
                || layer
                    .anim_parent
                    .is_some_and(|p| p >= tl.layers.len() || p == index)
            {
                return Err("error-document".into());
            }
            let mut parent = layer.anim_parent;
            for key in layer.keyframes.values() {
                if let Some(boundary) = &key.tween_boundary {
                    if boundary.duration == 0
                        || boundary.duration > MAX_FRAMES
                        || !boundary.time_end.is_finite()
                        || !boundary.time_start.is_finite()
                        || boundary.time_start < 0.0
                        || boundary.time_end <= boundary.time_start
                        || boundary.time_end > boundary.duration as f32
                        || key.tween == Tween::None
                        || boundary.pose_edits.len() > MAX_FRAMES as usize
                        || boundary.pose_edits.iter().any(|e| {
                            !e.finite()
                                || e.element >= key.elements.len()
                                || e.time < 0.0
                                || e.time > boundary.duration as f32
                                || e.end.abs() > boundary.duration as f32
                        })
                    {
                        return Err("error-document".into());
                    }
                }
            }
            let mut visited = vec![index];
            while let Some(p) = parent {
                if visited.contains(&p) {
                    return Err("error-document".into());
                }
                visited.push(p);
                parent = tl.layers.get(p).and_then(|l| l.anim_parent);
            }
        }
    }
    Ok(())
}

pub fn open(path: &FilePath) -> Result<Document, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("error-document".into());
    }
    let doc: Document = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    validate(&doc)?;
    Ok(doc)
}

pub fn save(doc: &Document, path: &FilePath) -> Result<(), String> {
    validate(doc)?;
    let bytes =
        serde_json::to_vec_pretty(&doc.with_floats_committed()).map_err(|e| e.to_string())?;
    atomic_write(path, &bytes)
}

pub fn atomic_write(path: &FilePath, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| FilePath::new("."));
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.as_file().sync_all())
        .map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn blank(scene: &str, layer: &str) -> Document {
    let mut doc = Document {
        width: 550.0,
        height: 400.0,
        background: Color::rgb(255, 255, 255),
        ..Default::default()
    };
    doc.scene.name = scene.into();
    doc.scene.layers[0].name = layer.into();
    doc.scene.layers[0].end = 1;
    doc
}

fn shape(name: &str, path: Path, color: Color) -> Element {
    Element::shape(name, Shape::new(path, Some(color), None))
}

/// Original fixture artwork; names are authored content, not interface messages.
pub fn example() -> Document {
    let mut doc = blank("Orbit", "Orbit paths");
    doc.width = 960.0;
    doc.height = 540.0;
    doc.background = Color::rgb(242, 236, 222);
    let mut background = Layer::new("Atmosphere", 2);
    background.end = 72;
    background.locked = true;
    background.keyframes.insert(
        0,
        Keyframe::with(vec![
            shape(
                "Field",
                Path::ellipse(480.0, 285.0, 315.0, 198.0),
                Color::rgb(221, 225, 218),
            ),
            shape(
                "Halo",
                Path::ellipse(480.0, 270.0, 192.0, 192.0),
                Color::rgb(233, 192, 154),
            ),
            shape(
                "Sun",
                Path::ellipse(480.0, 270.0, 154.0, 154.0),
                Color::rgb(235, 112, 74),
            ),
            shape(
                "Light",
                Path::ellipse(430.0, 218.0, 64.0, 64.0),
                Color::rgb(248, 144, 96),
            ),
        ]),
    );
    let mut symbol = Timeline::new("Satellite");
    symbol.layers[0].keyframes.insert(
        0,
        Keyframe::with(vec![
            shape(
                "Satellite",
                Path::ellipse(0.0, 0.0, 26.0, 26.0),
                Color::rgb(60, 77, 109),
            ),
            shape(
                "Highlight",
                Path::ellipse(-8.0, -8.0, 7.0, 7.0),
                Color::rgb(139, 166, 190),
            ),
        ]),
    );
    let id = doc.add_symbol("Satellite", SymbolKind::Graphic, symbol);
    let mut orbit = Layer::new("Satellites", 0);
    orbit.end = 72;
    for (frame, angle) in [
        (0, 0.0f32),
        (24, 2.094),
        (48, 4.189),
        (71, std::f32::consts::TAU),
    ] {
        let mut key = Keyframe::with(vec![
            Element::instance(
                "Satellite A",
                id,
                Matrix::translate(480.0 + angle.cos() * 275.0, 270.0 + angle.sin() * 160.0),
            ),
            Element::instance(
                "Satellite B",
                id,
                Matrix::translate(480.0 - angle.cos() * 245.0, 270.0 - angle.sin() * 160.0)
                    .concat(&Matrix::scale(0.55, 0.55)),
            ),
        ]);
        key.tween = Tween::Classic {
            ease: 0.0,
            rotate_turns: 0,
        };
        orbit.keyframes.insert(frame, key);
    }
    let mut accents = Layer::new("Constellation", 1);
    accents.end = 72;
    accents.keyframes.insert(
        0,
        Keyframe::with(vec![
            shape(
                "Star",
                Path::star(190.0, 142.0, 15.0, 4.0, 4),
                Color::rgb(70, 85, 93),
            ),
            shape(
                "Star",
                Path::star(784.0, 369.0, 19.0, 5.0, 4),
                Color::rgb(70, 85, 93),
            ),
            shape(
                "Dot",
                Path::ellipse(732.0, 139.0, 5.0, 5.0),
                Color::rgb(235, 112, 74),
            ),
            shape(
                "Dot",
                Path::ellipse(203.0, 398.0, 7.0, 7.0),
                Color::rgb(235, 112, 74),
            ),
        ]),
    );
    doc.scene.layers = vec![orbit, accents, background];
    doc
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editable_symbols_roundtrip_and_undo() {
        let mut editor = ac_core::Editor::new(example());
        let original = editor.doc.clone();
        editor.frame = 12;
        editor.insert_keyframe();
        assert!(editor.timeline().layers[0].keyframes.contains_key(&12));
        editor.undo();
        assert_eq!(editor.doc, original);
        editor.redo();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.acx.json");
        save(&editor.doc, &path).unwrap();
        assert_eq!(open(&path).unwrap(), editor.doc);
        save(&original, &path).unwrap();
        assert_eq!(open(&path).unwrap(), original);
    }
    #[test]
    fn reject_bad_dimensions_and_cyclic_parents() {
        let mut doc = example();
        doc.width = f32::INFINITY;
        assert!(validate(&doc).is_err());
        doc.width = 960.0;
        doc.scene.layers[0].anim_parent = Some(1);
        doc.scene.layers[1].anim_parent = Some(0);
        assert!(validate(&doc).is_err());
    }

    #[test]
    fn validate_retained_tween_time_domain_and_legacy_default() {
        let mut doc = example();
        let clip = ac_core::frames::copy_frames(&doc.scene.layers[0], 0, 7);
        doc.scene.layers[0].keyframes = clip.keyframes.into_iter().collect();
        doc.scene.layers[0].end = clip.len;
        assert!(validate(&doc).is_ok());
        for end in [0.0, -1.0, f32::NAN, f32::INFINITY, 25.0] {
            let mut bad = doc.clone();
            bad.scene.layers[0]
                .keyframes
                .get_mut(&0)
                .unwrap()
                .tween_boundary
                .as_mut()
                .unwrap()
                .time_end = end;
            assert!(validate(&bad).is_err());
        }
        let mut bad = doc.clone();
        for start in [-1.0, 8.0, 25.0, f32::NAN, f32::INFINITY] {
            let mut bad = doc.clone();
            bad.scene.layers[0]
                .keyframes
                .get_mut(&0)
                .unwrap()
                .tween_boundary
                .as_mut()
                .unwrap()
                .time_start = start;
            assert!(validate(&bad).is_err());
        }
        bad.scene.layers[0]
            .keyframes
            .get_mut(&0)
            .unwrap()
            .tween_boundary
            .as_mut()
            .unwrap()
            .duration = 0;
        assert!(validate(&bad).is_err());
        let old = serde_json::to_string(&Keyframe::blank()).unwrap();
        assert!(!old.contains("tween_boundary"));
        assert!(serde_json::from_str::<Keyframe>(&old)
            .unwrap()
            .tween_boundary
            .is_none());
        let old_boundary = r#"{"duration":12,"time_end":8.0,"elements":[],"shape_hints":[]}"#;
        let restored: ac_core::TweenBoundary = serde_json::from_str(old_boundary).unwrap();
        assert_eq!(restored.time_start, 0.0);
        assert!(!restored.reversed);
        assert!(restored.pose_edits.is_empty());
        let mut edited = doc.clone();
        edited.scene.layers[0]
            .keyframes
            .get_mut(&0)
            .unwrap()
            .tween_boundary
            .as_mut()
            .unwrap()
            .pose_edits
            .push(ac_core::tween_edit::TweenPoseEdit {
                element: 0,
                time: 3.0,
                end: 8.0,
                matrix: [0.0; 6],
                alpha: 0.1,
                fill: None,
            });
        assert!(validate(&edited).is_ok());
        for case in 0..6 {
            let mut bad = edited.clone();
            let edit = &mut bad.scene.layers[0]
                .keyframes
                .get_mut(&0)
                .unwrap()
                .tween_boundary
                .as_mut()
                .unwrap()
                .pose_edits[0];
            match case {
                0 => edit.element = usize::MAX,
                1 => edit.end = edit.time,
                2 => edit.time = f32::NAN,
                3 => edit.matrix[0] = f32::INFINITY,
                4 => edit.fill = Some([f32::NAN; 4]),
                _ => edit.end = -100_000.0,
            }
            assert!(validate(&bad).is_err());
        }
    }
}
