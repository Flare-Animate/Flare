// ============================================================================
// AI-GENERATED DRAFT -- cpp2rust, commit 337827dde8a229193acd95e7c1a55a3b650051ce. DO NOT MERGE AS-IS.
//
// Source:   flarelib/sandor_fxs/SError.cpp   (the original .cpp is untouched and still in the tree)
//
// cpp2rust output is a starting point, not a port. Before this becomes the
// port, someone has to, against the C++ (see docs/PORTING.md):
//   1. read the two side by side and fix every mistranslation;
//   2. check bit-exactness -- see the differential-harness pattern in
//      tests/native/ (a golden captured from the original C++ is the only
//      real proof; docs/CPP2RUST_TRIAL.md records why that matters);
//   3. give it a C ABI (extern "C" + the guard, never panic across FFI);
//   4. rewrite flarelib/sandor_fxs/SError.cpp's header as a thin forwarder to that ABI;
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
#[derive(Clone, Record, ByteRepr, VaArg, FnPtrArg, Default)]
#[byte_size(4)]
pub struct _UC_PIXEL {
    #[offset(0)]
    pub b: u8,
    #[offset(1)]
    pub g: u8,
    #[offset(2)]
    pub r: u8,
    #[offset(3)]
    pub m: u8,
}
#[derive(Clone, Record, ByteRepr, VaArg, FnPtrArg, Default)]
#[byte_size(8)]
pub struct _US_PIXEL {
    #[offset(0)]
    pub b: u16,
    #[offset(2)]
    pub g: u16,
    #[offset(4)]
    pub r: u16,
    #[offset(6)]
    pub m: u16,
}
#[derive(Clone, Record, ByteRepr, VaArg, FnPtrArg, Default)]
#[byte_size(16)]
pub struct _I_PIXEL {
    #[offset(0)]
    pub b: i32,
    #[offset(4)]
    pub g: i32,
    #[offset(8)]
    pub r: i32,
    #[offset(12)]
    pub m: i32,
}
#[derive(Clone, Record, ByteRepr, VaArg, FnPtrArg, Default)]
#[byte_size(16)]
pub struct SRECT {
    #[offset(0)]
    pub x0: i32,
    #[offset(4)]
    pub y0: i32,
    #[offset(8)]
    pub x1: i32,
    #[offset(12)]
    pub y1: i32,
}
#[derive(Clone, Record, ByteRepr, VaArg, FnPtrArg, Default)]
#[byte_size(32)]
pub struct SDRECT {
    #[offset(0)]
    pub x0: f64,
    #[offset(8)]
    pub y0: f64,
    #[offset(16)]
    pub x1: f64,
    #[offset(24)]
    pub y1: f64,
}
#[derive(Clone, Record, ByteRepr, VaArg, FnPtrArg, Default)]
#[byte_size(16)]
pub struct SLINE {
    #[offset(0)]
    pub x0: i32,
    #[offset(4)]
    pub y0: i32,
    #[offset(8)]
    pub x1: i32,
    #[offset(12)]
    pub y1: i32,
}
#[derive(Clone, Record, ByteRepr, VaArg, FnPtrArg, Default)]
#[byte_size(32)]
pub struct SDLINE {
    #[offset(0)]
    pub x0: f64,
    #[offset(8)]
    pub y0: f64,
    #[offset(16)]
    pub x1: f64,
    #[offset(24)]
    pub y1: f64,
}
#[derive(Clone, Record, ByteRepr, VaArg, FnPtrArg, Default)]
#[byte_size(8)]
pub struct SPOINT {
    #[offset(0)]
    pub x: i32,
    #[offset(4)]
    pub y: i32,
}
#[derive(Clone, Record, ByteRepr, VaArg, FnPtrArg, Default)]
#[byte_size(16)]
pub struct SDPOINT {
    #[offset(0)]
    pub x: f64,
    #[offset(8)]
    pub y: f64,
}
#[derive(Clone, Record, ByteRepr, VaArg, FnPtrArg, Default)]
#[byte_size(12)]
pub struct SXYW {
    #[offset(0)]
    pub x: i32,
    #[offset(4)]
    pub y: i32,
    #[offset(8)]
    pub w: i32,
}
#[derive(Clone, Record, ByteRepr, VaArg, FnPtrArg, Default)]
#[byte_size(16)]
pub struct SXYDW {
    #[offset(0)]
    pub x: i32,
    #[offset(4)]
    pub y: i32,
    #[offset(8)]
    pub w: f64,
}
#[derive(Clone, Record, ByteRepr, VaArg, FnPtrArg, Default)]
#[byte_size(16)]
pub struct SXYD {
    #[offset(0)]
    pub x: i32,
    #[offset(4)]
    pub y: i32,
    #[offset(8)]
    pub d: f64,
}
#[derive(Clone, Record, ByteRepr, VaArg, FnPtrArg, CppFrom, Destructor)]
#[cpp_from(Ptr< i8  >  , new_1, value)]
#[byte_size(40)]
pub struct SError {
    #[offset(8)]
    #[byte_size(32)]
    m_msg: Vec<i8>,
}
impl SError {
    pub fn new_2() -> Self {
        Self {
            m_msg: {
                let mut __bytes = Ptr::<i8>::from_string_literal(b"").to_c_bytes();
                __bytes.push(0);
                __bytes
            },
        }
    }
    pub fn new_1(mut s: Ptr<i8>) -> Self {
        Self {
            m_msg: {
                let mut __bytes = s.to_c_bytes();
                __bytes.push(0);
                __bytes
            },
        }
    }
}
impl Default for SError {
    fn default() -> Self {
        { SError::new_2() }
    }
}
#[derive(Clone, Record, ByteRepr, VaArg, FnPtrArg, CppFrom, Destructor)]
#[cpp_from(Ptr< i8  >  , new_1, value)]
#[byte_size(40)]
pub struct SMemAllocError {}
impl SMemAllocError {
    pub fn new_2() -> Self {
        Self {}
    }
    pub fn new_1(mut s: Ptr<i8>) -> Self {
        Self {}
    }
}
impl Default for SMemAllocError {
    fn default() -> Self {
        { SMemAllocError::new_2() }
    }
}
#[derive(Clone, Record, ByteRepr, VaArg, FnPtrArg, CppFrom, Destructor)]
#[cpp_from(Ptr< i8  >  , new_1, value)]
#[byte_size(40)]
pub struct SWriteRasterError {}
impl SWriteRasterError {
    pub fn new_2() -> Self {
        Self {}
    }
    pub fn new_1(mut s: Ptr<i8>) -> Self {
        Self {}
    }
}
impl Default for SWriteRasterError {
    fn default() -> Self {
        { SWriteRasterError::new_2() }
    }
}
#[derive(Clone, Record, ByteRepr, VaArg, FnPtrArg, CppFrom, Destructor)]
#[cpp_from(Ptr< i8  >  , new_1, value)]
#[byte_size(40)]
pub struct SBlurMatrixError {}
impl SBlurMatrixError {
    pub fn new_2() -> Self {
        Self {}
    }
    pub fn new_1(mut s: Ptr<i8>) -> Self {
        Self {}
    }
}
impl Default for SBlurMatrixError {
    fn default() -> Self {
        { SBlurMatrixError::new_2() }
    }
}
#[derive(Clone, Record, ByteRepr, VaArg, FnPtrArg, CppFrom, Destructor)]
#[cpp_from(Ptr< i8  >  , new_1, value)]
#[byte_size(40)]
pub struct SFileReadError {}
impl SFileReadError {
    pub fn new_2() -> Self {
        Self {}
    }
    pub fn new_1(mut s: Ptr<i8>) -> Self {
        Self {}
    }
}
impl Default for SFileReadError {
    fn default() -> Self {
        { SFileReadError::new_2() }
    }
}
impl SError for SBlurMatrixError {
    fn debug_print(&self) {}
}
impl SError for SFileReadError {
    fn debug_print(&self) {}
}
impl SError for SMemAllocError {
    fn debug_print(&self) {}
}
impl SError for SWriteRasterError {
    fn debug_print(&self) {}
}
pub fn __cpp2rust_init_globals() {}
