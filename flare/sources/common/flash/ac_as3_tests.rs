//! Tests of the native AS3/ABC core. The golden file `as3_cases.json` is
//! recorded by running the ORIGINAL Python implementation (Next2Flash
//! `app/lib/as3_decompiler`) via `tests/native/gen_as3_cases.py`.

use super::ac_as3_abc::*;
use super::ac_as3_api as api;
use super::ac_as3_decompile::*;
use super::ac_as3_patch as patch;
use serde_json::Value;
use std::collections::HashMap;

fn golden() -> Value {
    serde_json::from_str(include_str!("as3_cases.json")).unwrap()
}
fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
}
fn first_diff(a: &str, b: &str) -> String {
    for (i, (x, y)) in a.lines().zip(b.lines()).enumerate() {
        if x != y {
            return format!("line {i}:\n  got:  {x}\n  want: {y}");
        }
    }
    format!("length differs: got {} lines, want {}", a.lines().count(), b.lines().count())
}

#[test]
fn abc_round_trip_and_class_sources_match_python() {
    let g = golden();
    let mut classes = 0;
    for c in g["cases"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let data = unhex(c["abc"].as_str().unwrap());
        let abc = AbcFile::parse(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(abc.serialize() == data, c["roundtrip"].as_bool().unwrap(), "{name} round trip");
        let dec = As3Decompiler::new(&abc);
        let list = dec.list_classes();
        assert_eq!(list.len(), c["list"].as_array().unwrap().len(), "{name} class count");
        for (l, w) in list.iter().zip(c["list"].as_array().unwrap()) {
            assert_eq!(l.full_name, w["full_name"].as_str().unwrap(), "{name}");
            assert_eq!(l.super_name, w["super"].as_str().unwrap(), "{name}");
            assert_eq!(l.is_interface, w["is_interface"].as_bool().unwrap(), "{name}");
            assert_eq!(l.trait_count as u64, w["trait_count"].as_u64().unwrap(), "{name}");
        }
        for w in c["classes"].as_array().unwrap() {
            let idx = w["index"].as_u64().unwrap() as usize;
            let got = dec.decompile_class(idx);
            match w.get("source") {
                Some(src) => {
                    let got = got.unwrap_or_else(|e| panic!("{name} class {idx}: {e}"));
                    assert!(got == src.as_str().unwrap(), "{name} class {idx}: {}", first_diff(&got, src.as_str().unwrap()));
                    classes += 1;
                }
                None => assert!(got.is_err(), "{name} class {idx} should fail"),
            }
        }
        for w in c["scripts"].as_array().unwrap() {
            let idx = w["index"].as_u64().unwrap() as usize;
            if let Some(src) = w.get("source") {
                let got = dec.decompile_script(idx).unwrap();
                assert!(got == src.as_str().unwrap(), "{name} script {idx}: {}", first_diff(&got, src.as_str().unwrap()));
            }
        }
        for sl in c["scripts_list"].as_array().unwrap() {
            let i = sl["index"].as_u64().unwrap() as usize;
            assert_eq!(dec.list_scripts()[i].class_count as u64, sl["class_count"].as_u64().unwrap());
        }
    }
    assert!(classes > 30, "only {classes} classes compared");
}

#[test]
fn method_decompile_and_disassembly_match_python() {
    let g = golden();
    let mut n = 0;
    for c in g["cases"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let abc = AbcFile::parse(&unhex(c["abc"].as_str().unwrap())).unwrap();
        let dec = As3Decompiler::new(&abc);
        for (mi, m) in c["methods"].as_object().unwrap() {
            let mi: u32 = mi.parse().unwrap();
            let ctx = m["ctx"].as_array().unwrap();
            let got = dec.decompile_method(mi, ctx[0].as_i64().unwrap(), ctx[1].as_bool().unwrap(), ctx[2].as_str().unwrap());
            let want = m["src"].as_str().unwrap();
            assert!(got == want, "{name} method {mi}: {}", first_diff(&got, want));
            let dis = disassemble(&abc, &abc.method_bodies[&mi].code);
            assert!(dis == m["dis"].as_str().unwrap(), "{name} method {mi} disasm: {}", first_diff(&dis, m["dis"].as_str().unwrap()));
            n += 1;
        }
    }
    assert!(n > 100, "only {n} methods compared");
}

#[test]
fn transplant_matches_python() {
    let g = golden();
    for t in g["transplants"].as_array().unwrap() {
        let name = t["name"].as_str().unwrap();
        let changed: Option<Vec<String>> = t["changed"].as_array().map(|a| a.iter().map(|x| x.as_str().unwrap().to_string()).collect());
        let edits: Option<Vec<(String, String)>> =
            t["edits"].as_array().map(|a| a.iter().map(|p| (p[0].as_str().unwrap().to_string(), p[1].as_str().unwrap().to_string())).collect());
        let r = patch::transplant_class(
            &unhex(t["orig"].as_str().unwrap()),
            &unhex(t["compiled"].as_str().unwrap()),
            t["class"].as_str().unwrap(),
            changed.as_deref(),
            edits.as_deref(),
        );
        if t.get("error").is_some() {
            assert!(r.is_err(), "{name}");
        } else {
            let (got, _log) = r.unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(hex(&got), t["result"].as_str().unwrap(), "{name}");
        }
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[test]
fn string_patch_matches_python() {
    let g = golden();
    for t in g["swfpatch"].as_array().unwrap() {
        let map: HashMap<String, String> = t["mapping"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect();
        let (out, rep, blocks) = patch::patch_strings(&unhex(t["swf"].as_str().unwrap()), &map).unwrap();
        assert_eq!(rep as u64, t["replaced"].as_u64().unwrap());
        assert_eq!(blocks as u64, t["blocks"].as_u64().unwrap());
        let s = patch::read_swf_full(&out).unwrap();
        let want: Vec<(u64, String)> = t["tags"].as_array().unwrap().iter().map(|p| (p[0].as_u64().unwrap(), p[1].as_str().unwrap().to_string())).collect();
        let got: Vec<(u64, String)> = s.tags.iter().map(|(c, b)| (*c as u64, hex(b))).collect();
        assert_eq!(got, want);
    }
}

#[test]
fn difflib_port_matches_python() {
    let g = golden();
    for d in g["diffs"].as_array().unwrap() {
        let a: Vec<String> = d["a"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
        let b: Vec<String> = d["b"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
        let got: Vec<(String, u64, u64, u64, u64)> = patch::diff_opcodes(&a, &b).into_iter().map(|(t, i1, i2, j1, j2)| (t.to_string(), i1 as u64, i2 as u64, j1 as u64, j2 as u64)).collect();
        let want: Vec<(String, u64, u64, u64, u64)> = d["ops"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| (o[0].as_str().unwrap().to_string(), o[1].as_u64().unwrap(), o[2].as_u64().unwrap(), o[3].as_u64().unwrap(), o[4].as_u64().unwrap()))
            .collect();
        assert_eq!(got, want, "{a:?} vs {b:?}");
    }
}

#[test]
fn float_formatting_matches_python() {
    let g = golden();
    for f in g["floats"].as_array().unwrap() {
        let v = f64::from_bits(f["bits"].as_u64().unwrap());
        assert_eq!(py_float_repr(v), f["repr"].as_str().unwrap());
        assert_eq!(fmt_g(v, 15), f["g15"].as_str().unwrap());
    }
}

#[test]
fn source_helpers_match_python() {
    let g = golden();
    for t in g["texts"].as_array().unwrap() {
        let (o, e, cn) = (t["orig"].as_str().unwrap(), t["edited"].as_str().unwrap(), t["class"].as_str().unwrap());
        let want_m: Vec<(String, String)> = t["methods"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect();
        let mut got_m = patch::extract_method_texts(o, cn);
        got_m.sort();
        let mut want_m = want_m;
        want_m.sort();
        assert_eq!(got_m, want_m);
        let want_c: Vec<String> = t["changed"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
        assert_eq!(patch::detect_changed_methods(o, e, cn), want_c);
        let want_e: Vec<(String, String)> = t["string_edits"].as_array().unwrap().iter().map(|p| (p[0].as_str().unwrap().to_string(), p[1].as_str().unwrap().to_string())).collect();
        assert_eq!(patch::extract_string_edits_from_source(o, e), want_e);
    }
}

// ---- Rust-only behaviour (no Python oracle) ------------------------------

fn sample_swf() -> (Vec<u8>, Value) {
    let g = golden();
    let t = &g["swfpatch"][0];
    (unhex(t["swf"].as_str().unwrap()), t.clone())
}

#[test]
fn malformed_input_is_an_error_not_a_panic() {
    assert!(AbcFile::parse(&[]).is_err());
    assert!(AbcFile::parse(&[0x10, 0, 0x2e, 0, 5, 1]).is_err());
    assert!(api::list(&[1, 2, 3])["blocks"][0].get("error").is_some());
    let (swf, _) = sample_swf();
    for cut in [10usize, 20, 40, swf.len() / 2, swf.len() - 3] {
        let _ = api::list(&swf[..cut]);
        let _ = api::decompile_all(&swf[..cut]);
    }
}

#[test]
fn assembler_matches_instruction_decoder() {
    let mut a = Assembler::new();
    a.emit("getlocal_0", &[]).unwrap();
    a.label("top").unwrap();
    a.emit("pushbyte", &[Arg::Val(3)]).unwrap();
    a.emit("iftrue", &[Arg::from("top")]).unwrap();
    a.emit_switch("top", &["top", "top"]).unwrap();
    a.emit("returnvoid", &[]).unwrap();
    let code = a.assemble().unwrap();
    let ins = parse_instructions(&code);
    assert_eq!(ins.len(), 5);
    assert_eq!(ins[2].operands[0], (D::S24, 1 - ins[2].end as i64));
    assert!(a.emit("nosuchop", &[]).is_err());
    assert!(disassemble(&AbcFile::parse(&unhex(golden()["cases"][0]["abc"].as_str().unwrap())).unwrap(), &code).contains("lookupswitch"));
}

#[test]
fn edited_string_literal_is_written_back_without_a_compiler() {
    let g = golden();
    let b = g["cases"].as_array().unwrap().iter().find(|c| c["name"] == "fuzz3").unwrap();
    let abc_bytes = unhex(b["abc"].as_str().unwrap());
    let src = b["classes"][0]["source"].as_str().unwrap();
    // find a string literal inside a method body
    let lit = patch::string_literals(src).into_iter().find(|s| !s.is_empty() && !s.contains('\\')).expect("a literal");
    let swf = g["swfpatch"][0]["swf"].as_str().unwrap();
    let swf = unhex(swf);
    let edited = src.replacen(&format!("\"{lit}\""), "\"EDITED-LITERAL\"", 1);
    let p = patch::patch_class_source(&swf, 0, 0, &edited).unwrap();
    let blocks = api::blocks(&p.swf).unwrap();
    let abc = AbcFile::parse(&blocks[0].1).unwrap();
    let new_src = As3Decompiler::new(&abc).decompile_class(0).unwrap();
    assert!(new_src.contains("EDITED-LITERAL"));
    assert!(new_src.matches("EDITED-LITERAL").count() >= 1);
    // every other class byte is untouched: only the ABC tag differs
    let (a, b2) = (patch::read_swf_full(&swf).unwrap(), patch::read_swf_full(&p.swf).unwrap());
    for (x, y) in a.tags.iter().zip(&b2.tags) {
        if x.0 != 82 {
            assert_eq!(x, y);
        }
    }
    let _ = abc_bytes;
    // a structural edit needs a compiler
    let e2 = src.replacen("return;", "return; return;", 1);
    assert!(patch::patch_class_source(&swf, 0, 0, &e2).unwrap_err().contains("needs compiler") || e2 == src);
}

#[test]
fn json_api_lists_and_decompiles() {
    let (swf, _) = sample_swf();
    let l = api::list(&swf);
    assert_eq!(l["ok"], true);
    assert!(l["blocks"][0]["classes"][0]["members"].as_array().unwrap().len() >= 3);
    let d = api::decompile_all(&swf);
    assert!(d["blocks"][0]["classes"][0]["source"].as_str().unwrap().starts_with("package"));
    assert!(api::disassemble_method(&swf, 0, 1).is_ok());
    assert!(api::decompile_class(&swf, 5, 0).is_err());
}
