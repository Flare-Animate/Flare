//! ABC / SWF patching: constant-pool merge, bytecode translation, stack/scope
//! analysis, class transplant (ID-preserving: every other class, tag and
//! constant keeps its bytes and index), inline constant patching, string
//! retexting, and a native "edit a string literal in a decompiled class and
//! write it back" path that needs no Flex SDK.
//!
//! Derived from Next2Flash's `app/lib/as3_decompiler/{abc_patcher,
//! swf_patcher}.py` (https://github.com/SSF2-Mods-Official/Next2Flash, MIT,
//! Copyright (c) 2026 masterwebx; see `flare/sources/THIRD-PARTY-NOTICES.md`).
//! The Python original is the oracle for `ac_as3_tests.rs`.
//!
//! Deviations: diagnostics that Python prints to stderr are returned as a log;
//! the Flex `mxmlc` JVM-shell manager and the decompile-all-then-recompile
//! workflow are not ported (only a one-shot `compile_sources`); the SWF writer
//! keeps the original RECT bytes like the Python one but compresses with
//! flate2 (byte stream differs from zlib level 9, content is identical).

use super::ac_as3_abc::*;
use super::ac_as3_decompile::{escape_str, As3Decompiler};
use super::ac_swf;
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------- pool merge

#[derive(Default, Debug)]
pub struct PoolMapping {
    pub strings: HashMap<u32, u32>,
    pub integers: HashMap<u32, u32>,
    pub uintegers: HashMap<u32, u32>,
    pub doubles: HashMap<u32, u32>,
    pub namespaces: HashMap<u32, u32>,
    pub ns_sets: HashMap<u32, u32>,
    pub multinames: HashMap<u32, u32>,
    pub methods: HashMap<u32, u32>,
    pub classes: HashMap<u32, u32>,
}

impl PoolMapping {
    fn new() -> Self {
        let mut p = PoolMapping::default();
        for m in [&mut p.strings, &mut p.integers, &mut p.uintegers, &mut p.doubles, &mut p.namespaces, &mut p.ns_sets, &mut p.multinames] {
            m.insert(0, 0);
        }
        p
    }
}

fn g(m: &HashMap<u32, u32>, k: u32) -> u32 {
    *m.get(&k).unwrap_or(&k)
}

fn collect_private_ns(abc: &AbcFile, ci: usize) -> Vec<(u32, HashSet<String>)> {
    let inst = &abc.instances[ci];
    let cls = &abc.classes[ci];
    let mut out: Vec<(u32, HashSet<String>)> = Vec::new();
    for traits in [&inst.traits, &cls.traits] {
        for t in traits {
            if t.name_idx == 0 || t.name_idx as usize >= abc.multinames.len() {
                continue;
            }
            let m = &abc.multinames[t.name_idx as usize];
            if matches!(m.kind, MN_QNAME | MN_QNAME_A) && m.data.len() >= 2 {
                let (ns_idx, name_idx) = (m.data[0], m.data[1]);
                if ns_idx > 0 && (ns_idx as usize) < abc.namespaces.len() && abc.namespaces[ns_idx as usize].0 == NS_PRIVATE {
                    let name = if name_idx > 0 && (name_idx as usize) < abc.strings.len() { abc.strings[name_idx as usize].clone() } else { String::new() };
                    if let Some(e) = out.iter_mut().find(|e| e.0 == ns_idx) {
                        e.1.insert(name);
                    } else {
                        out.push((ns_idx, HashSet::from([name])));
                    }
                }
            }
        }
    }
    out
}

fn build_private_ns_overrides(orig: &AbcFile, compiled: &AbcFile) -> Vec<(u32, u32)> {
    let mut overrides: Vec<(u32, u32)> = Vec::new();
    let mut used: HashSet<u32> = HashSet::new();
    let mut by_name: HashMap<String, usize> = HashMap::new();
    for ci in 0..orig.instances.len() {
        by_name.insert(orig.mn_full(orig.instances[ci].name_idx), ci);
    }
    for cci in 0..compiled.instances.len() {
        let name = compiled.mn_full(compiled.instances[cci].name_idx);
        let Some(&oci) = by_name.get(&name) else { continue };
        let mut comp_ns = collect_private_ns(compiled, cci);
        let orig_ns = collect_private_ns(orig, oci);
        // sorted(key=-len): stable
        comp_ns.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
        for (cidx, cnames) in comp_ns {
            if overrides.iter().any(|o| o.0 == cidx) {
                continue;
            }
            let mut best: Option<u32> = None;
            let mut best_overlap = 0;
            for (oidx, onames) in &orig_ns {
                if used.contains(oidx) {
                    continue;
                }
                let ov = cnames.intersection(onames).count();
                if ov > best_overlap {
                    best_overlap = ov;
                    best = Some(*oidx);
                }
            }
            if let Some(b) = best {
                overrides.push((cidx, b));
                used.insert(b);
            }
        }
    }
    overrides
}

fn merge_pools(dst: &mut AbcFile, src: &AbcFile, ns_overrides: &[(u32, u32)]) -> PoolMapping {
    let mut pm = PoolMapping::new();
    let mut lk: HashMap<String, u32> = dst.strings.iter().enumerate().map(|(i, s)| (s.clone(), i as u32)).collect();
    for si in 1..src.strings.len() {
        let s = &src.strings[si];
        if let Some(&i) = lk.get(s) {
            pm.strings.insert(si as u32, i);
        } else {
            let n = dst.strings.len() as u32;
            dst.strings.push(s.clone());
            lk.insert(s.clone(), n);
            pm.strings.insert(si as u32, n);
        }
    }
    let mut lk: HashMap<i32, u32> = dst.integers.iter().enumerate().map(|(i, v)| (*v, i as u32)).collect();
    for si in 1..src.integers.len() {
        let v = src.integers[si];
        let n = *lk.entry(v).or_insert_with(|| {
            dst.integers.push(v);
            (dst.integers.len() - 1) as u32
        });
        pm.integers.insert(si as u32, n);
    }
    let mut lk: HashMap<u32, u32> = dst.uintegers.iter().enumerate().map(|(i, v)| (*v, i as u32)).collect();
    for si in 1..src.uintegers.len() {
        let v = src.uintegers[si];
        let n = *lk.entry(v).or_insert_with(|| {
            dst.uintegers.push(v);
            (dst.uintegers.len() - 1) as u32
        });
        pm.uintegers.insert(si as u32, n);
    }
    let mut lk: HashMap<u64, u32> = dst.doubles.iter().enumerate().map(|(i, v)| (v.to_bits(), i as u32)).collect();
    for si in 1..src.doubles.len() {
        let v = src.doubles[si];
        let n = *lk.entry(v.to_bits()).or_insert_with(|| {
            dst.doubles.push(v);
            (dst.doubles.len() - 1) as u32
        });
        pm.doubles.insert(si as u32, n);
    }
    let mut lk: HashMap<(u8, u32), u32> = dst.namespaces.iter().enumerate().map(|(i, v)| (*v, i as u32)).collect();
    for si in 1..src.namespaces.len() {
        if let Some(o) = ns_overrides.iter().find(|o| o.0 == si as u32) {
            pm.namespaces.insert(si as u32, o.1);
            continue;
        }
        let (kind, name_idx) = src.namespaces[si];
        let key = (kind, g(&pm.strings, name_idx));
        let n = *lk.entry(key).or_insert_with(|| {
            dst.namespaces.push(key);
            (dst.namespaces.len() - 1) as u32
        });
        pm.namespaces.insert(si as u32, n);
    }
    let mut lk: HashMap<Vec<u32>, u32> = dst.ns_sets.iter().enumerate().map(|(i, v)| (v.clone(), i as u32)).collect();
    for si in 1..src.ns_sets.len() {
        let mapped: Vec<u32> = src.ns_sets[si].iter().map(|&n| g(&pm.namespaces, n)).collect();
        let n = *lk.entry(mapped.clone()).or_insert_with(|| {
            dst.ns_sets.push(mapped.clone());
            (dst.ns_sets.len() - 1) as u32
        });
        pm.ns_sets.insert(si as u32, n);
    }
    let mut lk: HashMap<Multiname, u32> = dst.multinames.iter().enumerate().map(|(i, v)| (v.clone(), i as u32)).collect();
    for si in 1..src.multinames.len() {
        let m = &src.multinames[si];
        let d = &m.data;
        let data: Vec<u32> = match m.kind {
            MN_QNAME | MN_QNAME_A => vec![g(&pm.namespaces, d[0]), g(&pm.strings, d[1])],
            MN_RTQNAME | MN_RTQNAME_A => vec![g(&pm.strings, d[0])],
            MN_RTQNAME_L | MN_RTQNAME_LA => vec![],
            MN_MULTINAME | MN_MULTINAME_A => vec![g(&pm.strings, d[0]), g(&pm.ns_sets, d[1])],
            MN_MULTINAME_L | MN_MULTINAME_LA => vec![g(&pm.ns_sets, d[0])],
            MN_TYPENAME => {
                // Python maps through pm.multinames, which only has entries for
                // already-merged (lower-index) multinames; later ones fall back to the raw index.
                let mut v = vec![g(&pm.multinames, d[0])];
                v.extend(d[1..].iter().map(|&p| g(&pm.multinames, p)));
                v
            }
            _ => d.clone(),
        };
        let key = Multiname { kind: m.kind, data };
        let n = *lk.entry(key.clone()).or_insert_with(|| {
            dst.multinames.push(key.clone());
            (dst.multinames.len() - 1) as u32
        });
        pm.multinames.insert(si as u32, n);
    }
    pm
}

