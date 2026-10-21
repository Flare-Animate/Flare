//! Port of `common/flash/FDTSounds.cpp`: the SWF sound tags.
//!
//! The classes stay in `FDTSounds.h` with inline members: `FDTDefineSound*` owns the PCM/ADPCM
//! buffer (`std::vector<U8>`, filled through the already ported `FSoundComp`, whose bytes the
//! tag then references as *large data*), `FDTDefineSoundWAV` reads a caller-owned `FILE *`
//! (`fread`/`fseek`/`ftell`, so the RIFF walk stays C++), and `FSoundInfo` owns and calls an
//! `FSndEnv`. What moved here is the byte layout of the tags, behind `flare_fdt_sound_*` /
//! `flare_fsound_*` on the `FSWFStream` handles ([`crate::fswfstream`]), null-checked and under
//! `catch_unwind`:
//!
//! * `DefineSound` (id, 4+2+1+1 format bits, sample count, the sample block as large data),
//!   `SoundStreamBlock`, `SoundStreamHead`, `SoundStreamHead2`;
//! * `FSndEnv::WriteToSWFStream` (a `DWORD` and two `WORD`s) and the part of
//!   `FSoundInfo::WriteToSWFStream` up to and including `nPoints`; the envelope itself follows,
//!   written by the C++ `FSndEnv`.
//!
//! Quirk kept on purpose: the `FSndEnv` constructor of the original assigns its parameters to
//! themselves (`mark44 = mark44; ...`: the parameters shadow the members), so the members stay
//! uninitialised and what [`flare_fsndenv_write`] writes is whatever the object's memory held.
//! The header keeps that constructor verbatim (a "fixed" constructor would change the bytes of
//! every file that sets an envelope); `swfshapes_ref.cpp` pins it by constructing the object
//! over pre-filled memory.
//!
//! Also kept: `StreamHead` writes the compression (1), rate (0) and size (1) constants of the
//! original, taking only the mix format, sound type and sample count from the object; an
//! `FSoundInfo` with `hasEnvelope` but no envelope object dereferenced NULL in C++ (the header
//! still does).

use crate::fswfstream::Stream;
use std::os::raw::c_void;

const STAG_DEFINE_SOUND: u16 = 14;
const STAG_SOUND_STREAM_HEAD: u16 = 18;
const STAG_SOUND_STREAM_BLOCK: u16 = 19;
const STAG_SOUND_STREAM_HEAD2: u16 = 45;

unsafe fn tag(out: *mut c_void, id: u16, build: impl FnOnce(&mut Stream)) -> i32 {
    if out.is_null() {
        return 1;
    }
    crate::guard(2, || {
        let mut body = Stream::new();
        build(&mut body);
        let n = body.size();
        (*(out as *mut Stream)).append_tag(id, n, Some(&mut body));
        0
    })
}

unsafe fn on(h: *mut c_void, f: impl FnOnce(&mut Stream)) -> i32 {
    if h.is_null() {
        return 1;
    }
    crate::guard(2, || {
        f(&mut *(h as *mut Stream));
        0
    })
}

/// `FDTDefineSound::WriteToSWFStream`.
///
/// `sound` is registered as large data (not copied): it must stay alive until the stream is
/// written, which the owning `FDTDefineSound` guarantees.
///
/// # Safety
/// `out` NULL or a live `FSWFStream` handle; `sound` valid for `sound_size` bytes at write time
/// (NULL only with `sound_size == 0`).
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_fdt_define_sound(
    out: *mut c_void,
    sound_id: u16,
    format: u8,
    rate: u8,
    size: u8,
    sound_type: u8,
    sample_count: u32,
    sound: *const u8,
    sound_size: u32,
) -> i32 {
    if sound.is_null() && sound_size > 0 {
        return 1;
    }
    tag(out, STAG_DEFINE_SOUND, |b| {
        b.write_word(sound_id as u32);
        b.write_bits(format as u32, 4);
        b.write_bits(rate as u32, 2);
        b.write_bits(size as u32, 1);
        b.write_bits(sound_type as u32, 1);
        b.write_dword(sample_count);
        b.write_large_data(sound, sound_size);
    })
}

/// `FDTSoundStreamBlock::WriteToSWFStream`.
///
/// # Safety
/// As [`flare_fdt_define_sound`] for `data`.
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_sound_stream_block(out: *mut c_void, data: *const u8, size: u32) -> i32 {
    if data.is_null() && size > 0 {
        return 1;
    }
    tag(out, STAG_SOUND_STREAM_BLOCK, |b| b.write_large_data(data, size))
}

