//! XDTS timing sheets ("exchangeDigitalTimeSheet", read and written by Clip
//! Studio Paint, OpenToonz, TVPaint and other exposure-sheet tools): export,
//! and import onto layers ([`parse`], [`apply`]).
//!
//! Format as written by OpenToonz's `xdtsio.cpp` (BSD-3-Clause, © DWANGO and
//! OpenToonz contributors; see THIRD-PARTY-NOTICES.md): an identifier line,
//! then JSON with one time table whose cell field has a track per layer.
//!
//! Mapping: each normal layer (bottom → top = columns left → right) is a track
//! named after the layer; each keyframe with content is a drawing numbered
//! 1, 2, 3… in time order; blank keyframes and frames past the layer's end are
//! empty cells. Tweened frames show their keyframe's drawing (key timing).

use crate::model::*;
use serde_json::{json, Value};

pub const IDENTIFIER: &str = "exchangeDigitalTimeSheet Save Data";
const NULL_CELL: &str = "SYMBOL_NULL_CELL";

/// The XDTS text for the current scene `tl` of `doc`.
pub fn export(doc: &Document, tl: &Timeline) -> String {
    let duration = tl.frame_count();
    let layers: Vec<&Layer> = tl.layers.iter().rev().filter(|l| l.kind == LayerKind::Normal).collect();
    let names: Vec<String> = layers.iter().map(|l| l.name.clone()).collect();
    let tracks: Vec<Value> = layers.iter().enumerate().map(|(n, l)| json!({ "trackNo": n, "frames": track_frames(l, duration) })).collect();
    let data = json!({
        "header": { "cut": "1", "scene": "1" },
        "timeTables": [{
            "duration": duration,
            "name": if tl.name.is_empty() { doc_name(doc) } else { tl.name.clone() },
            "timeTableHeaders": [{ "fieldId": 0, "names": names }],
            "fields": [{ "fieldId": 0, "tracks": tracks }]
        }],
        "version": 5
    });
    format!("{IDENTIFIER}\n{}\n", serde_json::to_string_pretty(&data).unwrap_or_default())
}

fn doc_name(_doc: &Document) -> String {
    "Scene 1".into()
}

/// Cell changes of one layer: (frame, value) where value is a drawing number
/// or the null cell.
fn track_frames(l: &Layer, duration: u32) -> Vec<Value> {
    let mut out = vec![];
    let mut number = 0u32;
    let mut last: Option<String> = None;
    let mut push = |f: u32, v: String, last: &mut Option<String>| {
        if last.as_deref() != Some(v.as_str()) {
            out.push(json!({ "frame": f, "data": [{ "id": 0, "values": [v.clone()] }] }));
            *last = Some(v);
        }
    };
    if l.keyframes.keys().next().is_some_and(|&k| k > 0) {
        push(0, NULL_CELL.into(), &mut last);
    }
    for (&start, kf) in &l.keyframes {
        if start >= l.end.min(duration) {
            break;
        }
        let v = if kf.elements.is_empty() {
            NULL_CELL.to_string()
        } else {
            number += 1;
            number.to_string()
        };
        push(start, v, &mut last);
    }
    if l.end < duration {
        push(l.end, NULL_CELL.into(), &mut last);
    }
    out
}

// ------------------------------------------------------------------ import
//
// Reading follows OpenToonz's `xdtsio.cpp` / `xdtsimportpopup.cpp` semantics:
// a track's frame entries set the cell from that frame on; SYMBOL_NULL_CELL
// empties it; SYMBOL_HYPHEN, SYMBOL_TICK_1/2 (inbetween marks) keep the
// previous drawing; an entry at a negative frame (written by some exporters)
// sets the drawing shown before the first entry; a track whose last cell holds
// a drawing keeps it to the end of the sheet. Column n is named names[n].

/// A timing sheet read from an .xdts file.
#[derive(Clone, Debug, PartialEq)]
pub struct Sheet {
    pub duration: u32,
    pub tracks: Vec<SheetTrack>,
}

/// One column: its name and the drawing shown on each frame (None = empty).
#[derive(Clone, Debug, PartialEq)]
pub struct SheetTrack {
    pub name: String,
    pub cells: Vec<Option<String>>,
}

/// Drawing names compare as numbers when they are numbers ("001" = "1").
pub fn drawing_key(s: &str) -> String {
    let t = s.trim();
    t.parse::<u64>().map(|n| n.to_string()).unwrap_or_else(|_| t.to_string())
}