// ---------------------------------------------------------------- bytecode

fn remap_operands(ops: &[(D, i64)], pm: &PoolMapping) -> Vec<(D, i64)> {
    ops.iter()
        .map(|&(d, v)| {
            let m = |h: &HashMap<u32, u32>| h.get(&(v as u32)).map_or(v, |&x| x as i64);
            let nv = match d {
                D::Multiname => m(&pm.multinames),
                D::Str | D::DebugString => m(&pm.strings),
                D::Int => m(&pm.integers),
                D::UInt => m(&pm.uintegers),
                D::Double => m(&pm.doubles),
                D::Namespace => m(&pm.namespaces),
                D::Method => m(&pm.methods),
                D::Class => m(&pm.classes),
                _ => v,
            };
            (d, nv)
        })
        .collect()
}

/// Re-encode `instrs` with (possibly resized) operands, fixing branch and
/// switch offsets. Returns `(code, old_pos -> new_pos)`.
fn relayout(code_len: usize, instrs: &[Instr], new_ops: &[Vec<(D, i64)>]) -> (Vec<u8>, HashMap<usize, usize>) {
    let mut old_to_new: HashMap<usize, usize> = HashMap::new();
    let mut new_pos = 0;
    let mut sizes = Vec::new();
    for (i, ins) in instrs.iter().enumerate() {
        old_to_new.insert(ins.start, new_pos);
        let sz = 1 + new_ops[i].iter().map(|&(d, v)| operand_byte_size(d, v)).sum::<usize>();
        sizes.push(sz);
        new_pos += sz;
    }
    if let Some(l) = instrs.last() {
        old_to_new.insert(l.end, new_pos);
    }
    old_to_new.insert(code_len, new_pos);
    let mut buf = Vec::new();
    let mut cur = 0;
    for (i, ins) in instrs.iter().enumerate() {
        let end = cur + sizes[i];
        buf.push(ins.op);
        for &(d, v) in &new_ops[i] {
            let nv = match d {
                D::S24 => old_to_new.get(&((ins.end as i64 + v) as usize)).map_or(v, |&t| t as i64 - end as i64),
                D::SwitchDefault | D::SwitchCase => old_to_new.get(&((ins.start as i64 + v) as usize)).map_or(v, |&t| t as i64 - cur as i64),
                _ => v,
            };
            write_operand(&mut buf, d, nv);
        }
        cur = end;
    }
    (buf, old_to_new)
}

pub fn translate_bytecode(code: &[u8], pm: &PoolMapping) -> (Vec<u8>, HashMap<usize, usize>) {
    if code.is_empty() {
        return (vec![], HashMap::from([(0, 0)]));
    }
    let instrs = parse_instructions(code);
    let ops: Vec<_> = instrs.iter().map(|i| remap_operands(&i.operands, pm)).collect();
    relayout(code.len(), &instrs, &ops)
}

fn stack_fixed(op: u8) -> Option<(i32, i32)> {
    Some(match op {
        0x01 | 0x02 | 0x06 | 0x08 | 0x09 | 0x10 | 0x1D | 0x2B | 0x47 | 0x92 | 0x94 | 0xC2 | 0xC3 | 0xEF | 0xF0 | 0xF1 => (0, 0),
        0x20 | 0x21 | 0x24 | 0x25 | 0x26 | 0x27 | 0x28 | 0x2A | 0x2C | 0x2D | 0x2E | 0x2F | 0x31 | 0x32 | 0x40 | 0x57 | 0x5A | 0x60
        | 0x62 | 0x64 | 0x65 | 0x6E | 0xD0..=0xD3 => (0, 1),
        0x03 | 0x07 | 0x11 | 0x12 | 0x29 | 0x30 | 0x1C | 0x48 | 0x63 | 0x6F | 0xD4..=0xD7 | 0x1B => (1, 0),
        0x35..=0x39 | 0x50..=0x52 | 0x58 | 0x6C | 0x70..=0x78 | 0x80..=0x86 | 0x88 | 0x89 | 0x90 | 0x91 | 0x93 | 0x95 | 0x96 | 0x97
        | 0xB2 | 0xC0 | 0xC1 | 0xC4 => (1, 1),
        0x0C..=0x0F | 0x13..=0x1A | 0x3A..=0x3E | 0x6D => (2, 0),
        0x1E | 0x1F | 0x23 | 0x87 | 0xA0..=0xB1 | 0xB3 | 0xB4 | 0xC5..=0xC7 => (2, 1),
        _ => return None,
    })
}

fn is_cond_branch(op: u8) -> bool {
    matches!(op, 0x0C..=0x0F | 0x11..=0x1A)
}

fn mn_stack_extras(mn: i64, abc: Option<&AbcFile>) -> i32 {
    let Some(abc) = abc else { return 0 };
    if mn <= 0 || mn as usize >= abc.multinames.len() {
        return 0;
    }
    match abc.multinames[mn as usize].kind {
        0x0F | 0x10 => 1,
        0x11 | 0x12 => 2,
        0x1B | 0x1C => 1,
        _ => 0,
    }
}

fn variable_stack_delta(op: u8, operands: &[(D, i64)], abc: Option<&AbcFile>) -> i32 {
    let mut mn_ext = 0;
    let mut argc = 0;
    for &(d, v) in operands {
        if d == D::Multiname {
            mn_ext = mn_stack_extras(v, abc);
        } else if d == D::U30 {
            argc = v as i32;
        }
    }
    match op {
        0x04 => -mn_ext,
        0x05 => -(2 + mn_ext),
        0x59 => -mn_ext,
        0x5D | 0x5E | 0x5F => 1 - mn_ext,
        0x61 => -(2 + mn_ext),
        0x66 => -mn_ext,
        0x68 => -(2 + mn_ext),
        0x6A => -mn_ext,
        0x41 => -(1 + argc),
        0x42 | 0x43 | 0x44 => -argc,
        0x45 | 0x46 => -(mn_ext + argc),
        0x49 => -(1 + argc),
        0x4A | 0x4C => -(mn_ext + argc),
        0x4E | 0x4F => -(1 + mn_ext + argc),
        0x53 => -argc,
        0x55 => 1 - 2 * argc,
        0x56 => 1 - argc,
        _ => 0,
    }
}

struct Walker<'a> {
    instrs: &'a [Instr],
    off_to_idx: HashMap<usize, usize>,
    seen: Vec<bool>,
    scopepos_after: Vec<i32>,
    max_stack: i32,
    max_scope: i32,
    max_local: i64,
    has_activation: bool,
    abc: Option<&'a AbcFile>,
}