/// `FDTSoundStreamHead::WriteToSWFStream`.
///
/// # Safety
/// `out` NULL or a live `FSWFStream` handle.
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_sound_stream_head(out: *mut c_void, mix_format: u8, sound_type: u8, count: u16) -> i32 {
    tag(out, STAG_SOUND_STREAM_HEAD, |b| {
        b.write_byte(mix_format as u32);
        b.write_bits(1, 4);
        b.write_bits(0, 2);
        b.write_bits(1, 1);
        b.write_bits(sound_type as u32, 1);
        b.write_word(count as u32);
    })
}

/// `FDTSoundStreamHead2::WriteToSWFStream`.
///
/// # Safety
/// `out` NULL or a live `FSWFStream` handle.
#[no_mangle]
pub unsafe extern "C" fn flare_fdt_sound_stream_head2(
    out: *mut c_void,
    mix_format: u8,
    compression: u8,
    rate: u8,
    size: u8,
    sound_type: u8,
    count: u16,
) -> i32 {
    tag(out, STAG_SOUND_STREAM_HEAD2, |b| {
        b.write_byte(mix_format as u32);
        b.write_bits(compression as u32, 4);
        b.write_bits(rate as u32, 2);
        b.write_bits(size as u32, 1);
        b.write_bits(sound_type as u32, 1);
        b.write_word(count as u32);
    })
}

/// `FSndEnv::WriteToSWFStream`.
///
/// # Safety
/// `out` NULL or a live `FSWFStream` handle.
#[no_mangle]
pub unsafe extern "C" fn flare_fsndenv_write(out: *mut c_void, mark44: u32, level0: u16, level1: u16) -> i32 {
    on(out, |s| {
        s.write_dword(mark44);
        s.write_word(level0 as u32);
        s.write_word(level1 as u32);
    })
}