/// Read an .xdts file's text.
pub fn parse(text: &str) -> Result<Sheet, String> {
    let body = text.trim_start_matches('\u{feff}').strip_prefix(IDENTIFIER).ok_or("not an XDTS file (missing the exchangeDigitalTimeSheet identifier)")?;
    let v: Value = serde_json::from_str(body.trim()).map_err(|e| format!("XDTS: {e}"))?;
    let tt = v["timeTables"].get(0).ok_or("XDTS: no time table")?;
    let duration = tt["duration"].as_u64().ok_or("XDTS: no duration")?.max(1) as u32;
    let field = |arr: &Value| arr.as_array().and_then(|a| a.iter().find(|h| h["fieldId"].as_u64() == Some(0)).cloned());
    let names: Vec<String> = field(&tt["timeTableHeaders"]).and_then(|h| h["names"].as_array().cloned()).unwrap_or_default().iter().map(|n| n.as_str().unwrap_or("").trim().to_string()).collect();
    let mut tracks = vec![];
    let Some(cells) = field(&tt["fields"]) else { return Ok(Sheet { duration, tracks }) };
    let mut list: Vec<&Value> = cells["tracks"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    list.sort_by_key(|t| t["trackNo"].as_i64().unwrap_or(0));
    for t in list {
        let no = t["trackNo"].as_u64().unwrap_or(0) as usize;
        let name = names.get(no).cloned().filter(|n| !n.is_empty()).unwrap_or_else(|| format!("Track {}", no + 1));
        let mut entries: Vec<(i64, String)> = t["frames"]
            .as_array()
            .map(|a| a.iter().filter_map(|f| Some((f["frame"].as_i64()?, f["data"].get(0)?["values"].get(0)?.as_str()?.to_string()))).collect())
            .unwrap_or_default();
        entries.sort_by_key(|e| e.0);
        let mut out: Vec<Option<String>> = vec![];
        let mut initial: Option<String> = None;
        for (f, v) in entries {
            if f < 0 {
                initial = cell(&v, &initial);
                continue;
            }
            let f = (f as usize).min(duration as usize);
            while out.len() < f {
                out.push(out.last().cloned().unwrap_or_else(|| initial.clone()));
            }
            if out.len() > f {
                continue; // two entries on one frame: the first wins
            }
            let prev = out.last().cloned().unwrap_or_else(|| initial.clone());
            out.push(cell(&v, &prev));
        }
        let last = out.last().cloned().flatten();
        out.resize(duration as usize, last);
        out.truncate(duration as usize);
        tracks.push(SheetTrack { name, cells: out });
    }
    Ok(Sheet { duration, tracks })
}

fn cell(v: &str, prev: &Option<String>) -> Option<String> {
    match v {
        NULL_CELL => None,
        "SYMBOL_HYPHEN" | "SYMBOL_TICK_1" | "SYMBOL_TICK_2" => prev.clone(),
        d => Some(drawing_key(d)),
    }
}

/// Where a track's drawings come from.
#[derive(Clone, Debug, PartialEq)]
pub enum TrackSource {
    /// The layer of the same name: drawing n = its n-th keyframe with content
    /// (as [`export`] numbers them), or a keyframe labelled with the name.
    Layer(usize),
    /// Images, drawing name → library bitmap (a new layer).
    Bitmaps(Vec<(String, crate::bitmap::BitmapId)>),
    /// Nothing to show yet: a new layer of blank keyframes labelled with the
    /// drawing names (comment labels), ready to draw into.
    Timing,
}

/// What [`apply`] did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportReport {
    pub retimed: Vec<String>,
    pub from_images: Vec<String>,
    pub timing_only: Vec<String>,
    /// "track: drawing" pairs that had no drawing (left blank, labelled).
    pub missing: Vec<String>,
}

/// The layer of the current timeline a track maps onto (same name, ignoring case).
pub fn layer_for(tl: &Timeline, name: &str) -> Option<usize> {
    tl.layers.iter().position(|l| l.kind == LayerKind::Normal && l.name.trim().eq_ignore_ascii_case(name.trim()))
}

/// The drawings of a layer by name: "1", "2"… for keyframes with content in
/// time order, plus each content keyframe's label.
pub fn layer_drawings(l: &Layer) -> Vec<(String, Keyframe)> {
    let mut out = vec![];
    for (n, kf) in l.keyframes.values().filter(|k| !k.elements.is_empty()).enumerate() {
        out.push(((n + 1).to_string(), kf.clone()));
        if let Some(lb) = kf.label.as_ref().filter(|s| !s.trim().is_empty()) {
            out.push((drawing_key(lb), kf.clone()));
        }
    }
    out
}

