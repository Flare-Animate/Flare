//! Robustness tests for issues #110/#117: truncated and corrupted .fla / .xfl /
//! .swf inputs, built in-test from the handmade fixture, must produce `Err`
//! (or a best-effort document), never a panic.

use super::*;
use std::panic::{catch_unwind, AssertUnwindSafe};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn fixture() -> Package {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/handmade");
    read_dir(&dir).unwrap()
}

fn exercise_fla(b: &[u8]) {
    if let Ok(pkg) = unzip(b) {
        let _ = read_package(&pkg);
    }
    let _ = repair_end_record(b);
}

fn exercise_swf(b: &[u8]) {
    let _ = swf_read::import(b);
    let _ = swf::parse(b);
}

fn hammer(label: &str, good: &[u8], f: fn(&[u8]), rounds: usize) {
    let mut bad: Vec<String> = Vec::new();
    let mut check = |what: String, v: &[u8]| {
        if catch_unwind(AssertUnwindSafe(|| f(v))).is_err() {
            bad.push(what);
        }
    };
    check("empty".into(), &[]);
    let step = (good.len() / 400).max(1);
    for n in (0..good.len()).step_by(step) {
        check(format!("truncated to {n}"), &good[..n]);
    }
    let mut rng = Rng(0xC0FF_EE12_3456_789B);
    for r in 0..rounds {
        let mut v = good.to_vec();
        for _ in 0..1 + r % 5 {
            let i = (rng.next() as usize) % v.len();
            v[i] = rng.next() as u8;
        }
        check(format!("mutation {r}"), &v);
    }
    assert!(bad.is_empty(), "{label}: panics on {:?}", &bad[..bad.len().min(5)]);
}

#[test]
fn corrupt_fla_zip_never_panics() {
    let fla = zip(&fixture()).unwrap();
    hammer("fla", &fla, exercise_fla, 400);
}

#[test]
fn corrupt_xfl_xml_never_panics() {
    let pkg = fixture();
    let dom = pkg.get("DOMDocument.xml").expect("fixture has DOMDocument.xml").clone();
    for round in 0..200u64 {
        let mut p = pkg.clone();
        let mut d = dom.clone();
        let mut rng = Rng(round + 1);
        if round % 2 == 0 {
            d.truncate((rng.next() as usize) % d.len().max(1));
        } else {
            let i = (rng.next() as usize) % d.len();
            d[i] = rng.next() as u8;
        }
        p.insert("DOMDocument.xml".into(), d);
        assert!(catch_unwind(AssertUnwindSafe(|| {
            let _ = read_package(&p);
        }))
        .is_ok(), "round {round}");
    }
    // missing document entirely
    let mut p = fixture();
    p.remove("DOMDocument.xml");
    assert!(read_package(&p).is_err());
}

#[test]
fn corrupt_swf_never_panics() {
    let im = read_package(&fixture()).unwrap();
    let e = swf_write::export(&im.doc).unwrap();
    hammer("swf", &e.bytes, exercise_swf, 400);
}

#[test]
fn garbage_inputs_are_errors_not_panics() {
    for junk in [&b""[..], b"PK", b"PK\x05\x06", b"FWS", b"CWS\x0a\xff\xff\xff\xff", b"\0\0\0\0\0\0\0\0\0"] {
        exercise_fla(junk);
        exercise_swf(junk);
    }
}
