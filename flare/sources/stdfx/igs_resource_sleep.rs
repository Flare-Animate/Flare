// ============================================================================
// AI-GENERATED DRAFT -- cpp2rust, commit 05b75bf7e8722485e715e88bde0863c0ebb52420. DO NOT MERGE AS-IS.
//
// Source:   stdfx/igs_resource_sleep.cpp   (the original .cpp is untouched and still in the tree)
//
// cpp2rust output is a starting point, not a port. Before this becomes the
// port, someone has to, against the C++ (see docs/PORTING.md):
//   1. read the two side by side and fix every mistranslation;
//   2. check bit-exactness -- see the differential-harness pattern in
//      tests/native/ (a golden captured from the original C++ is the only
//      real proof; docs/CPP2RUST_TRIAL.md records why that matters);
//   3. give it a C ABI (extern "C" + the guard, never panic across FFI);
//   4. rewrite stdfx/igs_resource_sleep.cpp's header as a thin forwarder to that ABI;
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
pub fn sleep_sn_0(mut seconds: i64, mut nano_seconds: i64) {
    let req: Value<libcc2rs::Timespec> = Rc::new(RefCell::new(Default::default()));
    (*req.borrow_mut()).tv_sec = seconds;
    (*req.borrow_mut()).tv_nsec = nano_seconds;
    let rem: Value<libcc2rs::Timespec> = Rc::new(RefCell::new(Default::default()));
    (*rem.borrow_mut()).tv_sec = 0_i64;
    (*rem.borrow_mut()).tv_nsec = 0_i64;
    if (({ nanosleep_1((req.as_pointer()), (rem.as_pointer())) }) < 0) {
        {
            let __tmp_0: Value<Vec<i8>> = Rc::new(RefCell::new(
                ({
                    let _tit: Value<Vec<i8>> = Rc::new(RefCell::new({
                        let mut __bytes =
                            Ptr::<i8>::from_string_literal(b"nanosleep(-)").to_c_bytes();
                        __bytes.push(0);
                        __bytes
                    }));
                    let _file: Value<Vec<i8>> = Rc::new(RefCell::new({
                        let mut __bytes = Ptr::<i8 >::from_string_literal(b"/home/runner/work/Flare/Flare/flare/sources/stdfx/igs_resource_sleep_unix.cpp" ) .to_c_bytes();
                        __bytes.push(0);
                        __bytes
                    }));
                    let _line: Value<Vec<i8>> = Rc::new(RefCell::new({
                        let mut __bytes = Ptr::<i8>::from_string_literal(b"18").to_c_bytes();
                        __bytes.push(0);
                        __bytes
                    }));
                    let _pretty_function: Value<Vec<i8>> = Rc::new(RefCell::new({
                        let mut __bytes = Ptr::<i8>::from_string_literal(
                            b"void igs::resource::sleep_sn(const time_t, const long)",
                        )
                        .to_c_bytes();
                        __bytes.push(0);
                        __bytes
                    }));
                    let _comp_type: Value<Vec<i8>> = Rc::new(RefCell::new({
                        let mut __bytes = Ptr::<i8>::from_string_literal(b"__GNUC__").to_c_bytes();
                        __bytes.push(0);
                        __bytes
                    }));
                    let _gnuc: Value<Vec<i8>> = Rc::new(RefCell::new({
                        let mut __bytes = Ptr::<i8>::from_string_literal(b"4").to_c_bytes();
                        __bytes.push(0);
                        __bytes
                    }));
                    let _gnuc_minor: Value<Vec<i8>> = Rc::new(RefCell::new({
                        let mut __bytes = Ptr::<i8>::from_string_literal(b"2").to_c_bytes();
                        __bytes.push(0);
                        __bytes
                    }));
                    let _gnuc_patchlevel: Value<Vec<i8>> = Rc::new(RefCell::new({
                        let mut __bytes = Ptr::<i8>::from_string_literal(b"1").to_c_bytes();
                        __bytes.push(0);
                        __bytes
                    }));
                    let _gnuc_rh_release: Value<Vec<i8>> = Rc::new(RefCell::new({
                        let mut __bytes =
                            Ptr::<i8>::from_string_literal(b"__GNUC_RH_RELEASE__").to_c_bytes();
                        __bytes.push(0);
                        __bytes
                    }));
                    let _date: Value<Vec<i8>> = Rc::new(RefCell::new({
                        let mut __bytes =
                            Ptr::<i8>::from_string_literal(b"Oct 19 2026").to_c_bytes();
                        __bytes.push(0);
                        __bytes
                    }));
                    let _time: Value<Vec<i8>> = Rc::new(RefCell::new({
                        let mut __bytes = Ptr::<i8>::from_string_literal(b"12:10:00").to_c_bytes();
                        __bytes.push(0);
                        __bytes
                    }));
                    msg_from_err__2(
                        _tit.as_pointer(),
                        (libcc2rs::cpp2rust_errno().read()),
                        _file.as_pointer(),
                        _line.as_pointer(),
                        _pretty_function.as_pointer(),
                        _comp_type.as_pointer(),
                        _gnuc.as_pointer(),
                        _gnuc_minor.as_pointer(),
                        _gnuc_patchlevel.as_pointer(),
                        _gnuc_rh_release.as_pointer(),
                        _date.as_pointer(),
                        _time.as_pointer(),
                    )
                }),
            ));
            std_domain_error::new_1({ __tmp_0.as_pointer() })
        };
    }
}
pub fn sleep_m_3(mut milli_seconds: u32) {
    let mut seconds: i64 = (((milli_seconds).wrapping_div(1000_u32)) as i64);
    let mut nano_seconds: i64 =
        ((((milli_seconds).wrapping_rem(1000_u32)).wrapping_mul(1000000_u32)) as i64);
    ({ sleep_sn_0(seconds, nano_seconds) });
}
pub fn __cpp2rust_init_globals() {}
