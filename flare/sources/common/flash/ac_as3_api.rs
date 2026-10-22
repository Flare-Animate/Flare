//! JSON-level AS3 API shared by the FFI surface (`flare_as3_*` in
//! `flare_formats.rs`) and the tests: list, decompile, disassemble, method
//! inventory, patch. Input is a SWF image or a bare ABC block.
//!
//! The decompile / patch logic is derived from Next2Flash (MIT, Copyright (c)
//! 2026 masterwebx; see `flare/sources/THIRD-PARTY-NOTICES.md`).

use super::ac_as3_abc::*;
use super::ac_as3_decompile::As3Decompiler;
use super::ac_as3_patch as patch;
use serde_json::{json, Value};
use std::collections::HashMap;

pub const VERSION: &str = "flare-as3-native/1";

/// `(name, abc)` blocks of a SWF, or the single bare ABC block.
pub fn blocks(data: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    if data.is_empty() {
        return Err("empty input".into());
    }
    if looks_like_swf(data) {
        Ok(read_abc_blocks(data)?.1)
    } else {
        Ok(vec![("DoABC".into(), data.to_vec())])
    }
}

fn block(data: &[u8], idx: usize) -> Result<AbcFile, String> {
    let b = blocks(data)?;
    let (_, abc) = b.get(idx).ok_or_else(|| format!("ABC block {idx} not found (input has {} blocks)", b.len()))?;
    AbcFile::parse(abc)
}

fn err(e: impl Into<String>) -> Value {
    json!({"ok": false, "error": e.into()})
}

/// Class + member inventory of every block.
pub fn list(data: &[u8]) -> Value {
    let bl = match blocks(data) {
        Ok(b) => b,
        Err(e) => return err(e),
    };
    let mut out = Vec::new();
    for (i, (name, raw)) in bl.iter().enumerate() {
        let abc = match AbcFile::parse(raw) {
            Ok(a) => a,
            Err(e) => {
                out.push(json!({"index": i, "name": name, "error": e}));
                continue;
            }
        };
        let dec = As3Decompiler::new(&abc);
        let classes: Vec<Value> = dec
            .list_classes()
            .iter()
            .map(|c| {
                let inst = &abc.instances[c.index];
                let cls = &abc.classes[c.index];
                let mut members = Vec::new();
                for (is_static, traits) in [(false, &inst.traits), (true, &cls.traits)] {
                    for t in traits {
                        let kind = match t.kind {
                            TRAIT_SLOT => "var",
                            TRAIT_CONST => "const",
                            TRAIT_METHOD => "method",
                            TRAIT_GETTER => "getter",
                            TRAIT_SETTER => "setter",
                            TRAIT_FUNCTION => "function",
                            _ => "other",
                        };
                        let mut m = json!({"kind": kind, "name": abc.mn_name(t.name_idx), "static": is_static});
                        if matches!(t.kind, TRAIT_SLOT | TRAIT_CONST) {
                            m["type"] = json!(abc.type_name(t.type_name));
                        } else if let Some(mi) = abc.methods.get(t.method_idx as usize) {
                            m["method"] = json!(t.method_idx);
                            m["params"] = json!(dec.format_params(mi));
                            m["returns"] = json!(abc.type_name(mi.return_type));
                            m["code_size"] = json!(abc.method_bodies.get(&t.method_idx).map_or(0, |b| b.code.len()));
                        }
                        members.push(m);
                    }
                }
                json!({
                    "index": c.index, "name": c.name, "package": c.package, "full_name": c.full_name,
                    "super": c.super_name, "is_interface": c.is_interface, "trait_count": c.trait_count,
                    "interfaces": inst.interfaces.iter().map(|&x| abc.mn_full(x)).collect::<Vec<_>>(),
                    "constructor": inst.iinit, "cinit": cls.cinit, "members": members,
                })
            })
            .collect();
        let scripts: Vec<Value> = dec
            .list_scripts()
            .iter()
            .map(|s| json!({"index": s.index, "sinit": s.sinit, "functions": s.functions, "variables": s.variables, "class_count": s.class_count}))
            .collect();
        out.push(json!({"index": i, "name": name, "classes": classes, "scripts": scripts}));
    }
    json!({"ok": true, "version": VERSION, "blocks": out})
}

