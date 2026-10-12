//! Minimal reader for XFL `edges` attributes (the vector data inside
//! `DOMDocument.xml` / `LIBRARY/*.xml` of an uncompressed .fla).
//!
//! Grammar as understood so far (EXPERIMENTAL — needs a corpus of real files):
//! - coordinates are twips (1/20 px), either decimal (`-22.5`) or
//!   hex fixed-point `#XX.YY` (signed 32-bit integer part, 8-bit fraction)
//! - `!x y` move-to, `|x y` or `/x y` line-to, `[cx cy x y` or `]cx cy x y` quad-to
//! - `S<n>` selection markers, ignored
//! - `(`…`;` cubic hint blocks live in a separate `cubics` attribute, ignored here

use crate::geom::{pt, Path, Seg};

pub fn parse_edges(s: &str) -> Result<Path, String> {
    let mut segs = Vec::new();
    let mut chars = s.chars().peekable();
    let mut pen: Option<crate::geom::Pt> = None;

    fn skip_ws(it: &mut std::iter::Peekable<std::str::Chars>) {
        while matches!(it.peek(), Some(c) if c.is_whitespace()) {
            it.next();
        }
    }
    fn number(it: &mut std::iter::Peekable<std::str::Chars>) -> Result<f32, String> {
        skip_ws(it);
        let mut tok = String::new();
        if it.peek() == Some(&'#') {
            it.next();
            while let Some(&c) = it.peek() {
                if c.is_ascii_hexdigit() || c == '.' {
                    tok.push(c);
                    it.next();
                } else {
                    break;
                }
            }
            let (int, frac) = tok.split_once('.').unwrap_or((&tok, "0"));
            let iv =
                u32::from_str_radix(int, 16).map_err(|e| format!("bad hex '{tok}': {e}"))? as i32;
            let fv = u32::from_str_radix(if frac.is_empty() { "0" } else { frac }, 16)
                .map_err(|e| format!("bad hex '{tok}': {e}"))?;
            return Ok((iv as f32 + fv as f32 / 256.0) / 20.0);
        }
        while let Some(&c) = it.peek() {
            if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e' || c == 'E' {
                tok.push(c);
                it.next();
            } else {
                break;
            }
        }
        tok.parse::<f32>()
            .map(|v| v / 20.0)
            .map_err(|_| format!("expected number, got '{tok}'"))
    }

    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {}
            '!' => {
                let p = pt(number(&mut chars)?, number(&mut chars)?);
                // consecutive edges usually re-state the pen position; don't start a new subpath then
                if pen.map(|q| q.dist(p) > 1e-3).unwrap_or(true) {
                    segs.push(Seg::M(p));
                }
                pen = Some(p);
            }
            '|' | '/' => {
                let p = pt(number(&mut chars)?, number(&mut chars)?);
                segs.push(Seg::L(p));
                pen = Some(p);
            }
            '[' | ']' => {
                let c = pt(number(&mut chars)?, number(&mut chars)?);
                let p = pt(number(&mut chars)?, number(&mut chars)?);
                segs.push(Seg::Q(c, p));
                pen = Some(p);
            }
            'S' => {
                while matches!(chars.peek(), Some(d) if d.is_ascii_digit()) {
                    chars.next();
                }
            }
            other => return Err(format!("unexpected '{other}' in edges")),
        }
    }
    Ok(Path { segs })
}
