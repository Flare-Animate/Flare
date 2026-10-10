//! Moho (.moho zip / .mohoproj json) parser: layers, groups, switch layers, bones, keyframes + interpolation.
use serde_json::{json, Value};

fn u16le(d: &[u8], o: usize) -> Option<usize> { Some(u16::from_le_bytes(d.get(o..o + 2)?.try_into().ok()?) as usize) }
fn u32le(d: &[u8], o: usize) -> Option<usize> { Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?) as usize) }

/// Extract Project.mohoproj / Project.animeproj from a ZIP (central directory walk).
fn unzip(d: &[u8]) -> Option<Vec<u8>> {
    let eocd = (0..d.len().saturating_sub(21)).rev().take(70000).find(|&i| d[i..i + 4] == [0x50, 0x4b, 5, 6])?;
    let (n, mut p) = (u16le(d, eocd + 10)?, u32le(d, eocd + 16)?);
    for _ in 0..n {
        if d.get(p..p + 4)? != [0x50, 0x4b, 1, 2] { return None; }
        let (method, csize, nl, el, cl, lo) = (u16le(d, p + 10)?, u32le(d, p + 20)?, u16le(d, p + 28)?, u16le(d, p + 30)?, u16le(d, p + 32)?, u32le(d, p + 42)?);
        let name = d.get(p + 46..p + 46 + nl)?;
        if name == b"Project.mohoproj" || name == b"Project.animeproj" {
            let start = lo + 30 + u16le(d, lo + 26)? + u16le(d, lo + 28)?;
            let raw = d.get(start..start + csize)?;
            return match method { 0 => Some(raw.to_vec()), 8 => miniz_oxide::inflate::decompress_to_vec(raw).ok(), _ => None };
        }
        p += 46 + nl + el + cl;
    }
    None
}

const INTERP: [&str; 6] = ["linear", "smooth", "ease_in", "ease_out", "step", "noise"];
fn interp_name(im: i64) -> &'static str { INTERP.get(im as usize).copied().unwrap_or("other") }

/// A channel is any object with parallel `when` + `val` arrays.
fn channel(owner: &str, name: &str, o: &Value, out: &mut Vec<Value>) {
    let (Some(w), Some(v)) = (o["when"].as_array(), o["val"].as_array()) else { return };
    let ip = o["interp"].as_array();
    let keys: Vec<Value> = w.iter().enumerate().map(|(i, f)| {
        let im = ip.and_then(|a| a.get(i)).and_then(|x| x["im"].as_i64()).unwrap_or(0);
        json!({"frame": f, "value": v.get(i).unwrap_or(&Value::Null), "interp": interp_name(im)})
    }).collect();
    out.push(json!({"owner": owner, "name": name, "type": o["type"], "muted": o["mute"].as_bool().unwrap_or(false), "keys": keys}));
}

fn scan(owner: &str, name: &str, v: &Value, out: &mut Vec<Value>) {
    match v {
        Value::Object(m) => {
            if m.contains_key("when") && m.contains_key("val") { channel(owner, name, v, out); return; }
            for (k, c) in m { if k != "layers" { scan(owner, k, c, out); } }
        }
        Value::Array(a) => for c in a { scan(owner, name, c, out); },
        _ => {}
    }
}

fn layer(l: &Value, parent: i64, layers: &mut Vec<Value>, bones: &mut Vec<Value>, tracks: &mut Vec<Value>) {
    let (name, ty) = (l["name"].as_str().unwrap_or(""), l["type"].as_str().unwrap_or(""));
    let id = layers.len() as i64;
    let kids = l["layers"].as_array();
    let alts: Vec<&str> = if ty == "SwitchLayer" { kids.into_iter().flatten().filter_map(|c| c["name"].as_str()).collect() } else { vec![] };
    layers.push(json!({"id": id, "parent": parent, "name": name, "type": ty, "uuid": l["uuid"], "visible": l["visible"].as_bool().unwrap_or(true),
        "parent_bone": l["parent_bone"].as_i64().unwrap_or(-2), "image_path": l["image_path"], "alternatives": alts}));
    if let Some(sk) = l["skeleton"]["bones"].as_array() {
        for (i, b) in sk.iter().enumerate() {
            let bn = b["name"].as_str().unwrap_or("");
            bones.push(json!({"layer": id, "index": i, "name": bn, "parent": b["parent"].as_i64().unwrap_or(-1), "length": b["length"].as_f64().unwrap_or(0.0)}));
            scan(&format!("{name}/{bn}"), "", b, tracks);
        }
    }
    for (k, v) in l.as_object().into_iter().flatten() { if k != "layers" && k != "skeleton" { scan(name, k, v, tracks); } }
    for c in kids.into_iter().flatten() { layer(c, id, layers, bones, tracks); }
}

/// Parse .moho (zip) or bare .mohoproj into a normalised JSON document.
pub fn parse(d: &[u8]) -> Result<String, String> {
    let body;
    let raw: &[u8] = if d.starts_with(b"PK") { body = unzip(d).ok_or("no Project.mohoproj in archive")?; &body } else { d };
    let root: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    if !root["mime_type"].as_str().unwrap_or("lm_mohodoc").contains("lm_mohodoc") { return Err("not a Moho document".into()); }
    let (mut layers, mut bones, mut tracks) = (vec![], vec![], vec![]);
    for l in root["layers"].as_array().into_iter().flatten() { layer(l, -1, &mut layers, &mut bones, &mut tracks); }
    scan("", "", &root["animated_values"], &mut tracks);
    let nkeys: usize = tracks.iter().map(|t| t["keys"].as_array().map_or(0, |k| k.len())).sum();
    let p = &root["project_data"];
    Ok(json!({"version": root["version"], "width": p["width"], "height": p["height"], "fps": p["fps"],
        "start_frame": p["start_frame"], "end_frame": p["end_frame"],
        "layers": layers, "bones": bones, "tracks": tracks, "keyframe_count": nkeys}).to_string())
}

#[cfg(test)]
mod tests {
    const DIR: &str = "C:/Users/charl/Downloads/Flare Samples/online/moho/";
    fn load(f: &str) -> Option<serde_json::Value> {
        let d = std::fs::read(format!("{DIR}{f}")).ok()?; // skip when sample absent
        Some(serde_json::from_str(&super::parse(&d).unwrap()).unwrap())
    }
    #[test]
    fn charwalk_both_forms_agree() {
        let (Some(a), Some(b)) = (load("CharWalk.moho"), load("CharWalk.mohoproj")) else { return };
        assert_eq!(a, b);
        assert_eq!(a["layers"][0]["type"], "BoneLayer");
        assert_eq!(a["layers"][0]["name"], "CharRig");
        assert_eq!(a["layers"].as_array().unwrap().len(), 21);
        assert!(a["layers"].as_array().unwrap().iter().any(|l| l["type"] == "GroupLayer" && l["name"] == "HeadGroup"));
        assert!(a["bones"].as_array().unwrap().len() > 10);
        assert_eq!(a["bones"][1]["name"], "Head");
        assert!(a["keyframe_count"].as_u64().unwrap() > 50);
        assert!(a["tracks"].as_array().unwrap().iter().any(|t| t["name"] == "anim_pos" && t["keys"].as_array().unwrap().len() == 8));
    }
    #[test]
    fn rejects_garbage() { assert!(super::parse(b"hello").is_err()); }
}
