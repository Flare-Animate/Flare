// ============================================================================
// AI-GENERATED DRAFT -- cpp2rust, commit 5152092808c6a745a8a2a44d2b87d6ddc550e046. DO NOT MERGE AS-IS.
//
// Source:   common/psdlib/psdutils.cpp   (the original .cpp is untouched and still in the tree)
//
// cpp2rust output is a starting point, not a port. Before this becomes the
// port, someone has to, against the C++ (see docs/PORTING.md):
//   1. read the two side by side and fix every mistranslation;
//   2. check bit-exactness -- see the differential-harness pattern in
//      tests/native/ (a golden captured from the original C++ is the only
//      real proof; docs/CPP2RUST_TRIAL.md records why that matters);
//   3. give it a C ABI (extern "C" + the guard, never panic across FFI);
//   4. rewrite common/psdlib/psdutils.cpp's header as a thin forwarder to that ABI;
//   5. delete the .cpp and register the module in flare_formats.rs;
//   6. delete this banner.
//
// Not registered yet on purpose: an unregistered .rs compiles nowhere, so this
// draft cannot break the build until it has been reviewed.
// ============================================================================
extern crate libcc2rs;
use libcc2rs::*;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::prelude::*;
use std::io::{Read, Seek, Write};
use std::os::fd::AsFd;
use std::rc::{Rc, Weak};
pub type anon_0 = u32;
pub const anon_0_RAWDATA: anon_0 = 0;
pub const anon_0_RLECOMP: anon_0 = 1;
pub const anon_0_ZIPWITHOUTPREDICTION: anon_0 = 2;
pub const anon_0_ZIPWITHPREDICTION: anon_0 = 3;
#[derive(Clone, Record, ByteRepr, VaArg, FnPtrArg, Default)]
#[byte_size(64)]
pub struct TPSDChannelInfo {
    #[offset(0)]
    pub id: i32,
    #[offset(4)]
    pub comptype: i32,
    #[offset(8)]
    pub rows: i64,
    #[offset(16)]
    pub cols: i64,
    #[offset(24)]
    pub rowbytes: i64,
    #[offset(32)]
    pub length: i64,
    #[offset(40)]
    pub filepos: i64,
    #[offset(48)]
    #[byte_size(8)]
    pub rowpos: Ptr<i64>,
    #[offset(56)]
    #[byte_size(8)]
    pub unzipdata: Ptr<u8>,
}
pub fn readrow_1(
    mut psd: Ptr<CFile>,
    mut chan: Ptr<TPSDChannelInfo>,
    mut row: i64,
    mut inbuffer: Ptr<u8>,
    mut tmpbuffer: Ptr<u8>,
) {
    let mut n: i64 = 0_i64;
    let mut rlebytes: i64 = 0_i64;
    let mut pos: i64 = 0_i64;
    let mut seekres: i32 = 0;
    'switch: {
        match { chan.with(|__s| __s.comptype) } {
            __v if __v == (anon_0_RAWDATA as i32) => {
                pos = ({ chan.with(|__s| __s.filepos) } + {
                    ({ chan.with(|__s| __s.rowbytes) } * { row })
                });
                seekres = match psd.with_mut(|__v: &mut CFile| __v.seek(pos, ::libc::SEEK_SET)) {
                    -1 => -1,
                    _ => 0,
                };
                if (seekres != -1_i32) {
                    n = ({
                        let __a0 = (inbuffer).to_any();
                        let __a1 = 1_usize;
                        let __a2 = (chan.with(|__s| __s.rowbytes) as usize);
                        let __a3 = (psd).clone();
                        libcc2rs::fread_refcount(__a0, __a1, __a2, __a3)
                    } as i64);
                }
                break 'switch;
            }
            __v if __v == (anon_0_RLECOMP as i32) => {
                pos = { (elem!(chan.with(|__s| __s.rowpos.clone()), row).read()) };
                seekres = match psd.with_mut(|__v: &mut CFile| __v.seek(pos, ::libc::SEEK_SET)) {
                    -1 => -1,
                    _ => 0,
                };
                if (seekres != -1_i32) {
                    let __rhs = ({
                        let __a0 = (tmpbuffer).to_any();
                        let __a1 = 1_usize;
                        let __a2 = (({
                            (elem!(chan.with(|__s| __s.rowpos.clone()), (row + 1_i64)).read())
                        } - { pos }) as usize);
                        let __a3 = (psd).clone();
                        libcc2rs::fread_refcount(__a0, __a1, __a2, __a3)
                    } as i64);
                    rlebytes = __rhs;
                    n = (({
                        unpackrow_2(
                            (inbuffer).clone(),
                            (tmpbuffer).clone(),
                            chan.with(|__s| __s.rowbytes),
                            rlebytes,
                        )
                    }) as i64);
                }
                break 'switch;
            }
            __v if __v == (anon_0_ZIPWITHPREDICTION as i32)
                || __v == (anon_0_ZIPWITHOUTPREDICTION as i32) =>
            {
                {
                    (inbuffer).to_any().memcpy(
                        &(chan
                            .with(|__s| __s.unzipdata.clone())
                            .offset(({ chan.with(|__s| __s.rowbytes) } * { row }) as isize)
                            as Ptr<u8>)
                            .to_any(),
                        (chan.with(|__s| __s.rowbytes) as usize) as usize,
                    );
                    (inbuffer).to_any()
                };
                return;
            }
            _ => {}
        }
    };
    if ({ n } < { chan.with(|__s| __s.rowbytes) }) {
        {
            (inbuffer.offset((n) as isize) as Ptr<u8>).to_any().memset(
                (0) as u8,
                (({ chan.with(|__s| __s.rowbytes) } - { n }) as usize) as usize,
            );
            (inbuffer.offset((n) as isize) as Ptr<u8>).to_any()
        };
    }
}
pub fn unpackrow_2(mut out: Ptr<u8>, mut in_: Ptr<u8>, mut outlen: i64, mut inlen: i64) -> i32 {
    let mut i: i64 = 0_i64;
    let mut len: i64 = 0_i64;
    let mut val: i32 = 0_i32;
    i = 0_i64;
    'loop_: while (inlen > 1_i64) && (i < outlen) {
        len = ((in_.postfix_inc().read()) as i64);
        inlen.prefix_dec();
        if (len == 128_i64) {
        } else {
            if (len > 128_i64) {
                len = { (((1 + 256) as i64) - len) };
                val = ((in_.postfix_inc().read()) as i32);
                inlen.prefix_dec();
                if ((i + len) <= outlen) {
                    {
                        (out).to_any().memset((val) as u8, (len as usize) as usize);
                        (out).to_any()
                    };
                } else {
                    {
                        (out)
                            .to_any()
                            .memset((val) as u8, ((outlen - i) as usize) as usize);
                        (out).to_any()
                    };
                    len = 0_i64;
                }
            } else {
                len.prefix_inc();
                if ((i + len) <= outlen) {
                    if (len > inlen) {
                        break;
                    }
                    {
                        (out)
                            .to_any()
                            .memcpy(&(in_).to_any(), (len as usize) as usize);
                        (out).to_any()
                    };
                    in_ += len;
                    inlen -= len;
                } else {
                    {
                        (out)
                            .to_any()
                            .memcpy(&(in_).to_any(), ((outlen - i) as usize) as usize);
                        (out).to_any()
                    };
                    len = 0_i64;
                }
            }
            out += len;
            i += len;
        };
    }
    if (i < outlen) {}
    return (i as i32);
}
pub fn skipBlock_3(mut f: Ptr<CFile>) {
    let mut n: i64 = ({ read4Bytes_4((f).clone()) });
    if (n != 0) {
        match f.with_mut(|__v: &mut CFile| __v.seek(n, ::libc::SEEK_CUR)) {
            -1 => -1,
            _ => 0,
        };
    } else {
    }
}
pub fn read2UBytes_5(mut f: Ptr<CFile>) -> u32 {
    let mut n: u32 = ((({ fgetc_6((f).clone()) }) << 8) as u32);
    return {
        n |= (({ fgetc_6(f) }) as u32);
        n
    };
}
pub fn read2Bytes_7(mut f: Ptr<CFile>) -> i32 {
    let mut n: u32 = ((({ fgetc_6((f).clone()) }) << 8) as u32);
    n |= (({ fgetc_6((f).clone()) }) as u32);
    return (if (n < 32768_u32) {
        n
    } else {
        (n).wrapping_sub(65536_u32)
    } as i32);
}
pub fn read4Bytes_4(mut f: Ptr<CFile>) -> i64 {
    let mut n: i64 = ((({ fgetc_6((f).clone()) }) << 24) as i64);
    n |= ((({ fgetc_6((f).clone()) }) << 16) as i64);
    n |= ((({ fgetc_6((f).clone()) }) << 8) as i64);
    return ({ n } | { (({ fgetc_6(f) }) as i64) });
}
pub fn mymalloc_8(mut n: i64) -> AnyPtr {
    let mut p: AnyPtr = libcc2rs::malloc_refcount((n as usize));
    if !(p).is_null() {
        return p;
    } else {
    }
    return AnyPtr::default();
}
pub fn mycalloc_9(mut n: i64, mut size: i32) -> AnyPtr {
    let mut p: AnyPtr = libcc2rs::calloc_refcount((n as usize), (size as usize));
    if !(p).is_null() {
        return p;
    } else {
    }
    return AnyPtr::default();
}
pub fn psdUnzipWithoutPrediction_10(
    mut src_buf: Ptr<u8>,
    mut src_len: i32,
    mut dst_buf: Ptr<u8>,
    mut dst_len: i32,
) -> i32 {
    let stream: Value<z_stream_s> = Rc::new(RefCell::new(<z_stream_s>::default()));
    let mut state: i32 = 0_i32;
    {
        ((stream.as_pointer()) as Ptr<z_stream_s>)
            .to_any()
            .memset((0) as u8, 112usize as usize);
        ((stream.as_pointer()) as Ptr<z_stream_s>).to_any()
    };
    (*stream.borrow_mut()).data_type = 0;
    (*stream.borrow_mut()).next_in = src_buf.reinterpret_cast::<u8>();
    (*stream.borrow_mut()).avail_in = (src_len as u32);
    (*stream.borrow_mut()).next_out = dst_buf.reinterpret_cast::<u8>();
    (*stream.borrow_mut()).avail_out = (dst_len as u32);
    if (({
        inflateInit__11(
            (stream.as_pointer()),
            Ptr::<i8>::from_string_literal(b"1.3"),
            (112usize as i32),
        )
    }) != 0)
    {
        return 0;
    }
    let mut __do_while = true;
    'loop_: while __do_while || ({ (*stream.borrow()).avail_out } > 0_u32) {
        __do_while = false;
        state = ({ inflate_12((stream.as_pointer()), 1) });
        if (state == 1) {
            break;
        }
        if (state == (-3_i32)) || (state != 0) {
            break;
        }
    }
    if (state != 1) && (state != 0) {
        return 0;
    }
    return 1;
}
pub fn psdUnzipWithPrediction_13(
    mut src_buf: Ptr<u8>,
    mut src_len: i32,
    mut dst_buf: Ptr<u8>,
    mut dst_len: i32,
    mut row_size: i32,
    mut color_depth: i32,
) -> i32 {
    let mut status: i32 = 0_i32;
    let mut len: i32 = 0_i32;
    let mut buf: Ptr<u8> = Ptr::<u8>::null();
    status =
        ({ psdUnzipWithoutPrediction_10((src_buf).clone(), src_len, (dst_buf).clone(), dst_len) });
    if !(status != 0) {
        return status;
    }
    buf = (dst_buf).clone();
    let mut __do_while = true;
    'loop_: while __do_while || (dst_len > 0) {
        __do_while = false;
        len = row_size;
        if (color_depth == 16) {
            'loop_: while (len.prefix_dec() != 0) {
                elem!(buf, 2).write({
                    (((elem!(buf, 2).read()) as i32)
                        + (((elem!(buf, 0).read()) as i32)
                            + ((((elem!(buf, 1).read()) as i32)
                                + ((elem!(buf, 3).read()) as i32))
                                >> 8))) as u8
                });
                elem!(buf, 3).write({
                    (((elem!(buf, 3).read()) as i32) + ((elem!(buf, 1).read()) as i32)) as u8
                });
                buf += 2;
            }
            buf += 2;
            dst_len -= (row_size * 2);
        } else {
            'loop_: while (len.prefix_dec() != 0) {
                (buf.offset((1) as isize)).write({
                    ((((buf.offset((1) as isize)).read()) as i32) + ((buf.read()) as i32)) as u8
                });
                buf.postfix_inc();
            }
            buf.postfix_inc();
            dst_len -= row_size;
        }
    }
    return 1;
}
pub fn __cpp2rust_init_globals() {}
