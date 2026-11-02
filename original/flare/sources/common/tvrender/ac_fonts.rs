//! System fonts for text (font picker + outline loading). Installs a loader
//! into `ac_core::text` so documents can name any installed family.

use std::sync::OnceLock;

fn db() -> &'static fontdb::Database {
    static DB: OnceLock<fontdb::Database> = OnceLock::new();
    DB.get_or_init(|| {
        let mut d = fontdb::Database::new();
        d.load_system_fonts();
        if let Some(dir) = std::env::var_os("AC_FONT_DIR") {
            d.load_fonts_dir(dir);
        }
        d
    })
}

fn load(family: &str, weight: u16, italic: bool) -> Option<(Vec<u8>, u16, bool)> {
    let d = db();
    let q = fontdb::Query { families: &[fontdb::Family::Name(family)], weight: fontdb::Weight(weight), style: if italic { fontdb::Style::Italic } else { fontdb::Style::Normal }, ..Default::default() };
    let id = d.query(&q)?;
    let info = d.face(id)?;
    let (w, i) = (info.weight.0, info.style != fontdb::Style::Normal);
    let mut out = None;
    // ab_glyph reads the first face of a collection; good enough for .ttc defaults
    d.with_face_data(id, |data, _index| out = Some(data.to_vec()));
    out.map(|b| (b, w, i))
}

/// The faces a family has, as (style name, weight, italic), lightest first.
pub fn styles(family: &str) -> Vec<(String, u16, bool)> {
    let mut v: Vec<(String, u16, bool)> = db()
        .faces()
        .filter(|f| f.families.iter().any(|(n, _)| n == family))
        .map(|f| {
            let w = f.weight.0;
            let it = f.style != fontdb::Style::Normal;
            (style_name(w, it), w, it)
        })
        .collect();
    v.sort_by_key(|s| (s.1, s.2));
    v.dedup_by(|a, b| a.1 == b.1 && a.2 == b.2);
    v
}

/// "Bold Italic", "Light", "Regular"… for a weight.
pub fn style_name(weight: u16, italic: bool) -> String {
    let w = match weight {
        0..=149 => "Thin",
        150..=249 => "Extra Light",
        250..=349 => "Light",
        350..=449 => "Regular",
        450..=549 => "Medium",
        550..=649 => "Semibold",
        650..=749 => "Bold",
        750..=849 => "Extra Bold",
        _ => "Black",
    };
    match (w, italic) {
        ("Regular", true) => "Italic".into(),
        (w, true) => format!("{w} Italic"),
        (w, false) => w.into(),
    }
}

/// Make named font families available to text outlines (call once at start-up).
pub fn install_system_fonts() {
    ac_core::text::set_font_loader(load);
}

/// Installed font family names, sorted.
pub fn families() -> &'static [String] {
    static LIST: OnceLock<Vec<String>> = OnceLock::new();
    LIST.get_or_init(|| {
        let mut v: Vec<String> = db().faces().filter_map(|f| f.families.first().map(|(n, _)| n.clone())).collect();
        v.sort_by_key(|s| s.to_lowercase());
        v.dedup();
        v
    })
}

/// Installed fonts that cover characters the bundled UI fonts lack (CJK,
/// full-width punctuation such as the ＂ Windows puts in file names, symbols),
/// best first: (name, font file bytes, face index). For the editor's UI only.
pub fn ui_fallback_fonts() -> Vec<(String, Vec<u8>, u32)> {
    use ab_glyph::Font as _;
    const WANT: [char; 6] = ['＂', '漢', 'あ', '한', '★', '→'];
    const PREFERRED: [&str; 16] = [
        "Segoe UI",
        "Microsoft YaHei UI",
        "Microsoft YaHei",
        "Yu Gothic UI",
        "Meiryo",
        "Malgun Gothic",
        "Segoe UI Symbol",
        "Noto Sans CJK SC",
        "Noto Sans CJK JP",
        "Noto Sans CJK TC",
        "Noto Sans CJK KR",
        "Noto Sans",
        "Source Han Sans",
        "PingFang SC",
        "Hiragino Sans",
        "DejaVu Sans",
    ];
    let d = db();
    let mut covered = [false; WANT.len()];
    let mut out = vec![];
    for fam in PREFERRED {
        if covered.iter().all(|c| *c) || out.len() >= 3 {
            break;
        }
        let q = fontdb::Query { families: &[fontdb::Family::Name(fam)], ..Default::default() };
        let Some(id) = d.query(&q) else { continue };
        let mut found = None;
        d.with_face_data(id, |data, index| {
            if let Ok(f) = ab_glyph::FontRef::try_from_slice_and_index(data, index) {
                let adds: Vec<usize> = WANT.iter().enumerate().filter(|(i, c)| !covered[*i] && f.glyph_id(**c).0 != 0).map(|(i, _)| i).collect();
                if !adds.is_empty() {
                    found = Some((data.to_vec(), index, adds));
                }
            }
        });
        if let Some((bytes, index, adds)) = found {
            for i in adds {
                covered[i] = true;
            }
            out.push((fam.to_string(), bytes, index));
        }
    }
    out
}

#[cfg(test)]
mod fallback_tests {
    #[test]
    fn fallback_fonts_cover_fullwidth_when_installed() {
        use ab_glyph::Font as _;
        let fonts = super::ui_fallback_fonts();
        assert!(fonts.len() <= 3);
        let any_installed = std::process::Command::new("fc-list").arg(":charset=ff02").output().map(|o| {
            let t = String::from_utf8_lossy(&o.stdout).into_owned();
            ["Noto Sans CJK", "YaHei", "Yu Gothic", "Meiryo", "Malgun", "Source Han", "PingFang", "Hiragino", "Segoe UI"].iter().any(|n| t.contains(n))
        }).unwrap_or(false);
        // Flare adaptation: only meaningful when one of the preferred UI families is installed
        // (a host with only unifont / wqy-zenhei yields no fallback candidates at all).
        if any_installed {
            let ok = fonts.iter().any(|(_, b, i)| ab_glyph::FontRef::try_from_slice_and_index(b, *i).is_ok_and(|f| f.glyph_id('＂').0 != 0));
            assert!(ok, "no fallback covers U+FF02: {:?}", fonts.iter().map(|f| &f.0).collect::<Vec<_>>());
        }
    }
}