impl<'a> Walker<'a> {
    fn walk(&mut self, start_pos: usize, start_stack: i32, start_scope: i32) {
        let n = self.instrs.len();
        let mut worklist = vec![(start_pos, start_stack, start_scope)];
        while let Some((mut pos, mut stack, mut scope)) = worklist.pop() {
            while pos < n {
                if self.seen[pos] {
                    break;
                }
                self.seen[pos] = true;
                let ins = &self.instrs[pos];
                let op = ins.op;
                self.max_stack = self.max_stack.max(stack);
                self.max_scope = self.max_scope.max(scope);
                match op {
                    0xD0..=0xD3 => self.max_local = self.max_local.max((op - 0xD0) as i64),
                    0xD4..=0xD7 => self.max_local = self.max_local.max((op - 0xD4) as i64),
                    0x62 | 0x63 | 0x92 | 0x94 | 0xC2 | 0xC3 | 0x32 => {
                        for &(d, v) in &ins.operands {
                            if d == D::U30 {
                                self.max_local = self.max_local.max(v);
                            }
                        }
                    }
                    _ => {}
                }
                if op == 0x57 {
                    self.has_activation = true;
                }
                match stack_fixed(op) {
                    Some((pop, push)) => stack += push - pop,
                    None => stack += variable_stack_delta(op, &ins.operands, self.abc),
                }
                if op == 0x30 || op == 0x1C {
                    scope += 1;
                } else if op == 0x1D {
                    scope -= 1;
                }
                self.max_stack = self.max_stack.max(stack);
                self.max_scope = self.max_scope.max(scope);
                self.scopepos_after[pos] = scope;
                if op == 0x10 {
                    let mut moved = false;
                    for &(d, v) in &ins.operands {
                        if d == D::S24 {
                            if let Some(&t) = self.off_to_idx.get(&((ins.end as i64 + v) as usize)) {
                                pos = t;
                                moved = true;
                                break;
                            }
                        }
                    }
                    if !moved {
                        break;
                    }
                    continue;
                }
                if is_cond_branch(op) {
                    for &(d, v) in &ins.operands {
                        if d == D::S24 {
                            if let Some(&t) = self.off_to_idx.get(&((ins.end as i64 + v) as usize)) {
                                worklist.push((t, stack, scope));
                            }
                        }
                    }
                    pos += 1;
                    continue;
                }
                if op == 0x1B {
                    for &(d, v) in &ins.operands {
                        if matches!(d, D::SwitchDefault | D::SwitchCase) {
                            if let Some(&t) = self.off_to_idx.get(&((ins.start as i64 + v) as usize)) {
                                worklist.push((t, stack, scope));
                            }
                        }
                    }
                    break;
                }
                if matches!(op, 0x47 | 0x48 | 0x03) {
                    break;
                }
                pos += 1;
            }
        }
    }
}

/// `(max_stack, local_count, max_scope_depth, has_activation)` by a control-flow walk
/// that follows branches and exception handler entries (JPEXS `walkCode`).
pub fn compute_body_metadata(
    code: &[u8],
    exceptions: &[ExceptionInfo],
    init_scope_depth: u32,
    param_count: u32,
    abc: Option<&AbcFile>,
) -> (u32, u32, u32, bool) {
    if code.is_empty() {
        return (0, param_count + 1, init_scope_depth, false);
    }
    let instrs = parse_instructions(code);
    let n = instrs.len();
    let mut off_to_idx = HashMap::new();
    for (i, ins) in instrs.iter().enumerate() {
        off_to_idx.insert(ins.start, i);
    }
    off_to_idx.insert(code.len(), n);
    let mut w = Walker {
        instrs: &instrs,
        off_to_idx,
        seen: vec![false; n],
        scopepos_after: vec![0; n],
        max_stack: 0,
        max_scope: init_scope_depth as i32,
        max_local: param_count as i64,
        has_activation: false,
        abc,
    };
    w.walk(0, 0, init_scope_depth as i32);
    for exc in exceptions {
        let Some(&target_idx) = w.off_to_idx.get(&(exc.target as usize)) else { continue };
        let end_idx = w.off_to_idx.get(&(exc.to_pos as usize)).copied();
        let exc_scope = match end_idx {
            Some(e) if e > 0 => w.scopepos_after[e - 1],
            _ => init_scope_depth as i32,
        };
        w.walk(target_idx, 1, exc_scope);
    }
    (w.max_stack.max(0) as u32, (w.max_local + 1) as u32, w.max_scope.max(0) as u32, w.has_activation)
}

// ---------------------------------------------------------------- translate helpers

pub fn translate_exceptions(exc: &[ExceptionInfo], pm: &PoolMapping, pos_map: Option<&HashMap<usize, usize>>) -> Vec<ExceptionInfo> {
    exc.iter()
        .map(|e| {
            let mp = |v: u32| pos_map.map_or(v, |m| m.get(&(v as usize)).map_or(v, |&x| x as u32));
            ExceptionInfo {
                from_pos: mp(e.from_pos),
                to_pos: mp(e.to_pos),
                target: mp(e.target),
                exc_type: g(&pm.multinames, e.exc_type),
                var_name: g(&pm.multinames, e.var_name),
            }
        })
        .collect()
}

fn remap_vindex(vkind: u8, val: u32, pm: &PoolMapping) -> u32 {
    match vkind {
        0x01 => g(&pm.strings, val),
        0x03 => g(&pm.integers, val),
        0x04 => g(&pm.uintegers, val),
        0x06 => g(&pm.doubles, val),
        k if is_namespace_vkind(k) => g(&pm.namespaces, val),
        _ => val,
    }
}

pub fn translate_trait(t: &TraitInfo, pm: &PoolMapping) -> TraitInfo {
    let mut n = TraitInfo { name_idx: g(&pm.multinames, t.name_idx), kind: t.kind, attr: t.attr, slot_id: t.slot_id, disp_id: t.disp_id, ..Default::default() };
    match t.kind {
        TRAIT_SLOT | TRAIT_CONST => {
            n.type_name = g(&pm.multinames, t.type_name);
            n.vindex = t.vindex;
            n.vkind = t.vkind;
            if t.vindex != 0 {
                n.vindex = remap_vindex(t.vkind, t.vindex, pm);
            }
        }
        TRAIT_METHOD | TRAIT_GETTER | TRAIT_SETTER | TRAIT_FUNCTION => n.method_idx = g(&pm.methods, t.method_idx),
        TRAIT_CLASS => n.class_idx = g(&pm.classes, t.class_idx),
        _ => {}
    }
    n.metadata = t.metadata.clone();
    n
}

fn find_newfunction_refs(code: &[u8]) -> Vec<u32> {
    parse_instructions(code)
        .iter()
        .filter(|i| i.op == 0x40)
        .flat_map(|i| i.operands.iter().filter(|o| o.0 == D::Method).map(|o| o.1 as u32))
        .collect()
}

fn add_compiled_method_info(orig: &mut AbcFile, compiled: &AbcFile, comp_midx: u32, pm: &mut PoolMapping) -> u32 {
    let c = &compiled.methods[comp_midx as usize];
    let mut m = MethodInfo {
        param_count: c.param_count,
        return_type: g(&pm.multinames, c.return_type),
        param_types: c.param_types.iter().map(|&p| g(&pm.multinames, p)).collect(),
        name_idx: g(&pm.strings, c.name_idx),
        flags: c.flags,
        ..Default::default()
    };
    if c.flags & METHOD_HAS_OPTIONAL != 0 {
        m.optional_values = c.optional_values.iter().map(|&(vk, v)| (vk, remap_vindex(vk, v, pm))).collect();
    }
    if c.flags & METHOD_HAS_PARAM_NAMES != 0 {
        m.param_names = c.param_names.iter().map(|&p| g(&pm.strings, p)).collect();
    }
    let idx = orig.methods.len() as u32;
    orig.methods.push(m);
    pm.methods.insert(comp_midx, idx);
    idx
}

fn transplant_inner_functions(orig: &mut AbcFile, compiled: &AbcFile, comp_code: &[u8], pm: &mut PoolMapping, scope_delta: i64, visited: &mut HashSet<u32>) {
    for inner in find_newfunction_refs(comp_code) {
        if pm.methods.contains_key(&inner) || visited.contains(&inner) {
            continue;
        }
        visited.insert(inner);
        let new_midx = add_compiled_method_info(orig, compiled, inner, pm);
        let Some(cb) = compiled.method_bodies.get(&inner) else { continue };
        transplant_inner_functions(orig, compiled, &cb.code, pm, scope_delta, visited);
        let (new_code, pos_map) = translate_bytecode(&cb.code, pm);
        let new_exc = translate_exceptions(&cb.exceptions, pm, Some(&pos_map));
        let new_traits: Vec<TraitInfo> = cb.traits.iter().map(|t| translate_trait(t, pm)).collect();
        let new_init = (cb.init_scope_depth as i64 + scope_delta).max(0) as u32;
        let pc = orig.methods[new_midx as usize].param_count;
        let (ms, lc, msc, _) = compute_body_metadata(&new_code, &new_exc, new_init, pc, Some(orig));
        orig.method_bodies.insert(
            new_midx,
            MethodBody {
                method_idx: new_midx,
                max_stack: ms.max(cb.max_stack),
                local_count: lc.max(cb.local_count),
                init_scope_depth: new_init,
                max_scope_depth: msc.max(new_init),
                code: new_code,
                exceptions: new_exc,
                traits: new_traits,
            },
        );
    }
}

