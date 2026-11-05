//! Decoder for the 4-bit IMA ADPCM tracks of a PPM: a 4-byte block header (predictor, step index,
//! one unused byte), then two samples a byte, low nibble first. Output is 16-bit PCM at the
//! track's own rate, which the caller takes from the BGM and effect speeds.

const STEP_TABLE: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449,
    494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272,
    2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493,
    10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];

const INDEX_STEP: [i32; 8] = [-1, -1, -1, -1, 2, 4, 6, 8];

/// Decodes a whole track. An empty or short track gives no samples; a block header alone is
/// accepted as silence.
pub fn decode(track: &[u8]) -> Vec<i16> {
    let (Some(&lo), Some(&hi)) = (track.first(), track.get(1)) else {
        return Vec::new();
    };
    let mut predictor = i32::from(i16::from_le_bytes([lo, hi]));
    let mut index = i32::from(track.get(2).copied().unwrap_or(0)).min(88);
    let body = track.get(4..).unwrap_or_default();
    let mut out = Vec::with_capacity(body.len() * 2);
    for &byte in body {
        for nibble in [byte & 0xF, byte >> 4] {
            let step = STEP_TABLE.get(index as usize).copied().unwrap_or(1);
            let mut diff = step >> 3;
            if nibble & 1 != 0 {
                diff += step >> 2;
            }
            if nibble & 2 != 0 {
                diff += step >> 1;
            }
            if nibble & 4 != 0 {
                diff += step;
            }
            if nibble & 8 != 0 {
                diff = -diff;
            }
            predictor = (predictor + diff).clamp(-32768, 32767);
            index = (index
                + INDEX_STEP
                    .get(usize::from(nibble & 7))
                    .copied()
                    .unwrap_or(0))
            .clamp(0, 88);
            out.push(predictor as i16);
        }
    }
    out
}
