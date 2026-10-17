//! Optional speech-based lip sync through Rhubarb Lip Sync
//! (github.com/DanielSWolf/rhubarb-lip-sync, MIT), run as a separate program
//! the way OpenToonz and Flare use it. Nothing of Rhubarb is bundled: if the
//! `rhubarb` executable is installed (on PATH, next to Flare, or in
//! `AC_RHUBARB`), Auto Lip-Sync can use it; otherwise the built-in heuristic.
//!
//! Rhubarb's mouth shapes (from its README) map onto ours:
//! A closed (M B P) → MBP · B slightly open, teeth (K S T EE…) → etc ·
//! C open (EH AE) → E · D wide open (AA) → Ai · E slightly rounded (AO ER) → O ·
//! F puckered (UW OW W) → U · G (F V) → FV · H (long L) → L · X idle → rest.

use ac_core::sound::Viseme;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

static USE_RHUBARB: AtomicBool = AtomicBool::new(true);

/// Prefer Rhubarb when it's installed (default on).
pub fn set_preferred(on: bool) {
    USE_RHUBARB.store(on, Ordering::Relaxed);
}
pub fn preferred() -> bool {
    USE_RHUBARB.load(Ordering::Relaxed)
}

/// The Rhubarb executable: `AC_RHUBARB`, next to this program, or on PATH.
pub fn find() -> Option<PathBuf> {
    let name = if cfg!(windows) { "rhubarb.exe" } else { "rhubarb" };
    let mut cands: Vec<PathBuf> = vec![];
    if let Some(p) = std::env::var_os("AC_RHUBARB") {
        cands.push(p.into());
    }
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.to_path_buf())) {
        cands.push(dir.join(name));
        cands.push(dir.join("rhubarb").join(name));
    }
    if let Some(path) = std::env::var_os("PATH") {
        cands.extend(std::env::split_paths(&path).map(|d| d.join(name)));
    }
    cands.into_iter().find(|p| p.is_file())
}

pub fn shape(c: char) -> Option<Viseme> {
    Some(match c.to_ascii_uppercase() {
        'A' => Viseme::Mbp,
        'B' => Viseme::Etc,
        'C' => Viseme::E,
        'D' => Viseme::Ai,
        'E' => Viseme::O,
        'F' => Viseme::U,
        'G' => Viseme::Fv,
        'H' => Viseme::L,
        'X' => Viseme::Rest,
        _ => return None,
    })
}

/// `mouthCues` from Rhubarb's JSON output: (start s, end s, shape).
pub fn parse_json(text: &str) -> Result<Vec<(f32, f32, Viseme)>, String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("Rhubarb output: {e}"))?;
    let cues = v.get("mouthCues").and_then(|c| c.as_array()).ok_or("Rhubarb output has no mouthCues")?;
    cues.iter()
        .map(|c| {
            let start = c.get("start").and_then(|x| x.as_f64()).ok_or("cue without start")? as f32;
            let end = c.get("end").and_then(|x| x.as_f64()).ok_or("cue without end")? as f32;
            let s = c.get("value").and_then(|x| x.as_str()).and_then(|s| s.chars().next()).and_then(shape).ok_or("unknown mouth shape")?;
            Ok((start, end, s))
        })
        .collect()
}

/// One mouth shape per frame: frame `i` shows the cue at `t0 + i / fps`
/// (times relative to the start of the analysed audio).
pub fn frames(cues: &[(f32, f32, Viseme)], fps: f32, n: usize) -> Vec<Viseme> {
    (0..n)
        .map(|i| {
            let t = i as f32 / fps.max(1.0);
            cues.iter().find(|c| t >= c.0 && t < c.1).map(|c| c.2).unwrap_or(Viseme::Rest)
        })
        .collect()
}

/// Run Rhubarb on `n` frames of `p` from `t0` seconds at `fps`.
pub fn visemes(exe: &std::path::Path, p: &crate::Pcm, t0: f32, fps: f32, n: usize) -> Result<Vec<Viseme>, String> {
    let rate = p.rate;
    let a = ((t0.max(0.0) * rate as f32) as usize).min(p.frames());
    let b = (a + ((n as f32 / fps.max(1.0)) * rate as f32) as usize).min(p.frames());
    let wav = crate::wav(&p.samples[a * 2..b * 2], rate);
    let dir = std::env::temp_dir().join(format!("ac-rhubarb-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let input = dir.join("voice.wav");
    std::fs::write(&input, wav).map_err(|e| e.to_string())?;
    let out = std::process::Command::new(exe).arg("-f").arg("json").arg("--quiet").arg(&input).output().map_err(|e| format!("couldn't run Rhubarb: {e}"))?;
    let _ = std::fs::remove_dir_all(&dir);
    if !out.status.success() {
        return Err(format!("Rhubarb failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(frames(&parse_json(&String::from_utf8_lossy(&out.stdout))?, fps, n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rhubarb_cues_map_to_mouth_frames() {
        let json = r#"{"metadata":{"soundFile":"v.wav","duration":0.5},"mouthCues":[
            {"start":0.00,"end":0.10,"value":"X"},{"start":0.10,"end":0.20,"value":"A"},
            {"start":0.20,"end":0.35,"value":"D"},{"start":0.35,"end":0.50,"value":"F"}]}"#;
        let cues = parse_json(json).unwrap();
        assert_eq!(cues.len(), 4);
        let f = frames(&cues, 20.0, 10);
        assert_eq!(f, vec![Viseme::Rest, Viseme::Rest, Viseme::Mbp, Viseme::Mbp, Viseme::Ai, Viseme::Ai, Viseme::Ai, Viseme::U, Viseme::U, Viseme::U]);
        assert!(parse_json("{}").is_err());
        assert_eq!(shape('g'), Some(Viseme::Fv));
    }
}