// ---------------------------------------------------------------- difflib

/// Port of `difflib.SequenceMatcher(None, a, b, autojunk=False).get_opcodes()`.
pub fn diff_opcodes<T: Eq + std::hash::Hash>(a: &[T], b: &[T]) -> Vec<(&'static str, usize, usize, usize, usize)> {
    let mut b2j: HashMap<&T, Vec<usize>> = HashMap::new();
    for (j, x) in b.iter().enumerate() {
        b2j.entry(x).or_default().push(j);
    }
    let find_longest = |alo: usize, ahi: usize, blo: usize, bhi: usize| -> (usize, usize, usize) {
        let (mut besti, mut bestj, mut bestsize) = (alo, blo, 0usize);
        let mut j2len: HashMap<usize, usize> = HashMap::new();
        for i in alo..ahi {
            let mut newj2len: HashMap<usize, usize> = HashMap::new();
            if let Some(js) = b2j.get(&a[i]) {
                for &j in js {
                    if j < blo {
                        continue;
                    }
                    if j >= bhi {
                        break;
                    }
                    let k = if j > 0 { j2len.get(&(j - 1)).copied().unwrap_or(0) } else { 0 } + 1;
                    newj2len.insert(j, k);
                    if k > bestsize {
                        besti = i + 1 - k;
                        bestj = j + 1 - k;
                        bestsize = k;
                    }
                }
            }
            j2len = newj2len;
        }
        (besti, bestj, bestsize)
    };
    let (la, lb) = (a.len(), b.len());
    let mut queue = vec![(0usize, la, 0usize, lb)];
    let mut blocks: Vec<(usize, usize, usize)> = Vec::new();
    while let Some((alo, ahi, blo, bhi)) = queue.pop() {
        let (i, j, k) = find_longest(alo, ahi, blo, bhi);
        if k > 0 {
            blocks.push((i, j, k));
            if alo < i && blo < j {
                queue.push((alo, i, blo, j));
            }
            if i + k < ahi && j + k < bhi {
                queue.push((i + k, ahi, j + k, bhi));
            }
        }
    }
    blocks.sort();
    let mut merged: Vec<(usize, usize, usize)> = Vec::new();
    let (mut i1, mut j1, mut k1) = (0, 0, 0);
    for (i2, j2, k2) in blocks {
        if i1 + k1 == i2 && j1 + k1 == j2 {
            k1 += k2;
        } else {
            if k1 > 0 {
                merged.push((i1, j1, k1));
            }
            i1 = i2;
            j1 = j2;
            k1 = k2;
        }
    }
    if k1 > 0 {
        merged.push((i1, j1, k1));
    }
    merged.push((la, lb, 0));
    let (mut i, mut j) = (0, 0);
    let mut ops = Vec::new();
    for (ai, bj, size) in merged {
        let tag = if i < ai && j < bj {
            "replace"
        } else if i < ai {
            "delete"
        } else if j < bj {
            "insert"
        } else {
            ""
        };
        if !tag.is_empty() {
            ops.push((tag, i, ai, j, bj));
        }
        i = ai + size;
        j = bj + size;
        if size > 0 {
            ops.push(("equal", ai, i, bj, j));
        }
    }
    ops
}

// ---------------------------------------------------------------- source diff helpers

/// Double-quoted string literals of `src` (`"([^"\\]*(?:\\.[^"\\]*)*)"`), raw escaped text.
pub fn string_literals(src: &str) -> Vec<String> {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| regex::Regex::new(r#""([^"\\]*(?:\\.[^"\\]*)*)""#).unwrap());
    RE.captures_iter(src).map(|c| c[1].to_string()).collect()
}

/// `_extract_string_edits_from_source`: old literal -> new literal (insertion ordered).
pub fn extract_string_edits_from_source(orig: &str, edited: &str) -> Vec<(String, String)> {
    let o = string_literals(orig);
    let e = string_literals(edited);
    let mut edits: Vec<(String, String)> = Vec::new();
    for (tag, i1, i2, j1, j2) in diff_opcodes(&o, &e) {
        if tag == "replace" && i2 - i1 == j2 - j1 {
            for k in 0..i2 - i1 {
                if o[i1 + k] != e[j1 + k] {
                    if let Some(x) = edits.iter_mut().find(|x| x.0 == o[i1 + k]) {
                        x.1 = e[j1 + k].clone();
                    } else {
                        edits.push((o[i1 + k].clone(), e[j1 + k].clone()));
                    }
                }
            }
        }
    }
    edits
}

/// `extract_method_texts`: key (`constructor`, `method:n`, `get:n`, `set:n`) -> text.
pub fn extract_method_texts(source: &str, class_name: &str) -> Vec<(String, String)> {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(?:(?:public|private|protected|internal)\s+)?(?:(?:static|override|final)\s+)*function\s+(?:(get|set)\s+)?(\w+)\s*\(").unwrap()
    });
    let lines: Vec<&str> = source.split('\n').collect();
    let mut out: Vec<(String, String)> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some(m) = RE.captures(lines[i]) else {
            i += 1;
            continue;
        };
        let kind = m.get(1).map(|x| x.as_str());
        let name = m[2].to_string();
        let mut brace: i64 = 0;
        let mut func_lines: Vec<&str> = Vec::new();
        while i < lines.len() {
            let line = lines[i];
            func_lines.push(line);
            brace += line.matches('{').count() as i64 - line.matches('}').count() as i64;
            i += 1;
            if brace <= 0 && !func_lines.is_empty() && func_lines.join("").contains('{') {
                break;
            }
        }
        let text = func_lines.join("\n");
        let key = match kind {
            Some("get") => format!("get:{name}"),
            Some("set") => format!("set:{name}"),
            _ if !class_name.is_empty() && name == class_name => "constructor".to_string(),
            _ => format!("method:{name}"),
        };
        if let Some(e) = out.iter_mut().find(|e| e.0 == key) {
            e.1 = text;
        } else {
            out.push((key, text));
        }
    }
    out
}

pub fn detect_changed_methods(orig_src: &str, edited_src: &str, class_name: &str) -> Vec<String> {
    let om = extract_method_texts(orig_src, class_name);
    let em = extract_method_texts(edited_src, class_name);
    let mut changed: Vec<String> = Vec::new();
    for (k, v) in &em {
        if om.iter().find(|o| &o.0 == k).map_or(true, |o| &o.1 != v) {
            changed.push(k.clone());
        }
    }
    for (k, _) in &om {
        if !em.iter().any(|e| &e.0 == k) && !changed.contains(k) {
            changed.push(k.clone());
        }
    }
    changed.sort();
    changed
}

// ---------------------------------------------------------------- inline constant patch

#[derive(Clone, Debug, PartialEq)]
pub enum Cv {
    S(String),
    I(i32),
    U(u32),
    D(f64),
}

fn cv_key(c: &Cv) -> String {
    match c {
        Cv::S(s) => format!("s:{s}"),
        Cv::I(v) => format!("i:{v}"),
        Cv::U(v) => format!("u:{v}"),
        Cv::D(v) => format!("d:{}", py_float_repr(*v)),
    }
}

fn desc_name(d: D) -> &'static str {
    match d {
        D::Str => "string",
        D::DebugString => "debug_string",
        D::Int => "int",
        D::UInt => "uint",
        _ => "double",
    }
}

fn resolve_const(abc: &AbcFile, d: D, v: i64) -> Option<Cv> {
    if v <= 0 {
        return None;
    }
    let u = v as usize;
    match d {
        D::Str | D::DebugString => abc.strings.get(u).map(|s| Cv::S(s.clone())),
        D::Int => abc.integers.get(u).map(|&x| Cv::I(x)),
        D::UInt => abc.uintegers.get(u).map(|&x| Cv::U(x)),
        D::Double => abc.doubles.get(u).map(|&x| Cv::D(x)),
        _ => None,
    }
}