/// Decompile every class of every block (per-class errors are reported, not fatal).
pub fn decompile_all(data: &[u8]) -> Value {
    let bl = match blocks(data) {
        Ok(b) => b,
        Err(e) => return err(e),
    };
    let mut out = Vec::new();
    for (i, (name, raw)) in bl.iter().enumerate() {
        let abc = match AbcFile::parse(raw) {
            Ok(a) => a,
            Err(e) => return err(format!("block {i}: {e}")),
        };
        let dec = As3Decompiler::new(&abc);
        let mut classes = Vec::new();
        for c in dec.list_classes() {
            match dec.decompile_class(c.index) {
                Ok(src) => classes.push(json!({"index": c.index, "package": c.package, "name": c.name, "full_name": c.full_name, "source": src})),
                Err(e) => classes.push(json!({"index": c.index, "package": c.package, "name": c.name, "full_name": c.full_name, "error": e})),
            }
        }
        out.push(json!({"index": i, "name": name, "classes": classes}));
    }
    json!({"ok": true, "error": null, "blocks": out})
}

pub fn decompile_class(data: &[u8], block_idx: usize, class_idx: usize) -> Result<String, String> {
    let abc = block(data, block_idx)?;
    As3Decompiler::new(&abc).decompile_class(class_idx)
}

pub fn decompile_script(data: &[u8], block_idx: usize, script_idx: usize) -> Result<String, String> {
    let abc = block(data, block_idx)?;
    As3Decompiler::new(&abc).decompile_script(script_idx)
}

/// P-code listing of one method body.
pub fn disassemble_method(data: &[u8], block_idx: usize, method_idx: u32) -> Result<String, String> {
    let abc = block(data, block_idx)?;
    let b = abc.method_bodies.get(&method_idx).ok_or_else(|| format!("method {method_idx} has no body"))?;
    catch(|| disassemble(&abc, &b.code))
}

pub fn patch_strings(data: &[u8], patch_json: &str) -> Result<(Vec<u8>, Value), String> {
    let v: Value = serde_json::from_str(patch_json).map_err(|e| format!("could not read patch: {e}"))?;
    let obj = v.get("strings").and_then(|s| s.as_object()).ok_or("patch \"strings\" must be an object of {old: new}")?;
    let map: HashMap<String, String> = obj.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect();
    let (swf, replaced, blocks) = patch::patch_strings(data, &map)?;
    Ok((swf, json!({"ok": true, "replaced": replaced, "blocks": blocks, "error": null})))
}

pub fn patch_class(data: &[u8], block_idx: usize, class_idx: usize, edited: &str) -> Result<(Vec<u8>, Value), String> {
    let p = patch::patch_class_source(data, block_idx, class_idx, edited)?;
    Ok((p.swf, json!({"ok": true, "changed_methods": p.changed_methods, "replaced": p.replaced_strings, "error": null})))
}

/// Transplant a compiled class into `data`'s block (SWF in, SWF out).
pub fn transplant(data: &[u8], block_idx: usize, compiled_abc: &[u8], class_full_name: &str) -> Result<(Vec<u8>, Value), String> {
    let mut s = patch::read_swf_full(data)?;
    let idx: Vec<usize> = s.tags.iter().enumerate().filter(|(_, t)| t.0 == TAG_DOABC || t.0 == TAG_DOABC2).map(|(i, _)| i).collect();
    let ti = *idx.get(block_idx).ok_or("ABC block not found")?;
    let (code, body) = s.tags[ti].clone();
    let (prefix, abc) = if code == TAG_DOABC2 {
        let nul = body[4..].iter().position(|&b| b == 0).ok_or("bad DoABC2")? + 4;
        (body[..nul + 1].to_vec(), body[nul + 1..].to_vec())
    } else {
        (vec![], body)
    };
    let (new_abc, log) = patch::transplant_class(&abc, compiled_abc, class_full_name, None, None)?;
    let mut nb = prefix;
    nb.extend_from_slice(&new_abc);
    s.tags[ti].1 = nb;
    Ok((patch::write_swf_full(&s), json!({"ok": true, "log": log, "error": null})))
}
