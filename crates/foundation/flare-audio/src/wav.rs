//! Writing 16-bit PCM as a WAV file, the one form every speech tool reads.

/// `samples` (interleaved, `channels` of them per frame) at `rate` as the bytes of a WAV file.
/// `None` when the data would not fit a WAV's 32-bit sizes or `channels` is zero.
pub fn wav_pcm16(samples: &[i16], rate: u32, channels: u16) -> Option<Vec<u8>> {
    if channels == 0 {
        return None;
    }
    let data_len = u32::try_from(samples.len().checked_mul(2)?).ok()?;
    let riff_len = data_len.checked_add(36)?;
    let block = channels.checked_mul(2)?;
    let byte_rate = rate.checked_mul(u32::from(block))?;
    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_len.to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block.to_le_bytes());
    out.extend_from_slice(&16_u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    Some(out)
}