fn ensure_pool_value(abc: &mut AbcFile, d: D, val: &Cv) -> Option<u32> {
    match (d, val) {
        (D::Str | D::DebugString, Cv::S(s)) => Some(match abc.strings.iter().position(|x| x == s) {
            Some(i) => i as u32,
            None => {
                abc.strings.push(s.clone());
                (abc.strings.len() - 1) as u32
            }
        }),
        (D::Int, Cv::I(v)) => Some(match abc.integers.iter().position(|x| x == v) {
            Some(i) => i as u32,
            None => {
                abc.integers.push(*v);
                (abc.integers.len() - 1) as u32
            }
        }),
        (D::UInt, Cv::U(v)) => Some(match abc.uintegers.iter().position(|x| x == v) {
            Some(i) => i as u32,
            None => {
                abc.uintegers.push(*v);
                (abc.uintegers.len() - 1) as u32
            }
        }),
        (D::Double, Cv::D(v)) => Some(match abc.doubles.iter().position(|x| x == v || (x.is_nan() && v.is_nan())) {
            Some(i) => i as u32,
            None => {
                abc.doubles.push(*v);
                (abc.doubles.len() - 1) as u32
            }
        }),
        _ => None,
    }
}

fn const_sequence(abc: &AbcFile, code: &[u8]) -> Vec<(D, Cv)> {
    let mut out = Vec::new();
    for ins in parse_instructions(code) {
        for &(d, v) in &ins.operands {
            if d.is_const_pool() {
                if let Some(c) = resolve_const(abc, d, v) {
                    out.push((d, c));
                }
            }
        }
    }
    out
}

type Edits = Vec<((D, String), Cv)>;

fn compute_constant_edits(orig: &AbcFile, comp: &AbcFile, orig_code: &[u8], comp_code: &[u8]) -> Option<Edits> {
    let os = const_sequence(orig, orig_code);
    let cs = const_sequence(comp, comp_code);
    let ok: Vec<String> = os.iter().map(|(d, v)| format!("{}:{}", desc_name(*d), cv_key(v))).collect();
    let ck: Vec<String> = cs.iter().map(|(d, v)| format!("{}:{}", desc_name(*d), cv_key(v))).collect();
    let mut edits: Edits = Vec::new();
    let mut seq_ok = true;
    'o: for (tag, i1, i2, j1, j2) in diff_opcodes(&ok, &ck) {
        if tag == "equal" {
            continue;
        }
        if tag == "replace" && i2 - i1 == j2 - j1 {
            for k in 0..i2 - i1 {
                let (od, ov) = &os[i1 + k];
                let (nd, nv) = &cs[j1 + k];
                if od != nd {
                    seq_ok = false;
                    break 'o;
                }
                let key = (*od, cv_key(ov));
                if let Some(e) = edits.iter_mut().find(|e| e.0 == key) {
                    e.1 = nv.clone();
                } else {
                    edits.push((key, nv.clone()));
                }
            }
        } else {
            seq_ok = false;
            break;
        }
    }
    if seq_ok {
        return Some(edits);
    }
    // bag diff
    let count = |seq: &[(D, Cv)]| -> Vec<((D, String), (Cv, i64))> {
        let mut v: Vec<((D, String), (Cv, i64))> = Vec::new();
        for (d, c) in seq {
            let key = (*d, cv_key(c));
            if let Some(e) = v.iter_mut().find(|e| e.0 == key) {
                e.1 .1 += 1;
            } else {
                v.push((key, (c.clone(), 1)));
            }
        }
        v
    };
    let oc = count(&os);
    let cc = count(&cs);
    let sub = |a: &[((D, String), (Cv, i64))], b: &[((D, String), (Cv, i64))]| -> Vec<(D, Cv, i64)> {
        a.iter()
            .filter_map(|(k, (c, n))| {
                let m = b.iter().find(|e| &e.0 == k).map_or(0, |e| e.1 .1);
                if n - m > 0 {
                    Some((k.0, c.clone(), n - m))
                } else {
                    None
                }
            })
            .collect()
    };
    let removed = sub(&oc, &cc);
    let added = sub(&cc, &oc);
    let mut descs: Vec<D> = Vec::new();
    for (d, _, _) in removed.iter().chain(added.iter()) {
        if !descs.contains(d) {
            descs.push(*d);
        }
    }
    let mut bag: Edits = Vec::new();
    for d in descs {
        let mut rv: Vec<Cv> = removed.iter().filter(|e| e.0 == d).flat_map(|e| std::iter::repeat(e.1.clone()).take(e.2 as usize)).collect();
        let mut av: Vec<Cv> = added.iter().filter(|e| e.0 == d).flat_map(|e| std::iter::repeat(e.1.clone()).take(e.2 as usize)).collect();
        if rv.len() != av.len() {
            return None;
        }
        let strkey = |c: &Cv| match c {
            Cv::S(s) => s.clone(),
            Cv::I(v) => v.to_string(),
            Cv::U(v) => v.to_string(),
            Cv::D(v) => py_float_repr(*v),
        };
        rv.sort_by_key(|c| strkey(c));
        av.sort_by_key(|c| strkey(c));
        for (o, n) in rv.into_iter().zip(av) {
            if o != n {
                let key = (d, cv_key(&o));
                if let Some(e) = bag.iter_mut().find(|e| e.0 == key) {
                    e.1 = n;
                } else {
                    bag.push((key, n));
                }
            }
        }
    }
    Some(bag)
}

/// Apply constant edits to the *original* bytecode, keeping control flow,
/// exception handlers, scope depths and local counts.
pub fn apply_const_edits(orig: &mut AbcFile, body: &MethodBody, edits: &Edits) -> MethodBody {
    if edits.is_empty() {
        return body.clone();
    }
    let instrs = parse_instructions(&body.code);
    let seq = const_sequence(orig, &body.code);
    let mut remaining: HashMap<(D, String), i64> = HashMap::new();
    for (k, _) in edits {
        remaining.insert(k.clone(), seq.iter().filter(|(d, c)| (*d, cv_key(c)) == *k).count() as i64);
    }
    let mut ops: Vec<Vec<(D, i64)>> = instrs.iter().map(|i| i.operands.clone()).collect();
    for (ii, ins) in instrs.iter().enumerate() {
        for (oi, &(d, v)) in ins.operands.iter().enumerate() {
            if d.is_const_pool() {
                if let Some(c) = resolve_const(orig, d, v) {
                    let key = (d, cv_key(&c));
                    if let Some((_, nv)) = edits.iter().find(|e| e.0 == key) {
                        if remaining.get(&key).copied().unwrap_or(0) > 0 {
                            if let Some(ni) = ensure_pool_value(orig, d, nv) {
                                ops[ii][oi].1 = ni as i64;
                                *remaining.get_mut(&key).unwrap() -= 1;
                            }
                        }
                    }
                }
            }
        }
    }
    let (new_code, o2n) = relayout(body.code.len(), &instrs, &ops);
    let exc = body
        .exceptions
        .iter()
        .map(|e| ExceptionInfo {
            from_pos: o2n.get(&(e.from_pos as usize)).map_or(e.from_pos, |&x| x as u32),
            to_pos: o2n.get(&(e.to_pos as usize)).map_or(e.to_pos, |&x| x as u32),
            target: o2n.get(&(e.target as usize)).map_or(e.target, |&x| x as u32),
            exc_type: e.exc_type,
            var_name: e.var_name,
        })
        .collect();
    MethodBody { code: new_code, exceptions: exc, ..body.clone() }
}

/// `_try_inline_constant_patch`.
pub fn try_inline_constant_patch(
    orig: &mut AbcFile,
    compiled: &AbcFile,
    orig_body: &MethodBody,
    comp_body: &MethodBody,
    known: Option<&[(String, String)]>,
) -> Option<MethodBody> {
    let oi = parse_instructions(&orig_body.code);
    let ci = parse_instructions(&comp_body.code);
    if oi.len() != ci.len() || oi.iter().zip(&ci).any(|(a, b)| a.op != b.op) {
        return None;
    }
    let edits: Edits = match known {
        Some(k) if !k.is_empty() => k.iter().map(|(o, n)| ((D::Str, cv_key(&Cv::S(o.clone()))), Cv::S(n.clone()))).collect(),
        _ => compute_constant_edits(orig, compiled, &orig_body.code, &comp_body.code)?,
    };
    Some(apply_const_edits(orig, orig_body, &edits))
}

// ---------------------------------------------------------------- transplant

fn omap_put<K: PartialEq, V>(m: &mut Vec<(K, V)>, k: K, v: V) {
    if let Some(e) = m.iter_mut().find(|e| e.0 == k) {
        e.1 = v;
    } else {
        m.push((k, v));
    }
}

fn find_class_index(abc: &AbcFile, full: &str) -> Option<usize> {
    abc.instances.iter().position(|i| abc.mn_full(i.name_idx) == full)
}

