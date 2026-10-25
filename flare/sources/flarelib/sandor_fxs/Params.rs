// ============================================================================
// AI-GENERATED DRAFT -- cpp2rust, commit 1d1fb4c699e738772ff85d35fd4b1e01fddeb4ab. DO NOT MERGE AS-IS.
//
// Source:   flarelib/sandor_fxs/Params.cpp   (the original .cpp is untouched and still in the tree)
//
// cpp2rust output is a starting point, not a port. Before this becomes the
// port, someone has to, against the C++ (see docs/PORTING.md):
//   1. read the two side by side and fix every mistranslation;
//   2. check bit-exactness -- see the differential-harness pattern in
//      tests/native/ (a golden captured from the original C++ is the only
//      real proof; docs/CPP2RUST_TRIAL.md records why that matters);
//   3. give it a C ABI (extern "C" + the guard, never panic across FFI);
//   4. rewrite flarelib/sandor_fxs/Params.cpp's header as a thin forwarder to that ABI;
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
#[derive(Record, ByteRepr, VaArg, FnPtrArg, CopyAssign, Destructor)]
#[byte_size(56)]
pub struct CInputParam {
    #[offset(8)]
    pub m_scale: f64,
    #[offset(16)]
    pub m_isEconf: bool,
    #[offset(24)]
    #[byte_size(32)]
    pub m_econfFN: Vec<i8>,
}
impl CInputParam {
    pub fn new() -> Self {
        Self {
            m_scale: 0_f64,
            m_isEconf: false,
            m_econfFN: {
                let mut __bytes = Ptr::<i8>::from_string_literal(b"").to_c_bytes();
                __bytes.push(0);
                __bytes
            },
        }
    }
    pub fn copy_from(p: Ptr<CInputParam>) -> Self {
        Self {
            m_scale: p.with(|__s| __s.m_scale),
            m_isEconf: p.with(|__s| __s.m_isEconf),
            m_econfFN: ((*p.upgrade().deref()).m_econfFN).clone(),
        }
    }
}
impl Clone for CInputParam {
    fn clone(&self) -> Self {
        let __src: Value<CInputParam> = Rc::new(RefCell::new(CInputParam {
            m_scale: self.m_scale.clone(),
            m_isEconf: self.m_isEconf.clone(),
            m_econfFN: self.m_econfFN.clone(),
        }));
        CInputParam::copy_from(__src.as_pointer())
    }
}
impl Default for CInputParam {
    fn default() -> Self {
        { CInputParam::new() }
    }
}
pub trait CInputParamImpl {
    fn copy_assign(&self, _a0: Ptr<CInputParam>) -> Ptr<CInputParam>;
}
impl CInputParamImpl for Ptr<CInputParam> {
    fn copy_assign(&self, _a0: Ptr<CInputParam>) -> Ptr<CInputParam> {
        field!((*self), m_scale).write({ _a0.with(|__s| __s.m_scale) });
        field!((*self), m_isEconf).write({ _a0.with(|__s| __s.m_isEconf) });
        field!((*self), m_econfFN).write({ _a0.with(|__s| __s.m_econfFN.clone()) });
        return (*self).clone();
    }
}
pub fn __cpp2rust_init_globals() {}