/// Expose `sheet` on the current timeline: each track re-times the layer it
/// names, or becomes a new layer (from `sources`, default [`TrackSource::Timing`]
/// or the same-named layer). One undo step. New layers go on top, in column order.
pub fn apply(ed: &mut crate::editor::Editor, sheet: &Sheet, sources: &[(String, TrackSource)]) -> ImportReport {
    let mut rep = ImportReport::default();
    ed.begin_edit("Import XDTS");
    let (w, h) = (ed.doc.width, ed.doc.height);
    let mut plans = vec![];
    for t in &sheet.tracks {
        let src = sources.iter().find(|(n, _)| n == &t.name).map(|s| s.1.clone()).or_else(|| layer_for(ed.timeline(), &t.name).map(TrackSource::Layer)).unwrap_or(TrackSource::Timing);
        let drawings: Vec<(String, Keyframe)> = match &src {
            TrackSource::Layer(i) => ed.timeline().layers.get(*i).map(layer_drawings).unwrap_or_default(),
            TrackSource::Bitmaps(list) => list
                .iter()
                .filter_map(|(d, id)| {
                    let b = ed.doc.bitmap(*id)?;
                    // centred on the stage (a camera-sized drawing lands at 0,0)
                    let m = crate::geom::Matrix::translate(((w - b.width as f32) / 2.0).round(), ((h - b.height as f32) / 2.0).round());
                    Some((drawing_key(d), Keyframe::with(vec![Element::bitmap(&b.name, *id, m)])))
                })
                .collect(),
            TrackSource::Timing => vec![],
        };
        let mut keys = std::collections::BTreeMap::new();
        let mut f = 0usize;
        while f < t.cells.len() {
            let c = &t.cells[f];
            let start = f;
            while f < t.cells.len() && &t.cells[f] == c {
                f += 1;
            }
            let kf = match c {
                None => Keyframe::blank(),
                Some(d) => match drawings.iter().find(|(n, _)| n == d) {
                    Some((_, k)) => Keyframe { tween: Tween::None, motion: None, ease_curve: None, shape_hints: vec![], sound: None, script: None, ..k.clone() },
                    None => {
                        if !matches!(src, TrackSource::Timing) {
                            rep.missing.push(format!("{}: {d}", t.name));
                        }
                        Keyframe { label: Some(d.clone()), label_kind: LabelKind::Comment, ..Keyframe::blank() }
                    }
                },
            };
            keys.insert(start as u32, kf);
        }
        if keys.is_empty() {
            keys.insert(0, Keyframe::blank());
        }
        plans.push((t.name.clone(), src, keys));
    }
    // re-time existing layers first (their indices are still valid), then add new ones on top
    for (name, src, keys) in &plans {
        if let TrackSource::Layer(i) = src {
            rep.retimed.push(name.clone());
            let l = &mut ed.timeline_mut().layers[*i];
            l.keyframes = keys.clone();
            l.end = sheet.duration;
        }
    }
    for (name, src, keys) in plans {
        if matches!(src, TrackSource::Layer(_)) {
            continue;
        }
        let n = ed.timeline().layers.len();
        let mut l = Layer::new(&name, n);
        l.keyframes = keys;
        l.end = sheet.duration;
        for x in ed.timeline_mut().layers.iter_mut() {
            x.parent = x.parent.map(|p| p + 1);
            x.anim_parent = x.anim_parent.map(|p| p + 1);
        }
        ed.timeline_mut().layers.insert(0, l);
        if matches!(src, TrackSource::Bitmaps(_)) {
            rep.from_images.push(name);
        } else {
            rep.timing_only.push(name);
        }
    }
    ed.layer = 0;
    ed.selection.clear();
    rep
}

/// Images for a track named `name` in `dir`: files `name_0001.png`,
/// `name.0001.png`, `name0001.png` (any separator) next to the sheet, or
/// `0001.png`-style files in a folder `name/`. Drawing name = the number.
pub fn find_images(dir: &std::path::Path, name: &str) -> Vec<(String, std::path::PathBuf)> {
    let image = |p: &std::path::Path| {
        p.extension().and_then(|e| e.to_str()).is_some_and(|e| ["png", "jpg", "jpeg", "gif", "bmp", "tga", "tif", "tiff", "webp", "exr"].contains(&e.to_ascii_lowercase().as_str()))
    };
    let number = |s: &str| (!s.is_empty() && s.chars().all(|c| c.is_ascii_digit())).then(|| drawing_key(s));
    let mut out: Vec<(String, std::path::PathBuf)> = vec![];
    let lname = name.trim().to_lowercase();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for p in rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| image(p)) {
            let stem = p.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
            if let Some(rest) = stem.strip_prefix(&lname) {
                if let Some(n) = number(rest.trim_start_matches(['_', '.', '-', ' '])) {
                    out.push((n, p));
                }
            }
        }
    }
    let sub = dir.join(name.trim());
    if out.is_empty() && sub.is_dir() {
        if let Ok(rd) = std::fs::read_dir(&sub) {
            for p in rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| image(p)) {
                let stem = p.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
                let digits: String = stem.chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
                if let Some(n) = number(&digits) {
                    out.push((n, p));
                }
            }
        }
    }
    out.sort_by(|a, b| (a.0.parse::<u64>().ok(), &a.0).cmp(&(b.0.parse::<u64>().ok(), &b.0)));
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