fn match_key(abc: &AbcFile, name_idx: u32) -> String {
    if name_idx == 0 || name_idx as usize >= abc.multinames.len() {
        return "*".into();
    }
    let m = &abc.multinames[name_idx as usize];
    if matches!(m.kind, MN_QNAME | MN_QNAME_A) && !m.data.is_empty() {
        let ns = m.data[0];
        if ns > 0 && (ns as usize) < abc.namespaces.len() && abc.namespaces[ns as usize].0 == NS_PRIVATE {
            return format!("<private>.{}", abc.mn_name(name_idx));
        }
    }
    abc.mn_full(name_idx)
}

type FullMap = Vec<((String, u8), u32)>;

fn trait_map(traits: &[TraitInfo], abc: &AbcFile) -> FullMap {
    let mut full: FullMap = Vec::new();
    for t in traits {
        if kind_prefix(t.kind).is_some() {
            omap_put(&mut full, (match_key(abc, t.name_idx), t.kind), t.method_idx);
        }
    }
    full
}

fn short_key(abc: &AbcFile, traits: &[TraitInfo], kind: u8, midx: u32) -> String {
    let t = traits.iter().find(|t| kind_prefix(t.kind).is_some() && t.method_idx == midx).expect("trait");
    format!("{}:{}", kind_prefix(kind).unwrap(), abc.mn_name(t.name_idx))
}

/// Transplant one class's compiled method bodies into the original ABC.
/// Returns `(patched_abc, log)`.
pub fn transplant_class(
    orig_data: &[u8],
    compiled_data: &[u8],
    class_full_name: &str,
    changed_methods: Option<&[String]>,
    source_string_edits: Option<&[(String, String)]>,
) -> Result<(Vec<u8>, Vec<String>), String> {
    let mut orig = AbcFile::parse(orig_data)?;
    let compiled = AbcFile::parse(compiled_data)?;
    catch(|| transplant_inner(&mut orig, &compiled, class_full_name, changed_methods, source_string_edits))?
}

fn transplant_inner(
    orig: &mut AbcFile,
    compiled: &AbcFile,
    class_full_name: &str,
    changed: Option<&[String]>,
    string_edits: Option<&[(String, String)]>,
) -> Result<(Vec<u8>, Vec<String>), String> {
    let mut log: Vec<String> = Vec::new();
    let oci = find_class_index(orig, class_full_name).ok_or_else(|| format!("Class '{class_full_name}' not found in original ABC"))?;
    let cci = find_class_index(compiled, class_full_name).ok_or_else(|| format!("Class '{class_full_name}' not found in compiled ABC"))?;
    let ns_over = build_private_ns_overrides(orig, compiled);
    for (c, o) in &ns_over {
        log.push(format!("PrivateNs override: compiled ns[{c}] -> original ns[{o}]"));
    }
    let mut pm = merge_pools(orig, compiled, &ns_over);
    let (o_inst, o_cls) = (orig.instances[oci].clone(), orig.classes[oci].clone());
    let (c_inst, c_cls) = (compiled.instances[cci].clone(), compiled.classes[cci].clone());

    let o_inst_full = trait_map(&o_inst.traits, orig);
    let o_cls_full = trait_map(&o_cls.traits, orig);
    let c_inst_full = trait_map(&c_inst.traits, compiled);
    let c_cls_full = trait_map(&c_cls.traits, compiled);

    let mut method_pairs: Vec<(u32, (u32, String))> = Vec::new();
    omap_put(&mut method_pairs, c_inst.iinit, (o_inst.iinit, "constructor".to_string()));
    pm.methods.insert(c_inst.iinit, o_inst.iinit);
    omap_put(&mut method_pairs, c_cls.cinit, (o_cls.cinit, "cinit".to_string()));
    pm.methods.insert(c_cls.cinit, o_cls.cinit);
    for (fk, cm) in &c_inst_full {
        if let Some((_, om)) = o_inst_full.iter().find(|e| &e.0 == fk) {
            pm.methods.insert(*cm, *om);
            omap_put(&mut method_pairs, *cm, (*om, short_key(compiled, &c_inst.traits, fk.1, *cm)));
        }
    }
    for (fk, cm) in &c_cls_full {
        if let Some((_, om)) = o_cls_full.iter().find(|e| &e.0 == fk) {
            pm.methods.insert(*cm, *om);
            omap_put(&mut method_pairs, *cm, (*om, short_key(compiled, &c_cls.traits, fk.1, *cm)));
        }
    }

    // inheritance / interfaces
    let comp_super = compiled.mn_full(c_inst.super_idx);
    let orig_super = orig.mn_full(o_inst.super_idx);
    if comp_super != orig_super && c_inst.super_idx != 0 {
        log.push(format!("Superclass changed: {orig_super} -> {comp_super}"));
        orig.instances[oci].super_idx = g(&pm.multinames, c_inst.super_idx);
    }
    let cif: HashSet<String> = c_inst.interfaces.iter().map(|&i| compiled.mn_full(i)).collect();
    let oif: HashSet<String> = o_inst.interfaces.iter().map(|&i| orig.mn_full(i)).collect();
    if cif != oif {
        log.push("Interfaces changed".into());
        orig.instances[oci].interfaces = c_inst.interfaces.iter().map(|&i| g(&pm.multinames, i)).collect();
    }
    if c_inst.flags & INSTANCE_PROTECTED_NS != 0 && c_inst.protected_ns != 0 {
        let mp = g(&pm.namespaces, c_inst.protected_ns);
        if orig.instances[oci].protected_ns != mp {
            orig.instances[oci].protected_ns = mp;
        }
    }

    // signatures
    for (cm, (om, key)) in &method_pairs {
        let cmi = compiled.methods[*cm as usize].clone();
        let omi = orig.methods[*om as usize].clone();
        let mut sig = false;
        if cmi.param_count != omi.param_count {
            sig = true;
        } else if cmi.return_type != 0 && omi.return_type != 0 && compiled.mn_full(cmi.return_type) != orig.mn_full(omi.return_type) {
            sig = true;
        }
        if sig {
            log.push(format!("Signature changed for '{key}': params {}->{}", omi.param_count, cmi.param_count));
            let o = &mut orig.methods[*om as usize];
            o.param_count = cmi.param_count;
            o.return_type = g(&pm.multinames, cmi.return_type);
            o.param_types = cmi.param_types.iter().map(|&p| g(&pm.multinames, p)).collect();
            o.flags = cmi.flags;
            o.optional_values = if cmi.flags & METHOD_HAS_OPTIONAL != 0 {
                cmi.optional_values.iter().map(|&(vk, v)| (vk, remap_vindex(vk, v, &pm))).collect()
            } else {
                vec![]
            };
            o.param_names =
                if cmi.flags & METHOD_HAS_PARAM_NAMES != 0 { cmi.param_names.iter().map(|&p| g(&pm.strings, p)).collect() } else { vec![] };
        }
    }

    // new methods
    let mut new_methods = 0;
    let in_changed = |k: &str| changed.map_or(true, |c| c.iter().any(|x| x == k));
    for (is_static, c_full, o_full) in [(false, &c_inst_full, &o_inst_full), (true, &c_cls_full, &o_cls_full)] {
        for (fk, cm) in c_full.iter() {
            if o_full.iter().any(|e| &e.0 == fk) {
                continue;
            }
            let c_traits = if is_static { &c_cls.traits } else { &c_inst.traits };
            let ct = c_traits.iter().find(|t| kind_prefix(t.kind).is_some() && t.method_idx == *cm).unwrap();
            let src_key = format!("{}:{}", kind_prefix(ct.kind).unwrap_or("method"), compiled.mn_name(ct.name_idx));
            if !in_changed(&src_key) {
                continue;
            }
            log.push(format!("Adding new {} method: {src_key}", if is_static { "static" } else { "instance" }));
            let new_midx = add_compiled_method_info(orig, compiled, *cm, &mut pm);
            let mut nt = translate_trait(ct, &pm);
            nt.method_idx = new_midx;
            if is_static {
                orig.classes[oci].traits.push(nt);
            } else {
                orig.instances[oci].traits.push(nt);
            }
            omap_put(&mut method_pairs, *cm, (new_midx, src_key));
            new_methods += 1;
        }
    }

    // new slots
    let mut new_slots = 0;
    let slot_keys = |traits: &[TraitInfo], abc: &AbcFile| -> Vec<String> {
        traits.iter().filter(|t| matches!(t.kind, TRAIT_SLOT | TRAIT_CONST)).map(|t| match_key(abc, t.name_idx)).collect()
    };
    let o_inst_slots = slot_keys(&o_inst.traits, orig);
    let o_cls_slots = slot_keys(&o_cls.traits, orig);
    for (is_static, traits, existing) in [(false, &c_inst.traits, &o_inst_slots), (true, &c_cls.traits, &o_cls_slots)] {
        let mut seen: Vec<String> = Vec::new();
        for t in traits.iter().filter(|t| matches!(t.kind, TRAIT_SLOT | TRAIT_CONST)) {
            let k = match_key(compiled, t.name_idx);
            if seen.contains(&k) {
                continue; // dict keyed by name: later duplicates replace; keep first position
            }
            seen.push(k.clone());
            if !existing.contains(&k) {
                log.push(format!("Adding new {} slot: {}", if is_static { "static" } else { "instance" }, compiled.mn_full(t.name_idx)));
                // Python keeps the *last* trait for a duplicated name
                let last = traits.iter().rev().find(|x| matches!(x.kind, TRAIT_SLOT | TRAIT_CONST) && match_key(compiled, x.name_idx) == k).unwrap();
                let mut nt = translate_trait(last, &pm);
                nt.slot_id = 0;
                if is_static {
                    orig.classes[oci].traits.push(nt);
                } else {
                    orig.instances[oci].traits.push(nt);
                }
                new_slots += 1;
            }
        }
    }

    // removals on full transplant
    let mut removed = 0;
    if changed.is_none() {
        let ci_names: HashSet<String> = c_inst.traits.iter().map(|t| match_key(compiled, t.name_idx)).collect();
        let cc_names: HashSet<String> = c_cls.traits.iter().map(|t| match_key(compiled, t.name_idx)).collect();
        let ot = std::mem::take(&mut orig.instances[oci].traits);
        let keep: Vec<TraitInfo> = ot
            .into_iter()
            .filter(|t| {
                // new traits just appended are in compiled by construction
                let ok = ci_names.contains(&match_key_any(orig, compiled, t));
                if !ok {
                    removed += 1;
                }
                ok
            })
            .collect();
        orig.instances[oci].traits = keep;
        let ot = std::mem::take(&mut orig.classes[oci].traits);
        let keep: Vec<TraitInfo> = ot
            .into_iter()
            .filter(|t| {
                let ok = cc_names.contains(&match_key_any(orig, compiled, t));
                if !ok {
                    removed += 1;
                }
                ok
            })
            .collect();
        orig.classes[oci].traits = keep;
    }

    // bodies
    let mut transplanted = 0;
    let mut visited: HashSet<u32> = HashSet::new();
    for (cm, (om, key)) in method_pairs.clone() {
        if !in_changed(&key) {
            continue;
        }
        let Some(comp_body) = compiled.method_bodies.get(&cm).cloned() else { continue };
        let orig_body = orig.method_bodies.get(&om).cloned();
        if let Some(ob) = &orig_body {
            if let Some(r) = try_inline_constant_patch(orig, compiled, ob, &comp_body, string_edits) {
                orig.method_bodies.insert(om, r);
                transplanted += 1;
                continue;
            }
            if !ob.exceptions.is_empty() && comp_body.exceptions.is_empty() {
                log.push(format!("WARNING: method '{key}' (#{om}) lost {} exception handler(s)", ob.exceptions.len()));
            }
        }
        let (scope_delta, new_init) = match &orig_body {
            Some(ob) => (ob.init_scope_depth as i64 - comp_body.init_scope_depth as i64, ob.init_scope_depth),
            None => (0, comp_body.init_scope_depth),
        };
        transplant_inner_functions(orig, compiled, &comp_body.code, &mut pm, scope_delta, &mut visited);
        let (new_code, pos_map) = translate_bytecode(&comp_body.code, &pm);
        let mut new_exc = translate_exceptions(&comp_body.exceptions, &pm, Some(&pos_map));
        if let Some(ob) = &orig_body {
            if ob.exceptions.len() == new_exc.len() {
                for (n, o) in new_exc.iter_mut().zip(&ob.exceptions) {
                    n.var_name = o.var_name;
                    n.exc_type = o.exc_type;
                }
            }
        }
        let new_traits: Vec<TraitInfo> = comp_body.traits.iter().map(|t| translate_trait(t, &pm)).collect();
        let pc = orig.methods[om as usize].param_count;
        let (ms, lc, msc, _) = compute_body_metadata(&new_code, &new_exc, new_init, pc, Some(orig));
        orig.method_bodies.insert(
            om,
            MethodBody {
                method_idx: om,
                max_stack: ms.max(comp_body.max_stack),
                local_count: lc.max(comp_body.local_count),
                init_scope_depth: new_init,
                max_scope_depth: msc.max(new_init),
                code: new_code,
                exceptions: new_exc,
                traits: new_traits,
            },
        );
        transplanted += 1;
    }
    if transplanted == 0 {
        if let Some(c) = changed {
            if !c.is_empty() {
                return Err(format!("No method bodies transplanted for '{class_full_name}' (changed_methods={c:?})"));
            }
        }
    }
    log.push(format!("{class_full_name}: transplanted={transplanted} new_methods={new_methods} new_slots={new_slots} removed={removed}"));
    Ok((orig.serialize(), log))
}

