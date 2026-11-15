//! Reading what Rhubarb Lip Sync prints with `--exportFormat json`: `mouthCues`, each a start and
//! end in seconds and one of its nine shapes, a letter.

use serde_json::Value;

use super::{LipSyncError, MAX_MOUTH_CUES, MouthCue, Viseme};

/// The most of Rhubarb's output that is read.
pub const MAX_RHUBARB_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

/// One of Rhubarb's shapes (from its README) as one of ours: A closed (M, B, P); B slightly open,
/// teeth together (K, S, T, EE); C open (EH, AE); D wide open (AA); E slightly rounded (AO, ER);
/// F puckered (UW, OW, W); G the F and V shape; H a long L; X idle.
pub fn rhubarb_shape(letter: char) -> Option<Viseme> {
    Some(match letter.to_ascii_uppercase() {
        'A' => Viseme::Mbp,
        'B' => Viseme::Etc,
        'C' => Viseme::E,
        'D' => Viseme::Ai,
        'E' => Viseme::O,
        'F' => Viseme::U,
        'G' => Viseme::Fv,
        'H' => Viseme::L,
        'X' => Viseme::Neutral,
        _ => return None,
    })
}

/// The cues of Rhubarb's JSON output. Untrusted: its size, the number of cues and every time are
/// checked, and an unknown shape is an error rather than a guess.
pub fn parse_rhubarb(text: &[u8]) -> Result<Vec<MouthCue>, LipSyncError> {
    let fail = |why: &str| LipSyncError::Rhubarb(why.to_owned());
    if text.len() > MAX_RHUBARB_OUTPUT_BYTES {
        return Err(fail("it is too long"));
    }
    let value: Value =
        serde_json::from_slice(text).map_err(|e| LipSyncError::Rhubarb(e.to_string()))?;
    let cues = value
        .get("mouthCues")
        .and_then(Value::as_array)
        .ok_or_else(|| fail("it has no mouthCues"))?;
    if cues.len() > MAX_MOUTH_CUES {
        return Err(fail("it has too many cues"));
    }
    cues.iter()
        .map(|cue| {
            let time = |key: &str| {
                cue.get(key)
                    .and_then(Value::as_f64)
                    .filter(|t| t.is_finite() && *t >= 0.0)
                    .ok_or_else(|| fail("a cue has no usable start or end"))
            };
            let (start, end) = (time("start")?, time("end")?);
            if end < start {
                return Err(fail("a cue ends before it starts"));
            }
            let viseme = cue
                .get("value")
                .and_then(Value::as_str)
                .filter(|s| s.chars().count() == 1)
                .and_then(|s| s.chars().next())
                .and_then(rhubarb_shape)
                .ok_or_else(|| fail("a cue has a mouth shape Flare does not know"))?;
            Ok(MouthCue { start, end, viseme })
        })
        .collect()
}
