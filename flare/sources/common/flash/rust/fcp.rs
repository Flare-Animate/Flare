//! FlipaClip .fcp (zip: frame PNGs + project json) parser. Zip read by hand (stored/deflate).
use miniz_oxide::inflate::decompress_to_vec;
use serde_json::{json, Value};

fn u16le(d: &[u8], o: usize) -> Option<usize> {
    Some(u16::from_le_bytes(d.get(o..o + 2)?.try_into().ok()?) as usize)
}
fn u32le(d: &[u8], o: usize) -> Option<usize> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?) as usize)
}

/// (name, bytes) of each zip entry; stored or deflate only.
pub fn entries(d: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let e = d
        .windows(4)
        .rposition(|w| w == b"PK\x05\x06")
        .ok_or("no zip")?;
    let (n, mut p) = (
        u16le(d, e + 10).ok_or("eocd")?,
        u32le(d, e + 16).ok_or("eocd")?,
    );
    let mut out = vec![];
    for _ in 0..n {
        if d.get(p..p + 4) != Some(b"PK\x01\x02") {
            return Err("bad cd".into());
        }
        let (m, csz, nl, xl, cl, lh) = (
            u16le(d, p + 10).ok_or("cd")?,
            u32le(d, p + 20).ok_or("cd")?,
            u16le(d, p + 28).ok_or("cd")?,
            u16le(d, p + 30).ok_or("cd")?,
            u16le(d, p + 32).ok_or("cd")?,
            u32le(d, p + 42).ok_or("cd")?,
        );
        let name = String::from_utf8_lossy(d.get(p + 46..p + 46 + nl).ok_or("name")?).into_owned();
        let ds = lh + 30 + u16le(d, lh + 26).ok_or("lh")? + u16le(d, lh + 28).ok_or("lh")?;
        let raw = d.get(ds..ds + csz).ok_or("data")?;
        let data = match m {
            0 => raw.to_vec(),
            8 => decompress_to_vec(raw).map_err(|_| "inflate")?,
            _ => return Err("method".into()),
        };
        out.push((name, data));
        p += 46 + nl + xl + cl;
    }
    Ok(out)
}

fn num(v: &Value, ks: &[&str]) -> Option<f64> {
    ks.iter().find_map(|k| v.get(k)?.as_f64())
}

/// JSON summary: {fps,width,height,frames:[png names, natural order]}.
pub fn parse(d: &[u8]) -> Result<String, String> {
    let es = entries(d)?;
    let mut frames: Vec<&str> = es
        .iter()
        .map(|e| e.0.as_str())
        .filter(|n| n.to_lowercase().ends_with(".png"))
        .collect();
    frames.sort_by_key(|n| {
        let s: String = n
            .rsplit('/')
            .next()
            .unwrap_or(n)
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect();
        (s.parse::<u64>().unwrap_or(u64::MAX), n.to_string())
    });
    let meta: Value = es
        .iter()
        .find(|e| e.0.to_lowercase().ends_with(".json"))
        .and_then(|e| serde_json::from_slice(&e.1).ok())
        .unwrap_or(Value::Null);
    Ok(json!({
        "fps": num(&meta, &["fps", "frameRate", "frame_rate"]).unwrap_or(12.0),
        "width": num(&meta, &["width", "canvasWidth"]).unwrap_or(0.0),
        "height": num(&meta, &["height", "canvasHeight"]).unwrap_or(0.0),
        "frames": frames,
    })
    .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn zip(fs: &[(&str, &[u8])]) -> Vec<u8> {
        let (mut o, mut cd) = (vec![], vec![]);
        for (n, b) in fs {
            let off = o.len() as u32;
            o.extend(b"PK\x03\x04\x0a\0");
            o.extend([0u8; 12]);
            o.extend((b.len() as u32).to_le_bytes());
            o.extend((b.len() as u32).to_le_bytes());
            o.extend((n.len() as u16).to_le_bytes());
            o.extend([0, 0]);
            o.extend(n.as_bytes());
            o.extend(*b);
            cd.extend(b"PK\x01\x02\x14\0\x0a\0");
            cd.extend([0u8; 12]);
            cd.extend((b.len() as u32).to_le_bytes());
            cd.extend((b.len() as u32).to_le_bytes());
            cd.extend((n.len() as u16).to_le_bytes());
            cd.extend([0u8; 12]);
            cd.extend(off.to_le_bytes());
            cd.extend(n.as_bytes());
        }
        let (co, cl) = (o.len() as u32, cd.len() as u32);
        o.extend(cd);
        o.extend(b"PK\x05\x06\0\0\0\0");
        o.extend((fs.len() as u16).to_le_bytes());
        o.extend((fs.len() as u16).to_le_bytes());
        o.extend(cl.to_le_bytes());
        o.extend(co.to_le_bytes());
        o.extend([0, 0]);
        o
    }
    #[test]
    fn fcp() {
        let z = zip(&[
            ("frames/10.png", b"x"),
            ("frames/2.png", b"y"),
            ("project.json", br#"{"fps":24,"width":1280,"height":720}"#),
        ]);
        let v: Value = serde_json::from_str(&parse(&z).unwrap()).unwrap();
        assert_eq!(v["fps"], 24.0);
        assert_eq!(v["width"], 1280.0);
        assert_eq!(v["frames"], json!(["frames/2.png", "frames/10.png"]));
        assert!(parse(b"junk").is_err());
    }
}