/// Import an .xdts file: tracks named like layers re-time them; other tracks
/// load numbered images found next to the sheet (see [`find_images`]) onto
/// new layers, or become timing-only layers. One undo step.
#[cfg(feature = "codecs")]
pub fn import_file(ed: &mut crate::editor::Editor, path: &std::path::Path) -> Result<ImportReport, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let sheet = parse(&text)?;
    let dir = path.parent().map(|d| d.to_path_buf()).unwrap_or_default();
    let mark = ed.undo_depth();
    let mut sources = vec![];
    for t in &sheet.tracks {
        if layer_for(ed.timeline(), &t.name).is_some() {
            continue;
        }
        let files = find_images(&dir, &t.name);
        if files.is_empty() {
            continue;
        }
        // only the drawings the sheet uses
        let used: std::collections::HashSet<&String> = t.cells.iter().flatten().collect();
        let mut list = vec![];
        for (d, f) in files.iter().filter(|(d, _)| used.contains(d)) {
            let asset = crate::codecs::load_bitmap(f)?;
            list.push((d.clone(), ed.add_bitmap(asset)));
        }
        sources.push((t.name.clone(), TrackSource::Bitmaps(list)));
    }
    let rep = apply(ed, &sheet, &sources);
    ed.squash_undo(mark, "Import XDTS");
    Ok(rep)
}

impl ImportReport {
    /// One line for the status bar / command result.
    pub fn summary(&self) -> String {
        let mut parts = vec![];
        if !self.retimed.is_empty() {
            parts.push(format!("re-timed {}", self.retimed.join(", ")));
        }
        if !self.from_images.is_empty() {
            parts.push(format!("images for {}", self.from_images.join(", ")));
        }
        if !self.timing_only.is_empty() {
            parts.push(format!("timing only (labelled blank keyframes) for {}", self.timing_only.join(", ")));
        }
        if !self.missing.is_empty() {
            parts.push(format!("{} drawing(s) not found: {}", self.missing.len(), self.missing.iter().take(6).cloned().collect::<Vec<_>>().join(", ")));
        }
        if parts.is_empty() {
            "the sheet has no tracks".into()
        } else {
            format!("XDTS: {}", parts.join("; "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Path;

    #[test]
    fn layers_become_tracks_and_keys_become_drawings() {
        let mut d = Document::default();
        let sq = Element::shape("s", Shape::new(Path::rect(0.0, 0.0, 5.0, 5.0), Some(Color::rgb(0, 0, 0)), None));
        let mut a = Layer::new("A", 0);
        a.keyframes.insert(0, Keyframe::with(vec![sq.clone()]));
        a.keyframes.insert(3, Keyframe::blank());
        a.keyframes.insert(5, Keyframe::with(vec![sq.clone()]));
        a.end = 8;
        let mut b = Layer::new("B", 1);
        b.keyframes.clear();
        b.keyframes.insert(2, Keyframe::with(vec![sq]));
        b.end = 10;
        d.scene.layers = vec![b, a]; // top: B
        let text = export(&d, &d.scene);
        assert!(text.starts_with(IDENTIFIER));
        let v: Value = serde_json::from_str(text.split_once('\n').unwrap().1).unwrap();
        assert_eq!(v["version"], 5);
        let t = &v["timeTables"][0];
        assert_eq!(t["duration"], 10);
        assert_eq!(t["timeTableHeaders"][0]["names"], json!(["A", "B"]), "bottom layer first");
        let frames = |n: usize| t["fields"][0]["tracks"][n]["frames"].as_array().unwrap().iter().map(|f| (f["frame"].as_u64().unwrap(), f["data"][0]["values"][0].as_str().unwrap().to_string())).collect::<Vec<_>>();
        assert_eq!(frames(0), vec![(0, "1".into()), (3, NULL_CELL.into()), (5, "2".into()), (8, NULL_CELL.into())]);
        assert_eq!(frames(1), vec![(0, NULL_CELL.into()), (2, "1".into())]);
    }
}