/// Match key for a trait that lives in `orig` after its pool was extended by the merge
/// (merged entries keep indices valid in `orig`, so keying through `orig` is right).
fn match_key_any(orig: &AbcFile, _compiled: &AbcFile, t: &TraitInfo) -> String {
    match_key(orig, t.name_idx)
}

// ---------------------------------------------------------------- SWF container

#[derive(Clone, Debug)]
pub struct SwfFull {
    pub version: u8,
    pub rect_bytes: Vec<u8>,
    pub frame_rate: u16,
    pub frame_count: u16,
    pub compressed: bool,
    pub tags: Vec<(u16, Vec<u8>)>,
}

pub fn read_swf_full(bytes: &[u8]) -> Result<SwfFull, String> {
    if bytes.len() < 8 {
        return Err("File too short to be a SWF".into());
    }
    let body = match &bytes[..3] {
        b"FWS" => bytes[8..].to_vec(),
        b"CWS" => ac_swf::unzlib(&bytes[8..])?,
        b"ZWS" => return Err("LZMA-compressed SWF (ZWS) is not supported".into()),
        _ => return Err("Not a SWF file".into()),
    };
    let first = *body.first().ok_or("truncated SWF")?;
    let nbits = (first >> 3) as usize & 0x1F;
    let rect_len = (5 + nbits * 4 + 7) / 8;
    if body.len() < rect_len + 4 {
        return Err("truncated SWF".into());
    }
    let rect_bytes = body[..rect_len].to_vec();
    let frame_rate = u16::from_le_bytes([body[rect_len], body[rect_len + 1]]);
    let frame_count = u16::from_le_bytes([body[rect_len + 2], body[rect_len + 3]]);
    let tags = ac_swf::parse_tags(&body[rect_len + 4..]).into_iter().filter(|t| t.code != ac_swf::END).map(|t| (t.code, t.body)).collect();
    Ok(SwfFull { version: bytes[3], rect_bytes, frame_rate, frame_count, compressed: bytes[0] == b'C' || bytes[0] == b'Z', tags })
}

pub fn write_swf_full(s: &SwfFull) -> Vec<u8> {
    let mut body = s.rect_bytes.clone();
    body.extend_from_slice(&s.frame_rate.to_le_bytes());
    body.extend_from_slice(&s.frame_count.to_le_bytes());
    for (code, b) in &s.tags {
        body.extend_from_slice(&ac_swf::tag_bytes(*code, b, false));
    }
    body.extend_from_slice(&ac_swf::tag_bytes(0, &[], false));
    let total = (8 + body.len()) as u32;
    let mut out = Vec::new();
    out.extend_from_slice(if s.compressed { b"CWS" } else { b"FWS" });
    out.push(s.version);
    out.extend_from_slice(&total.to_le_bytes());
    if s.compressed {
        out.extend_from_slice(&ac_swf::zlib(&body));
    } else {
        out.extend_from_slice(&body);
    }
    out
}

fn abc_tag_indices(tags: &[(u16, Vec<u8>)]) -> Vec<usize> {
    tags.iter().enumerate().filter(|(_, t)| t.0 == TAG_DOABC || t.0 == TAG_DOABC2).map(|(i, _)| i).collect()
}