/// `FSoundInfo::WriteToSWFStream` up to and including `nPoints` (the envelope follows).
///
/// # Safety
/// `out` NULL or a live `FSWFStream` handle.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_fsoundinfo_write(
    out: *mut c_void,
    sync_flags: u8,
    has_envelope: u8,
    has_loops: u8,
    has_out_point: u8,
    has_in_point: u8,
    in_point: u32,
    out_point: u32,
    loop_count: u16,
    n_points: u8,
) -> i32 {
    on(out, |s| {
        s.write_bits(sync_flags as u32, 4);
        s.write_bits(has_envelope as u32, 1);
        s.write_bits(has_loops as u32, 1);
        s.write_bits(has_out_point as u32, 1);
        s.write_bits(has_in_point as u32, 1);
        if has_in_point != 0 {
            s.write_dword(in_point);
        }
        if has_out_point != 0 {
            s.write_dword(out_point);
        }
        if has_loops != 0 {
            s.write_word(loop_count as u32);
        }
        if has_envelope != 0 {
            s.write_byte(n_points as u32);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &mut Stream) -> *mut c_void {
        s as *mut Stream as *mut c_void
    }

    fn bytes(s: &mut Stream) -> Vec<u8> {
        let mut v = vec![0u8; s.full_size() as usize];
        assert!(s.write_to_memory(&mut v));
        v
    }

    fn unhex(s: &str) -> Vec<u8> {
        if s == "-" {
            return vec![];
        }
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
    }

    #[test]
    fn refuses_bad_arguments() {
        unsafe {
            assert_eq!(flare_fdt_define_sound(std::ptr::null_mut(), 1, 1, 0, 0, 0, 0, std::ptr::null(), 0), 1);
            let mut s = Stream::new();
            assert_eq!(flare_fdt_define_sound(p(&mut s), 1, 1, 0, 0, 0, 0, std::ptr::null(), 4), 1);
            assert_eq!(flare_fdt_sound_stream_block(p(&mut s), std::ptr::null(), 1), 1);
            assert_eq!(flare_fsndenv_write(std::ptr::null_mut(), 0, 0, 0), 1);
            assert_eq!(flare_fsoundinfo_write(std::ptr::null_mut(), 0, 0, 0, 0, 0, 0, 0, 0, 0), 1);
        }
    }

    #[test]
    fn define_sound_layout() {
        let pcm = [9u8, 8, 7];
        let mut s = Stream::new();
        unsafe { assert_eq!(flare_fdt_define_sound(p(&mut s), 5, 1, 3, 1, 1, 100, pcm.as_ptr(), 3), 0) };
        // tag 14, length 2 + 1 + 4 + 3; format 0001 rate 11 size 1 type 1 = 0x1f
        assert_eq!(bytes(&mut s), vec![(14u16 << 6 | 10) as u8, (14u16 << 6 >> 8) as u8, 5, 0, 0x1f, 100, 0, 0, 0, 9, 8, 7]);
    }

    #[test]
    fn stream_head_constants() {
        let mut s = Stream::new();
        unsafe { assert_eq!(flare_fdt_sound_stream_head(p(&mut s), 6, 1, 0x0102), 0) };
        // 0x06, then 0001 00 1 1, then count
        assert_eq!(bytes(&mut s), vec![(18u16 << 6 | 4) as u8, (18u16 << 6 >> 8) as u8, 6, 0x13, 2, 1]);
    }

    #[test]
    fn sound_info_fields_follow_flags() {
        let mut s = Stream::new();
        unsafe { assert_eq!(flare_fsoundinfo_write(p(&mut s), 3, 1, 1, 1, 1, 0x01020304, 0x0a0b0c0d, 0x0506, 2), 0) };
        assert_eq!(bytes(&mut s), vec![0x3f, 4, 3, 2, 1, 0x0d, 0x0c, 0x0b, 0x0a, 6, 5, 2]);
        let mut s = Stream::new();
        unsafe { assert_eq!(flare_fsoundinfo_write(p(&mut s), 1, 0, 0, 0, 0, 7, 7, 7, 7), 0) };
        assert_eq!(bytes(&mut s), vec![0x10]);
    }

    // Replays the records recorded from the original C++ (swfshapes_cases.txt).
    #[test]
    fn replay_recorded_cpp_cases() {
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        for line in include_str!("swfshapes_cases.txt").lines() {
            let f: Vec<&str> = line.split('|').collect();
            let a = |i: usize| -> i64 { f[i].parse().unwrap() };
            let mut s = Stream::new();
            match f[0] {
                "snddef" => {
                    // id|format|rate|size|type|count|sound hex|bytes
                    let snd = unhex(f[7]);
                    unsafe {
                        assert_eq!(
                            flare_fdt_define_sound(p(&mut s), a(1) as u16, a(2) as u8, a(3) as u8, a(4) as u8, a(5) as u8, a(6) as u32, snd.as_ptr(), snd.len() as u32),
                            0
                        )
                    };
                    assert_eq!(bytes(&mut s), unhex(f[8]), "{line}");
                }
                "sndblock" => {
                    let d = unhex(f[1]);
                    unsafe { assert_eq!(flare_fdt_sound_stream_block(p(&mut s), d.as_ptr(), d.len() as u32), 0) };
                    assert_eq!(bytes(&mut s), unhex(f[2]), "{line}");
                }
                "sndhead" => {
                    unsafe { assert_eq!(flare_fdt_sound_stream_head(p(&mut s), a(1) as u8, a(2) as u8, a(3) as u16), 0) };
                    assert_eq!(bytes(&mut s), unhex(f[4]), "{line}");
                }
                "sndhead2" => {
                    unsafe {
                        assert_eq!(flare_fdt_sound_stream_head2(p(&mut s), a(1) as u8, a(2) as u8, a(3) as u8, a(4) as u8, a(5) as u8, a(6) as u16), 0)
                    };
                    assert_eq!(bytes(&mut s), unhex(f[7]), "{line}");
                }
                "sndinfo" => {
                    // flags..., then the envelope: its members are whatever memory held (the
                    // constructor never sets them), recorded as the pre-fill
                    let env = unhex(f[10]);
                    let (mark44, l0, l1) = (
                        u32::from_le_bytes([env[0], env[1], env[2], env[3]]),
                        u16::from_le_bytes([env[4], env[5]]),
                        u16::from_le_bytes([env[6], env[7]]),
                    );
                    unsafe {
                        assert_eq!(
                            flare_fsoundinfo_write(p(&mut s), a(1) as u8, a(2) as u8, a(3) as u8, a(4) as u8, a(5) as u8, a(6) as u32, a(7) as u32, a(8) as u16, a(9) as u8),
                            0
                        );
                        if a(2) != 0 {
                            assert_eq!(flare_fsndenv_write(p(&mut s), mark44, l0, l1), 0);
                        }
                    }
                    assert_eq!(bytes(&mut s), unhex(f[11]), "{line}");
                }
                _ => continue,
            }
            *counts.entry(f[0].into()).or_default() += 1;
        }
        for k in ["snddef", "sndblock", "sndhead", "sndhead2", "sndinfo"] {
            assert!(counts.get(k).copied().unwrap_or(0) > 0, "no {k} cases");
        }
    }
}
