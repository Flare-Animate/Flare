//! Layer metadata stored in legacy `Page N` and `Symbol N` streams.

use thiserror::Error;

const LAYER_SIGNATURE: [u8; 10] = [0, 0, 0, 0, 0, 0x80, 0, 0, 0, 0x80];
const MAX_LAYERS: usize = 4096;
const MAX_NAME_CHARS: usize = 255;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LayerError {
    #[error("the stream contains more than {MAX_LAYERS} layers")]
    TooManyLayers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerKind {
    Normal,
    Guide,
    Folder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyLayer {
    pub name: String,
    pub kind: LayerKind,
    pub locked: bool,
    pub visible: bool,
}

fn flash_string(data: &[u8], at: usize) -> Option<(String, usize)> {
    if data.get(at..at.checked_add(3)?)? != [0xFF, 0xFE, 0xFF] {
        return None;
    }
    let len = usize::from(*data.get(at + 3)?);
    if len == 0 || len > MAX_NAME_CHARS {
        return None;
    }
    let end = at.checked_add(4)?.checked_add(len.checked_mul(2)?)?;
    let units: Vec<u16> = data
        .get(at + 4..end)?
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .collect();
    let name = String::from_utf16(&units).ok()?;
    if name.trim().is_empty() || name.chars().any(char::is_control) {
        return None;
    }
    Some((name, end))
}

fn kind(name: &str) -> LayerKind {
    if name.starts_with("Guide: ") {
        LayerKind::Guide
    } else if name
        .strip_prefix("Folder ")
        .is_some_and(|number| !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()))
    {
        LayerKind::Folder
    } else {
        LayerKind::Normal
    }
}

/// Recovers only layer records whose sentinel, schema, name and state bytes agree.
pub fn read(data: &[u8]) -> Result<Vec<LegacyLayer>, LayerError> {
    let mut layers = Vec::new();
    let mut at = 0;
    while at <= data.len().saturating_sub(LAYER_SIGNATURE.len()) {
        if data.get(at..at + LAYER_SIGNATURE.len()) != Some(&LAYER_SIGNATURE) {
            at += 1;
            continue;
        }
        let schema_at = at + LAYER_SIGNATURE.len();
        let Some(&schema) = data.get(schema_at) else {
            break;
        };
        if !(1..=30).contains(&schema) {
            at += 1;
            continue;
        }
        let Some((name, name_end)) = flash_string(data, schema_at + 1) else {
            at += 1;
            continue;
        };

        let (locked, visible) = if schema >= 4 {
            let Some(state) = data.get(name_end..name_end + 3) else {
                at += 1;
                continue;
            };
            if state[1] > 1 || state[2] > 1 {
                at += 1;
                continue;
            }
            (state[1] != 0, state[2] == 0)
        } else {
            (false, true)
        };

        if layers.len() == MAX_LAYERS {
            return Err(LayerError::TooManyLayers);
        }
        layers.push(LegacyLayer {
            kind: kind(&name),
            name,
            locked,
            visible,
        });
        at = name_end;
    }
    Ok(layers)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn record(name: &str, schema: u8, locked: bool, hidden: bool) -> Vec<u8> {
        let mut bytes = LAYER_SIGNATURE.to_vec();
        bytes.push(schema);
        bytes.extend_from_slice(&[0xFF, 0xFE, 0xFF, name.len() as u8]);
        for unit in name.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        if schema >= 4 {
            bytes.extend_from_slice(&[1, u8::from(locked), u8::from(hidden)]);
        }
        bytes
    }

    #[test]
    fn recovers_layer_names_types_visibility_and_lock_state() {
        let mut bytes = record("Layer 1", 11, true, true);
        bytes.extend(record("Guide: Motion", 11, false, false));
        bytes.extend(record("Folder 2", 11, false, false));
        let layers = read(&bytes).expect("layers");
        assert_eq!(
            layers,
            [
                LegacyLayer {
                    name: "Layer 1".to_owned(),
                    kind: LayerKind::Normal,
                    locked: true,
                    visible: false,
                },
                LegacyLayer {
                    name: "Guide: Motion".to_owned(),
                    kind: LayerKind::Guide,
                    locked: false,
                    visible: true,
                },
                LegacyLayer {
                    name: "Folder 2".to_owned(),
                    kind: LayerKind::Folder,
                    locked: false,
                    visible: true,
                },
            ]
        );
    }

    #[test]
    fn ignores_false_signatures_and_rejects_bad_state_flags() {
        let mut bytes = LAYER_SIGNATURE.to_vec();
        bytes.extend([31, 0, 0, 0, 0]);
        bytes.extend(record("Bad flags", 11, false, false));
        *bytes.last_mut().expect("last byte") = 2;
        assert!(read(&bytes).expect("layers").is_empty());
    }

    #[test]
    fn refuses_layer_counts_over_the_limit() {
        let bytes: Vec<u8> = (0..=MAX_LAYERS)
            .flat_map(|index| record(&format!("Layer {index}"), 11, false, false))
            .collect();
        assert_eq!(read(&bytes), Err(LayerError::TooManyLayers));
    }
}