/// Split a DoABC / DoABC2 body: `(prefix_bytes, abc)`. DoABC2 keeps flags + name.
fn split_abc_tag(code: u16, body: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    if code == TAG_DOABC2 {
        if body.len() < 5 {
            return None;
        }
        let nul = body[4..].iter().position(|&b| b == 0)? + 4;
        Some((body[..nul + 1].to_vec(), body[nul + 1..].to_vec()))
    } else {
        Some((vec![], body.to_vec()))
    }
}

/// Rewrite AS3 constant strings across every ABC block. Everything else stays
/// byte-identical. Returns `(swf, replaced, blocks)`.
pub fn patch_strings(swf: &[u8], mapping: &HashMap<String, String>) -> Result<(Vec<u8>, usize, usize), String> {
    if mapping.is_empty() {
        return Err("patch contains no \"strings\" entries".into());
    }
    catch(|| -> Result<(Vec<u8>, usize, usize), String> {
        let mut s = read_swf_full(swf)?;
        let (mut replaced, mut blocks) = (0, 0);
        for i in 0..s.tags.len() {
            let (code, body) = (s.tags[i].0, s.tags[i].1.clone());
            if code != TAG_DOABC && code != TAG_DOABC2 {
                continue;
            }
            let Some((prefix, abc_data)) = split_abc_tag(code, &body) else { continue };
            let mut ed = AbcEditor::new(&abc_data)?;
            let mut hits = 0;
            for idx in 1..ed.abc.strings.len() {
                if let Some(nv) = mapping.get(&ed.abc.strings[idx]) {
                    if *nv != ed.abc.strings[idx] {
                        ed.abc.strings[idx] = nv.clone();
                        hits += 1;
                    }
                }
            }
            if hits == 0 {
                continue;
            }
            let mut nb = prefix;
            nb.extend_from_slice(&ed.abc.serialize());
            s.tags[i].1 = nb;
            replaced += hits;
            blocks += 1;
        }
        if replaced == 0 {
            return Err("no matching AS3 constant strings found in this SWF".into());
        }
        Ok((write_swf_full(&s), replaced, blocks))
    })?
}

/// Undo `escape_str` for a literal taken from decompiled source.
pub fn unescape_literal(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('f') => out.push('\x0c'),
            Some('0') => out.push('\0'),
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            Some('x') => {
                let h: String = it.by_ref().take(2).collect();
                out.push(u8::from_str_radix(&h, 16).map(|b| b as char).unwrap_or('?'));
            }
            Some('u') => {
                let h: String = it.by_ref().take(4).collect();
                out.push(u32::from_str_radix(&h, 16).ok().and_then(char::from_u32).unwrap_or('?'));
            }
            Some(o) => {
                out.push('\\');
                out.push(o);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn blank_literals(s: &str) -> String {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| regex::Regex::new(r#""([^"\\]*(?:\\.[^"\\]*)*)""#).unwrap());
    RE.replace_all(s, "\"\"").lines().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n")
}

#[derive(Debug)]
pub struct ClassPatch {
    pub swf: Vec<u8>,
    pub changed_methods: Vec<String>,
    pub replaced_strings: usize,
}

/// Write an edited class back into the SWF *without a compiler* when the edit
/// is expressible: the edited source may differ from the decompiled source
/// only in string literals. Anything else is `Err("needs compiler: ...")`.
pub fn patch_class_source(swf: &[u8], block_index: usize, class_idx: usize, edited_source: &str) -> Result<ClassPatch, String> {
    catch(|| -> Result<ClassPatch, String> {
        let mut s = read_swf_full(swf)?;
        let idxs = abc_tag_indices(&s.tags);
        let ti = *idxs.get(block_index).ok_or_else(|| format!("ABC block {block_index} not found (SWF has {} blocks)", idxs.len()))?;
        let (code, body) = (s.tags[ti].0, s.tags[ti].1.clone());
        let (prefix, abc_data) = split_abc_tag(code, &body).ok_or("malformed DoABC tag")?;
        let mut abc = AbcFile::parse(&abc_data)?;
        let (class_name, orig_src) = {
            let dec = As3Decompiler::new(&abc);
            let l = dec.list_classes();
            let c = l.get(class_idx).ok_or_else(|| format!("class index {class_idx} not found"))?;
            (c.name.clone(), dec.decompile_class(class_idx)?)
        };
        if blank_literals(&orig_src) != blank_literals(edited_source) {
            return Err("needs compiler: the edit changes more than string literals".into());
        }
        let edits_src = extract_string_edits_from_source(&orig_src, edited_source);
        if edits_src.is_empty() {
            return Err("no string literal changes found".into());
        }
        let edits: Edits = edits_src
            .iter()
            .map(|(o, n)| ((D::Str, cv_key(&Cv::S(unescape_literal(o)))), Cv::S(unescape_literal(n))))
            .collect();
        let changed = detect_changed_methods(&orig_src, edited_source, &class_name);
        let ed = AbcEditor { abc: abc.clone() };
        let methods = ed.get_class_methods(class_idx);
        let mut targets: Vec<u32> = Vec::new();
        for key in &changed {
            for (k, mi) in &methods {
                let kk = k.strip_prefix("static:").unwrap_or(k);
                if kk == key && !targets.contains(mi) {
                    targets.push(*mi);
                }
            }
        }
        let mut replaced = 0;
        for mi in targets {
            let Some(b) = abc.method_bodies.get(&mi).cloned() else { continue };
            let nb = apply_const_edits(&mut abc, &b, &edits);
            if nb.code != b.code {
                replaced += 1;
                abc.method_bodies.insert(mi, nb);
            }
        }
        if replaced == 0 {
            return Err("string literals did not match any constant in the changed methods".into());
        }
        let mut nbody = prefix;
        nbody.extend_from_slice(&abc.serialize());
        s.tags[ti].1 = nbody;
        Ok(ClassPatch { swf: write_swf_full(&s), changed_methods: changed, replaced_strings: replaced })
    })?
}

// ---------------------------------------------------------------- mxmlc

const FLEX_ENV: [&str; 3] = ["FLARE_FLEX_SDK", "FLEX_SDK", "AIR_SDK"];

/// Locate the Flex SDK's `mxmlc` (env overrides, usual install dirs, then PATH).
pub fn find_mxmlc() -> Option<std::path::PathBuf> {
    let mut dirs: Vec<std::path::PathBuf> = FLEX_ENV.iter().filter_map(|v| std::env::var_os(v)).map(Into::into).collect();
    for d in ["/opt/flex-sdk", "/opt/air-sdk", "/usr/local/flex-sdk", "/Applications/Adobe Flex SDK", "C:/flex-sdk", "C:/air-sdk"] {
        dirs.push(d.into());
    }
    for d in dirs {
        for n in ["mxmlc.bat", "mxmlc", "mxmlc.exe", "mxmlc.sh"] {
            let p = d.join("bin").join(n);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    let path = std::env::var_os("PATH")?;
    for d in std::env::split_paths(&path) {
        for n in ["mxmlc", "mxmlc.bat", "mxmlc.exe"] {
            let p = d.join(n);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

fn collect_as(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect_as(&p, out);
            } else if p.extension().map_or(false, |x| x == "as") {
                out.push(p);
            }
        }
    }
}

/// Compile `.as` sources with the Flex SDK's mxmlc (the only step that needs Flex).
pub fn compile_sources(src_dir: &std::path::Path, out_swf: &std::path::Path) -> Result<String, String> {
    if !src_dir.is_dir() {
        return Err(format!("not a directory: {}", src_dir.display()));
    }
    let mut sources = Vec::new();
    collect_as(src_dir, &mut sources);
    sources.sort();
    if sources.is_empty() {
        return Err(format!("no .as sources found in {}", src_dir.display()));
    }
    let mxmlc = find_mxmlc().ok_or("AS3 compilation needs the Flex SDK (mxmlc). Set FLARE_FLEX_SDK to the SDK root or put mxmlc on PATH.")?;
    if let Some(p) = out_swf.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    let out = std::process::Command::new(&mxmlc)
        .arg("-source-path")
        .arg(src_dir)
        .arg("-output")
        .arg(out_swf)
        .args(&sources)
        .output()
        .map_err(|e| format!("mxmlc could not be run: {e}"))?;
    let log = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    if !out.status.success() || !out_swf.is_file() {
        let tail: String = log.chars().rev().take(8000).collect::<Vec<_>>().into_iter().rev().collect();
        return Err(format!("mxmlc exited with status {:?}\n{tail}", out.status.code()));
    }
    Ok(log)
}

#[allow(dead_code)]
fn _keep(s: &str) -> String {
    escape_str(s)
}
