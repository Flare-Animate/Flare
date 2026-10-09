//! XFL DOM layer/frame parsing (DOMLayer / DOMFrame), mirrors C++ XFL importer naming.

#[derive(Debug, PartialEq, Clone, Default)]
pub struct Frame { pub index: u32, pub duration: u32, pub key_mode: u32 }

#[derive(Debug, PartialEq, Clone, Default)]
pub struct Layer { pub name: String, pub frames: Vec<Frame> }

fn sattr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let k = format!(" {}=\"", name);
    let i = tag.find(&k)? + k.len();
    tag[i..].split('"').next()
}
fn nattr(tag: &str, name: &str, def: u32) -> u32 { sattr(tag, name).and_then(|v| v.parse().ok()).unwrap_or(def) }

/// Layers of the first timeline, in document order. Frames: index, duration (default 1), keyMode.
pub fn layers(d: &[u8]) -> Option<Vec<Layer>> {
    let s = std::str::from_utf8(d).ok()?;
    let mut out: Vec<Layer> = Vec::new();
    let mut p = 0;
    while let Some(i) = s[p..].find('<') {
        let start = p + i;
        let end = start + s[start..].find('>')?;
        let tag = &s[start..end];
        p = end + 1;
        if tag.starts_with("<DOMLayer") && tag[9..].starts_with([' ', '/']) {
            out.push(Layer { name: sattr(tag, "name").unwrap_or("").to_string(), frames: vec![] });
        } else if tag.starts_with("<DOMFrame") && tag[9..].starts_with([' ', '/']) {
            if let Some(l) = out.last_mut() {
                l.frames.push(Frame { index: nattr(tag, "index", 0), duration: nattr(tag, "duration", 1), key_mode: nattr(tag, "keyMode", 0) });
            }
        } else if tag.starts_with("</DOMTimeline") { break; }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn parse() {
        let x = br#"<DOMDocument><timelines><DOMTimeline name="S1"><layers>
<DOMLayer name="bg" color="0x4FFF4F"><frames><DOMFrame index="0" duration="10" keyMode="9728"><elements/></DOMFrame></frames></DOMLayer>
<DOMLayer name="fg"><frames><DOMFrame index="0"/><DOMFrame index="5" duration="3"/></frames></DOMLayer>
</layers></DOMTimeline><DOMTimeline name="S2"><layers><DOMLayer name="x"/></layers></DOMTimeline></timelines></DOMDocument>"#;
        let l = layers(x).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].name, "bg");
        assert_eq!(l[0].frames, vec![Frame { index: 0, duration: 10, key_mode: 9728 }]);
        assert_eq!(l[1].frames[1], Frame { index: 5, duration: 3, key_mode: 0 });
        assert_eq!(l[1].frames[0].duration, 1);
        assert!(layers(&[0xff]).is_none());
    }
}
