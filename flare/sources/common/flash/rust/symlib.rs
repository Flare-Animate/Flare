//! Flash-style symbol library: folders + symbols (movieclip/graphic/button).
//! Stored as TSV lines `kind\tfolder/path\tname\tlevel` (kind: mc|gr|btn|dir).
#[derive(Debug, Clone, PartialEq)]
pub struct Item { pub kind: String, pub folder: String, pub name: String, pub level: String }

pub fn parse(s: &str) -> Vec<Item> {
    let mut v: Vec<Item> = s.lines().filter_map(|l| {
        let f: Vec<&str> = l.split('\t').collect();
        let k = *f.first()?;
        if !matches!(k, "mc" | "gr" | "btn" | "dir") || f.len() < 3 || f[2].is_empty() { return None; }
        Some(Item { kind: k.into(), folder: f[1].trim_matches('/').into(), name: f[2].into(), level: f.get(3).unwrap_or(&"").to_string() })
    }).collect();
    // stable sort keeps first occurrence first, so dedup keeps the newest (prepended) entry
    v.sort_by(|a, b| (&a.folder, a.kind != "dir", &a.name).cmp(&(&b.folder, b.kind != "dir", &b.name)));
    v.dedup_by(|a, b| a.folder == b.folder && a.name == b.name);
    v
}

pub fn dump(v: &[Item]) -> String {
    v.iter().map(|i| format!("{}\t{}\t{}\t{}\n", i.kind, i.folder, i.name, i.level)).collect()
}

/// Add item (dup folder+name replaced). Returns normalized TSV.
pub fn add(lib: &str, line: &str) -> String {
    let mut v = parse(line);
    v.extend(parse(lib));
    v.sort_by(|a, b| (&a.folder, &a.name).cmp(&(&b.folder, &b.name)));
    v.dedup_by(|a, b| a.folder == b.folder && a.name == b.name);
    dump(&parse(&dump(&v)))
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn roundtrip() {
        let l = add("gr\tchars\tbob\tL1\nbad\n", "mc\t/chars/\twalk\tL2");
        let l = add(&l, "dir\t\tchars\t");
        let v = parse(&l);
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].name, "chars");
        assert_eq!(v[1].name, "bob");
        assert_eq!(v[2].kind, "mc");
        assert_eq!(parse(&add(&l, "btn\tchars\tbob\tL9"))[1].kind, "btn");
    }
}
