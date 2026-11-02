//! AVM2 / ABC core: binary readers and writers, the ABC file model (constant
//! pools, methods, traits, classes, scripts, method bodies), the opcode table,
//! instruction decoding/encoding, a disassembler, an assembler and the
//! constant-pool editing API.
//!
//! Derived from Next2Flash's `app/lib/as3_decompiler/{abc_parser,opcodes,
//! abc_editor,swf_reader}.py` and the serializer / instruction tables of
//! `abc_patcher.py` (https://github.com/SSF2-Mods-Official/Next2Flash, MIT,
//! Copyright (c) 2026 masterwebx; see `flare/sources/THIRD-PARTY-NOTICES.md`).
//! The logic is a faithful Rust port of that code; the Python original is the
//! oracle for the golden tests in `ac_as3_tests.rs` (`tests/native/gen_as3_cases.py`).
//!
//! The SWF container itself is *not* re-implemented here: DoABC blocks are
//! pulled out of the tag list produced by `ac_swf.rs`.

use std::collections::HashMap;
use std::panic;

// ---------------------------------------------------------------- errors

/// Marker payload used to unwind out of a reader that ran past the end of its
/// buffer (the Rust counterpart of Python's `IndexError`). Raised with
/// `resume_unwind`, which does not run the panic hook, so a malformed file
/// never prints a backtrace; callers turn it into an `Err` with [`catch`].
#[derive(Debug)]
pub struct Oob;

#[cold]
pub fn oob() -> ! {
    panic::resume_unwind(Box::new(Oob))
}

/// Run `f`, converting an [`Oob`] unwind or any panic into an `Err(String)`.
pub fn catch<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    match panic::catch_unwind(panic::AssertUnwindSafe(f)) {
        Ok(v) => Ok(v),
        Err(e) => {
            if e.downcast_ref::<Oob>().is_some() {
                Err("index out of range (truncated or malformed data)".into())
            } else if let Some(s) = e.downcast_ref::<String>() {
                Err(s.clone())
            } else if let Some(s) = e.downcast_ref::<&str>() {
                Err((*s).to_string())
            } else {
                Err("internal error".into())
            }
        }
    }
}

// ---------------------------------------------------------------- readers

#[inline]
pub fn at(d: &[u8], p: usize) -> u8 {
    match d.get(p) {
        Some(b) => *b,
        None => oob(),
    }
}
#[inline]
pub fn ru8(d: &[u8], p: usize) -> (u8, usize) {
    (at(d, p), p + 1)
}
pub fn ru16(d: &[u8], p: usize) -> (u16, usize) {
    (u16::from_le_bytes([at(d, p), at(d, p + 1)]), p + 2)
}
pub fn rs24(d: &[u8], p: usize) -> (i32, usize) {
    let mut v = at(d, p) as i32 | (at(d, p + 1) as i32) << 8 | (at(d, p + 2) as i32) << 16;
    if v & 0x80_0000 != 0 {
        v -= 0x100_0000;
    }
    (v, p + 3)
}
/// Variable-length u30 (masked to 30 bits like the original).
pub fn ru30(d: &[u8], mut p: usize) -> (u32, usize) {
    let mut result: u64 = 0;
    let mut shift = 0;
    for _ in 0..5 {
        let b = at(d, p);
        p += 1;
        result |= ((b & 0x7F) as u64) << shift;
        if b & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    ((result & 0x3FFF_FFFF) as u32, p)
}
/// Variable-length u32 (full 32 bits).
pub fn ru32(d: &[u8], mut p: usize) -> (u32, usize) {
    let mut result: u64 = 0;
    let mut shift = 0;
    for _ in 0..5 {
        let b = at(d, p);
        p += 1;
        result |= ((b & 0x7F) as u64) << shift;
        if b & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    ((result & 0xFFFF_FFFF) as u32, p)
}
pub fn rd64(d: &[u8], p: usize) -> (f64, usize) {
    let mut a = [0u8; 8];
    for (i, x) in a.iter_mut().enumerate() {
        *x = at(d, p + i);
    }
    (f64::from_le_bytes(a), p + 8)
}

// ---------------------------------------------------------------- writers

pub fn wu8(buf: &mut Vec<u8>, v: i64) {
    buf.push((v & 0xFF) as u8);
}
pub fn wu16(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&((v & 0xFFFF) as u16).to_le_bytes());
}
pub fn ws24(buf: &mut Vec<u8>, mut v: i64) {
    if v < 0 {
        v += 0x100_0000;
    }
    buf.push((v & 0xFF) as u8);
    buf.push(((v >> 8) & 0xFF) as u8);
    buf.push(((v >> 16) & 0xFF) as u8);
}
/// Encode a u30/u32 variable-length integer (masked to 32 bits).
pub fn wu30(buf: &mut Vec<u8>, v: i64) {
    let mut v = (v & 0xFFFF_FFFF) as u64;
    loop {
        let byte = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            buf.push(byte | 0x80);
        } else {
            buf.push(byte);
            break;
        }
    }
}
pub fn wd64(buf: &mut Vec<u8>, v: f64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

// ---------------------------------------------------------------- constants

pub const NS_NAMESPACE: u8 = 0x08;
pub const NS_PACKAGE: u8 = 0x16;
pub const NS_PACKAGE_INTERNAL: u8 = 0x17;
pub const NS_PROTECTED: u8 = 0x18;
pub const NS_EXPLICIT: u8 = 0x19;
pub const NS_STATIC_PROTECTED: u8 = 0x1A;
pub const NS_PRIVATE: u8 = 0x05;

pub const MN_QNAME: u8 = 0x07;
pub const MN_QNAME_A: u8 = 0x0D;
pub const MN_RTQNAME: u8 = 0x0F;
pub const MN_RTQNAME_A: u8 = 0x10;
pub const MN_RTQNAME_L: u8 = 0x11;
pub const MN_RTQNAME_LA: u8 = 0x12;
pub const MN_MULTINAME: u8 = 0x09;
pub const MN_MULTINAME_A: u8 = 0x0E;
pub const MN_MULTINAME_L: u8 = 0x1B;
pub const MN_MULTINAME_LA: u8 = 0x1C;
pub const MN_TYPENAME: u8 = 0x1D;

pub const TRAIT_SLOT: u8 = 0;
pub const TRAIT_METHOD: u8 = 1;
pub const TRAIT_GETTER: u8 = 2;
pub const TRAIT_SETTER: u8 = 3;
pub const TRAIT_CLASS: u8 = 4;
pub const TRAIT_FUNCTION: u8 = 5;
pub const TRAIT_CONST: u8 = 6;

pub const INSTANCE_SEALED: u8 = 0x01;
pub const INSTANCE_FINAL: u8 = 0x02;
pub const INSTANCE_INTERFACE: u8 = 0x04;
pub const INSTANCE_PROTECTED_NS: u8 = 0x08;

pub const METHOD_NEED_ARGUMENTS: u8 = 0x01;
pub const METHOD_NEED_ACTIVATION: u8 = 0x02;
pub const METHOD_NEED_REST: u8 = 0x04;
pub const METHOD_HAS_OPTIONAL: u8 = 0x08;
pub const METHOD_SET_DXNS: u8 = 0x40;
pub const METHOD_HAS_PARAM_NAMES: u8 = 0x80;

pub const CONSTANT_UTF8: u8 = 0x01;
pub const CONSTANT_INT: u8 = 0x03;
pub const CONSTANT_UINT: u8 = 0x04;
pub const CONSTANT_DOUBLE: u8 = 0x06;
pub const CONSTANT_TRUE: u8 = 0x0B;
pub const CONSTANT_FALSE: u8 = 0x0A;
pub const CONSTANT_NULL: u8 = 0x0C;
pub const CONSTANT_UNDEFINED: u8 = 0x00;

pub const TAG_DOABC: u16 = 72;
pub const TAG_DOABC2: u16 = 82;

/// Namespace-kind values that can be a trait/optional default `vkind`.
pub fn is_namespace_vkind(k: u8) -> bool {
    matches!(k, 0x08 | 0x16 | 0x17 | 0x18 | 0x19 | 0x1A | 0x05)
}

// ---------------------------------------------------------------- model

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MethodInfo {
    pub param_count: u32,
    pub return_type: u32,
    pub param_types: Vec<u32>,
    pub name_idx: u32,
    pub flags: u8,
    /// `(vkind, vindex)`
    pub optional_values: Vec<(u8, u32)>,
    pub param_names: Vec<u32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TraitInfo {
    pub name_idx: u32,
    pub kind: u8,
    pub attr: u8,
    pub slot_id: u32,
    pub type_name: u32,
    pub vindex: u32,
    pub vkind: u8,
    pub disp_id: u32,
    pub method_idx: u32,
    pub class_idx: u32,
    pub metadata: Vec<u32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InstanceInfo {
    pub name_idx: u32,
    pub super_idx: u32,
    pub flags: u8,
    pub protected_ns: u32,
    pub interfaces: Vec<u32>,
    pub iinit: u32,
    pub traits: Vec<TraitInfo>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClassInfo {
    pub cinit: u32,
    pub traits: Vec<TraitInfo>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScriptInfo {
    pub sinit: u32,
    pub traits: Vec<TraitInfo>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExceptionInfo {
    pub from_pos: u32,
    pub to_pos: u32,
    pub target: u32,
    pub exc_type: u32,
    pub var_name: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MethodBody {
    pub method_idx: u32,
    pub max_stack: u32,
    pub local_count: u32,
    pub init_scope_depth: u32,
    pub max_scope_depth: u32,
    pub code: Vec<u8>,
    pub exceptions: Vec<ExceptionInfo>,
    pub traits: Vec<TraitInfo>,
}

/// A multiname pool entry. `data` is kind specific, as in the Python tuple:
/// QName/QNameA `[ns, name]`; RTQName(A) `[name]`; RTQNameL(A) `[]`;
/// Multiname(A) `[name, ns_set]`; MultinameL(A) `[ns_set]`;
/// TypeName `[qname, param...]`. Entry 0 is `(0, [])`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Multiname {
    pub kind: u8,
    pub data: Vec<u32>,
}

impl Multiname {
    pub fn new(kind: u8, data: &[u32]) -> Self {
        Multiname { kind, data: data.to_vec() }
    }
}

const MAX_NAME_DEPTH: u32 = 32;

#[derive(Clone, Debug, Default)]
pub struct AbcFile {
    pub minor: u16,
    pub major: u16,
    pub integers: Vec<i32>,
    pub uintegers: Vec<u32>,
    pub doubles: Vec<f64>,
    pub strings: Vec<String>,
    pub namespaces: Vec<(u8, u32)>,
    pub ns_sets: Vec<Vec<u32>>,
    pub multinames: Vec<Multiname>,
    pub methods: Vec<MethodInfo>,
    pub metadata_entries: Vec<(u32, Vec<(u32, u32)>)>,
    pub instances: Vec<InstanceInfo>,
    pub classes: Vec<ClassInfo>,
    pub scripts: Vec<ScriptInfo>,
    pub method_bodies: HashMap<u32, MethodBody>,
    /// Original body order, preserved on serialization.
    pub method_body_order: Vec<u32>,
    /// Pool counts as read, for a byte-faithful round trip (integers, uints, doubles).
    pub raw_pool_counts: [Option<u32>; 3],
}

impl AbcFile {
    /// Parse a raw ABC block. Truncated or malformed input is an `Err`.
    pub fn parse(data: &[u8]) -> Result<AbcFile, String> {
        catch(|| Self::parse_inner(data))?
    }

    fn parse_inner(d: &[u8]) -> Result<AbcFile, String> {
        let mut a = AbcFile::default();
        let mut p = 0;
        (a.minor, p) = ru16(d, p);
        (a.major, p) = ru16(d, p);
        p = a.parse_cpool(d, p)?;
        p = a.parse_methods(d, p);
        p = a.parse_metadata(d, p);
        p = a.parse_instances_and_classes(d, p);
        p = a.parse_scripts(d, p);
        a.parse_method_bodies(d, p);
        Ok(a)
    }

    fn parse_cpool(&mut self, d: &[u8], mut p: usize) -> Result<usize, String> {
        let n;
        (n, p) = ru30(d, p);
        self.raw_pool_counts[0] = Some(n);
        self.integers = vec![0];
        for _ in 0..n.saturating_sub(1) {
            let v;
            (v, p) = ru32(d, p);
            self.integers.push(v as i32);
        }
        let n;
        (n, p) = ru30(d, p);
        self.raw_pool_counts[1] = Some(n);
        self.uintegers = vec![0];
        for _ in 0..n.saturating_sub(1) {
            let v;
            (v, p) = ru32(d, p);
            self.uintegers.push(v);
        }
        let n;
        (n, p) = ru30(d, p);
        self.raw_pool_counts[2] = Some(n);
        self.doubles = vec![0.0];
        for _ in 0..n.saturating_sub(1) {
            let v;
            (v, p) = rd64(d, p);
            self.doubles.push(v);
        }
        let n;
        (n, p) = ru30(d, p);
        self.strings = vec![String::new()];
        for _ in 0..n.saturating_sub(1) {
            let slen;
            (slen, p) = ru30(d, p);
            let end = p.checked_add(slen as usize).ok_or("bad string length")?;
            // Python slices silently at the end of the buffer.
            let s = &d[p.min(d.len())..end.min(d.len())];
            self.strings.push(String::from_utf8_lossy(s).into_owned());
            p = end;
        }
        let n;
        (n, p) = ru30(d, p);
        self.namespaces = vec![(0, 0)];
        for _ in 0..n.saturating_sub(1) {
            let kind;
            (kind, p) = ru8(d, p);
            let name_idx;
            (name_idx, p) = ru30(d, p);
            self.namespaces.push((kind, name_idx));
        }
        let n;
        (n, p) = ru30(d, p);
        self.ns_sets = vec![vec![]];
        for _ in 0..n.saturating_sub(1) {
            let c;
            (c, p) = ru30(d, p);
            let mut set = Vec::new();
            for _ in 0..c {
                let i;
                (i, p) = ru30(d, p);
                set.push(i);
            }
            self.ns_sets.push(set);
        }
        let n;
        (n, p) = ru30(d, p);
        self.multinames = vec![Multiname::default()];
        for _ in 0..n.saturating_sub(1) {
            let kind;
            (kind, p) = ru8(d, p);
            let mn = match kind {
                MN_QNAME | MN_QNAME_A => {
                    let (a, q) = ru30(d, p);
                    let (b, q) = ru30(d, q);
                    p = q;
                    Multiname { kind, data: vec![a, b] }
                }
                MN_RTQNAME | MN_RTQNAME_A | MN_MULTINAME_L | MN_MULTINAME_LA => {
                    let (a, q) = ru30(d, p);
                    p = q;
                    Multiname { kind, data: vec![a] }
                }
                MN_RTQNAME_L | MN_RTQNAME_LA => Multiname { kind, data: vec![] },
                MN_MULTINAME | MN_MULTINAME_A => {
                    let (a, q) = ru30(d, p);
                    let (b, q) = ru30(d, q);
                    p = q;
                    Multiname { kind, data: vec![a, b] }
                }
                MN_TYPENAME => {
                    let (qn, q) = ru30(d, p);
                    let (pc, mut q) = ru30(d, q);
                    let mut data = vec![qn];
                    for _ in 0..pc {
                        let (x, r) = ru30(d, q);
                        q = r;
                        data.push(x);
                    }
                    p = q;
                    Multiname { kind, data }
                }
                _ => return Err(format!("Unknown multiname kind {kind:#x} at offset {p}")),
            };
            self.multinames.push(mn);
        }
        Ok(p)
    }

    fn parse_methods(&mut self, d: &[u8], mut p: usize) -> usize {
        let n;
        (n, p) = ru30(d, p);
        for _ in 0..n {
            let mut m = MethodInfo::default();
            (m.param_count, p) = ru30(d, p);
            (m.return_type, p) = ru30(d, p);
            for _ in 0..m.param_count {
                let t;
                (t, p) = ru30(d, p);
                m.param_types.push(t);
            }
            (m.name_idx, p) = ru30(d, p);
            (m.flags, p) = ru8(d, p);
            if m.flags & METHOD_HAS_OPTIONAL != 0 {
                let c;
                (c, p) = ru30(d, p);
                for _ in 0..c {
                    let val;
                    (val, p) = ru30(d, p);
                    let kind;
                    (kind, p) = ru8(d, p);
                    m.optional_values.push((kind, val));
                }
            }
            if m.flags & METHOD_HAS_PARAM_NAMES != 0 {
                for _ in 0..m.param_count {
                    let nm;
                    (nm, p) = ru30(d, p);
                    m.param_names.push(nm);
                }
            }
            self.methods.push(m);
        }
        p
    }

    fn parse_metadata(&mut self, d: &[u8], mut p: usize) -> usize {
        let n;
        (n, p) = ru30(d, p);
        for _ in 0..n {
            let name_idx;
            (name_idx, p) = ru30(d, p);
            let c;
            (c, p) = ru30(d, p);
            let mut items = Vec::new();
            for _ in 0..c {
                let k;
                (k, p) = ru30(d, p);
                let v;
                (v, p) = ru30(d, p);
                items.push((k, v));
            }
            self.metadata_entries.push((name_idx, items));
        }
        p
    }

    fn parse_trait(d: &[u8], mut p: usize) -> (TraitInfo, usize) {
        let mut t = TraitInfo::default();
        (t.name_idx, p) = ru30(d, p);
        let kb;
        (kb, p) = ru8(d, p);
        t.kind = kb & 0x0F;
        t.attr = (kb >> 4) & 0x0F;
        match t.kind {
            TRAIT_SLOT | TRAIT_CONST => {
                (t.slot_id, p) = ru30(d, p);
                (t.type_name, p) = ru30(d, p);
                (t.vindex, p) = ru30(d, p);
                if t.vindex != 0 {
                    (t.vkind, p) = ru8(d, p);
                }
            }
            TRAIT_METHOD | TRAIT_GETTER | TRAIT_SETTER => {
                (t.disp_id, p) = ru30(d, p);
                (t.method_idx, p) = ru30(d, p);
            }
            TRAIT_CLASS => {
                (t.slot_id, p) = ru30(d, p);
                (t.class_idx, p) = ru30(d, p);
            }
            TRAIT_FUNCTION => {
                (t.slot_id, p) = ru30(d, p);
                (t.method_idx, p) = ru30(d, p);
            }
            _ => {}
        }
        if t.attr & 0x04 != 0 {
            let mc;
            (mc, p) = ru30(d, p);
            for _ in 0..mc {
                let mi;
                (mi, p) = ru30(d, p);
                t.metadata.push(mi);
            }
        }
        (t, p)
    }

    fn parse_traits(d: &[u8], mut p: usize) -> (Vec<TraitInfo>, usize) {
        let n;
        (n, p) = ru30(d, p);
        let mut v = Vec::new();
        for _ in 0..n {
            let t;
            (t, p) = Self::parse_trait(d, p);
            v.push(t);
        }
        (v, p)
    }

    fn parse_instances_and_classes(&mut self, d: &[u8], mut p: usize) -> usize {
        let n;
        (n, p) = ru30(d, p);
        for _ in 0..n {
            let mut inst = InstanceInfo::default();
            (inst.name_idx, p) = ru30(d, p);
            (inst.super_idx, p) = ru30(d, p);
            (inst.flags, p) = ru8(d, p);
            if inst.flags & INSTANCE_PROTECTED_NS != 0 {
                (inst.protected_ns, p) = ru30(d, p);
            }
            let ic;
            (ic, p) = ru30(d, p);
            for _ in 0..ic {
                let ii;
                (ii, p) = ru30(d, p);
                inst.interfaces.push(ii);
            }
            (inst.iinit, p) = ru30(d, p);
            (inst.traits, p) = Self::parse_traits(d, p);
            self.instances.push(inst);
        }
        for _ in 0..n {
            let mut ci = ClassInfo::default();
            (ci.cinit, p) = ru30(d, p);
            (ci.traits, p) = Self::parse_traits(d, p);
            self.classes.push(ci);
        }
        p
    }

    fn parse_scripts(&mut self, d: &[u8], mut p: usize) -> usize {
        let n;
        (n, p) = ru30(d, p);
        for _ in 0..n {
            let mut si = ScriptInfo::default();
            (si.sinit, p) = ru30(d, p);
            (si.traits, p) = Self::parse_traits(d, p);
            self.scripts.push(si);
        }
        p
    }

    fn parse_method_bodies(&mut self, d: &[u8], mut p: usize) -> usize {
        let n;
        (n, p) = ru30(d, p);
        for _ in 0..n {
            let mut mb = MethodBody::default();
            (mb.method_idx, p) = ru30(d, p);
            (mb.max_stack, p) = ru30(d, p);
            (mb.local_count, p) = ru30(d, p);
            (mb.init_scope_depth, p) = ru30(d, p);
            (mb.max_scope_depth, p) = ru30(d, p);
            let cl;
            (cl, p) = ru30(d, p);
            let end = p.saturating_add(cl as usize);
            mb.code = d[p.min(d.len())..end.min(d.len())].to_vec();
            p = end;
            let ec;
            (ec, p) = ru30(d, p);
            for _ in 0..ec {
                let mut ei = ExceptionInfo::default();
                (ei.from_pos, p) = ru30(d, p);
                (ei.to_pos, p) = ru30(d, p);
                (ei.target, p) = ru30(d, p);
                (ei.exc_type, p) = ru30(d, p);
                (ei.var_name, p) = ru30(d, p);
                mb.exceptions.push(ei);
            }
            (mb.traits, p) = Self::parse_traits(d, p);
            self.method_body_order.push(mb.method_idx);
            self.method_bodies.insert(mb.method_idx, mb);
        }
        p
    }

    // ------------------------------------------------------- name helpers

    pub fn string(&self, i: u32) -> &str {
        self.strings.get(i as usize).map(|s| s.as_str()).unwrap_or("")
    }

    pub fn ns_name(&self, ns_idx: u32) -> String {
        if ns_idx == 0 || ns_idx as usize >= self.namespaces.len() {
            return String::new();
        }
        let (_, name_idx) = self.namespaces[ns_idx as usize];
        if name_idx > 0 && (name_idx as usize) < self.strings.len() {
            self.strings[name_idx as usize].clone()
        } else {
            String::new()
        }
    }

    pub fn ns_kind(&self, ns_idx: u32) -> u8 {
        if ns_idx == 0 || ns_idx as usize >= self.namespaces.len() {
            0
        } else {
            self.namespaces[ns_idx as usize].0
        }
    }

    fn mn(&self, idx: u32) -> Option<&Multiname> {
        if idx == 0 || idx as usize >= self.multinames.len() {
            None
        } else {
            Some(&self.multinames[idx as usize])
        }
    }

    pub fn mn_is_attr(&self, idx: u32) -> bool {
        self.mn(idx).map_or(false, |m| matches!(m.kind, MN_QNAME_A | MN_RTQNAME_A | MN_MULTINAME_A))
    }

    fn str_or(&self, i: u32, dflt: &str) -> String {
        match self.strings.get(i as usize) {
            Some(s) => s.clone(),
            None => dflt.to_string(),
        }
    }

    /// Simple unqualified name of a multiname.
    pub fn mn_name(&self, idx: u32) -> String {
        self.mn_name_d(idx, 0)
    }
    fn mn_name_d(&self, idx: u32, depth: u32) -> String {
        let Some(m) = self.mn(idx) else { return "*".into() };
        if depth > MAX_NAME_DEPTH {
            return "*".into();
        }
        match m.kind {
            MN_QNAME | MN_QNAME_A if !m.data.is_empty() => self.str_or(m.data[1], ""),
            MN_MULTINAME | MN_MULTINAME_A | MN_RTQNAME | MN_RTQNAME_A if !m.data.is_empty() => {
                self.str_or(m.data[0], "")
            }
            MN_TYPENAME if !m.data.is_empty() => self.mn_name_d(m.data[0], depth + 1),
            _ => "*".into(),
        }
    }

    /// Fully qualified name like `flash.display.MovieClip`.
    pub fn mn_full(&self, idx: u32) -> String {
        self.mn_full_d(idx, 0)
    }
    fn mn_full_d(&self, idx: u32, depth: u32) -> String {
        let Some(m) = self.mn(idx) else { return "*".into() };
        if depth > MAX_NAME_DEPTH {
            return "*".into();
        }
        match m.kind {
            MN_QNAME | MN_QNAME_A if !m.data.is_empty() => {
                let ns = self.ns_name(m.data[0]);
                let name = self.str_or(m.data[1], "");
                if ns.is_empty() {
                    name
                } else {
                    format!("{ns}.{name}")
                }
            }
            MN_MULTINAME | MN_MULTINAME_A | MN_RTQNAME | MN_RTQNAME_A if !m.data.is_empty() => {
                self.str_or(m.data[0], "?")
            }
            MN_TYPENAME if !m.data.is_empty() => {
                let base = self.mn_full_d(m.data[0], depth + 1);
                let params: Vec<String> = m.data[1..].iter().map(|&q| self.mn_full_d(q, depth + 1)).collect();
                format!("{base}.<{}>", params.join(", "))
            }
            _ => "*".into(),
        }
    }

    /// Namespace part of a multiname.
    pub fn mn_ns(&self, idx: u32) -> String {
        match self.mn(idx) {
            Some(m) if matches!(m.kind, MN_QNAME | MN_QNAME_A) && !m.data.is_empty() => self.ns_name(m.data[0]),
            _ => String::new(),
        }
    }

    pub fn mn_ns_kind(&self, idx: u32) -> u8 {
        match self.mn(idx) {
            Some(m) if matches!(m.kind, MN_QNAME | MN_QNAME_A) && !m.data.is_empty() => self.ns_kind(m.data[0]),
            _ => 0,
        }
    }

    pub fn mn_needs_rt_name(&self, idx: u32) -> bool {
        self.mn(idx).map_or(false, |m| matches!(m.kind, MN_MULTINAME_L | MN_MULTINAME_LA | MN_RTQNAME_L | MN_RTQNAME_LA))
    }

    pub fn mn_needs_rt_ns(&self, idx: u32) -> bool {
        self.mn(idx).map_or(false, |m| matches!(m.kind, MN_RTQNAME | MN_RTQNAME_A | MN_RTQNAME_L | MN_RTQNAME_LA))
    }

    /// Type name for display (`int`, `Vector.<int>`, `*`).
    pub fn type_name(&self, mn_idx: u32) -> String {
        self.type_name_d(mn_idx, 0)
    }
    fn type_name_d(&self, mn_idx: u32, depth: u32) -> String {
        if mn_idx == 0 {
            return "*".into();
        }
        if (mn_idx as usize) < self.multinames.len() && depth <= MAX_NAME_DEPTH {
            let m = &self.multinames[mn_idx as usize];
            if m.kind == MN_TYPENAME && !m.data.is_empty() {
                let base = self.type_name_d(m.data[0], depth + 1);
                let params: Vec<String> = m.data[1..].iter().map(|&p| self.type_name_d(p, depth + 1)).collect();
                return format!("{base}.<{}>", params.join(", "));
            }
        }
        let name = self.mn_name(mn_idx);
        if name.is_empty() {
            "*".into()
        } else {
            name
        }
    }

    /// Format a default parameter / trait value.
    pub fn default_value_str(&self, vkind: u8, vindex: u32) -> String {
        match vkind {
            CONSTANT_INT => self.integers.get(vindex as usize).copied().unwrap_or(0).to_string(),
            CONSTANT_UINT => self.uintegers.get(vindex as usize).copied().unwrap_or(0).to_string(),
            CONSTANT_DOUBLE => {
                let v = self.doubles.get(vindex as usize).copied().unwrap_or(0.0);
                if v.is_nan() {
                    "NaN".into()
                } else if v.is_infinite() {
                    if v > 0.0 { "Infinity".into() } else { "-Infinity".into() }
                } else if v == v.trunc() && v.abs() < 1e15 {
                    format!("{}.0", v as i64)
                } else {
                    fmt_g(v, 15)
                }
            }
            CONSTANT_UTF8 => format!("\"{}\"", self.str_or(vindex, "")),
            CONSTANT_TRUE => "true".into(),
            CONSTANT_FALSE => "false".into(),
            CONSTANT_NULL => "null".into(),
            0x08 | 0x16 | 0x17 | 0x18 | 0x19 | 0x1A | 0x05 => {
                let n = self.ns_name(vindex);
                if n.is_empty() { "null".into() } else { n }
            }
            _ => "undefined".into(),
        }
    }

    /// Serialize back to raw ABC bytes (`serialize_abc`).
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        wu16(&mut buf, self.minor as u32);
        wu16(&mut buf, self.major as u32);
        let pool_count = |i: usize, len: usize| -> u32 {
            match self.raw_pool_counts[i] {
                Some(raw) if len <= std::cmp::max(1, raw as usize) => raw,
                _ => len as u32,
            }
        };
        wu30(&mut buf, pool_count(0, self.integers.len()) as i64);
        for &v in self.integers.iter().skip(1) {
            wu30(&mut buf, (v as u32) as i64);
        }
        wu30(&mut buf, pool_count(1, self.uintegers.len()) as i64);
        for &v in self.uintegers.iter().skip(1) {
            wu30(&mut buf, v as i64);
        }
        wu30(&mut buf, pool_count(2, self.doubles.len()) as i64);
        for &v in self.doubles.iter().skip(1) {
            wd64(&mut buf, v);
        }
        wu30(&mut buf, self.strings.len() as i64);
        for s in self.strings.iter().skip(1) {
            wu30(&mut buf, s.len() as i64);
            buf.extend_from_slice(s.as_bytes());
        }
        wu30(&mut buf, self.namespaces.len() as i64);
        for &(k, n) in self.namespaces.iter().skip(1) {
            wu8(&mut buf, k as i64);
            wu30(&mut buf, n as i64);
        }
        wu30(&mut buf, self.ns_sets.len() as i64);
        for set in self.ns_sets.iter().skip(1) {
            wu30(&mut buf, set.len() as i64);
            for &n in set {
                wu30(&mut buf, n as i64);
            }
        }
        wu30(&mut buf, self.multinames.len() as i64);
        for m in self.multinames.iter().skip(1) {
            wu8(&mut buf, m.kind as i64);
            match m.kind {
                MN_QNAME | MN_QNAME_A | MN_MULTINAME | MN_MULTINAME_A => {
                    wu30(&mut buf, *m.data.first().unwrap_or(&0) as i64);
                    wu30(&mut buf, *m.data.get(1).unwrap_or(&0) as i64);
                }
                MN_RTQNAME | MN_RTQNAME_A | MN_MULTINAME_L | MN_MULTINAME_LA => {
                    wu30(&mut buf, *m.data.first().unwrap_or(&0) as i64);
                }
                MN_TYPENAME => {
                    wu30(&mut buf, *m.data.first().unwrap_or(&0) as i64);
                    wu30(&mut buf, m.data.len().saturating_sub(1) as i64);
                    for &x in m.data.iter().skip(1) {
                        wu30(&mut buf, x as i64);
                    }
                }
                _ => {}
            }
        }
        wu30(&mut buf, self.methods.len() as i64);
        for m in &self.methods {
            wu30(&mut buf, m.param_count as i64);
            wu30(&mut buf, m.return_type as i64);
            for &pt in &m.param_types {
                wu30(&mut buf, pt as i64);
            }
            wu30(&mut buf, m.name_idx as i64);
            wu8(&mut buf, m.flags as i64);
            if m.flags & METHOD_HAS_OPTIONAL != 0 {
                wu30(&mut buf, m.optional_values.len() as i64);
                for &(vk, val) in &m.optional_values {
                    wu30(&mut buf, val as i64);
                    wu8(&mut buf, vk as i64);
                }
            }
            if m.flags & METHOD_HAS_PARAM_NAMES != 0 {
                for &nm in &m.param_names {
                    wu30(&mut buf, nm as i64);
                }
            }
        }
        wu30(&mut buf, self.metadata_entries.len() as i64);
        for (name_idx, items) in &self.metadata_entries {
            wu30(&mut buf, *name_idx as i64);
            wu30(&mut buf, items.len() as i64);
            for &(k, v) in items {
                wu30(&mut buf, k as i64);
                wu30(&mut buf, v as i64);
            }
        }
        wu30(&mut buf, self.instances.len() as i64);
        for inst in &self.instances {
            wu30(&mut buf, inst.name_idx as i64);
            wu30(&mut buf, inst.super_idx as i64);
            wu8(&mut buf, inst.flags as i64);
            if inst.flags & INSTANCE_PROTECTED_NS != 0 {
                wu30(&mut buf, inst.protected_ns as i64);
            }
            wu30(&mut buf, inst.interfaces.len() as i64);
            for &i in &inst.interfaces {
                wu30(&mut buf, i as i64);
            }
            wu30(&mut buf, inst.iinit as i64);
            wu30(&mut buf, inst.traits.len() as i64);
            for t in &inst.traits {
                serialize_trait(&mut buf, t);
            }
        }
        for ci in &self.classes {
            wu30(&mut buf, ci.cinit as i64);
            wu30(&mut buf, ci.traits.len() as i64);
            for t in &ci.traits {
                serialize_trait(&mut buf, t);
            }
        }
        wu30(&mut buf, self.scripts.len() as i64);
        for si in &self.scripts {
            wu30(&mut buf, si.sinit as i64);
            wu30(&mut buf, si.traits.len() as i64);
            for t in &si.traits {
                serialize_trait(&mut buf, t);
            }
        }
        let mut bodies: Vec<&MethodBody> = Vec::new();
        if !self.method_body_order.is_empty() {
            for idx in &self.method_body_order {
                if let Some(b) = self.method_bodies.get(idx) {
                    bodies.push(b);
                }
            }
            let known: std::collections::HashSet<u32> = self.method_body_order.iter().copied().collect();
            let mut extra: Vec<&MethodBody> =
                self.method_bodies.iter().filter(|(k, _)| !known.contains(k)).map(|(_, v)| v).collect();
            extra.sort_by_key(|b| b.method_idx);
            bodies.extend(extra);
        } else {
            bodies = self.method_bodies.values().collect();
            bodies.sort_by_key(|b| b.method_idx);
        }
        wu30(&mut buf, bodies.len() as i64);
        for mb in bodies {
            wu30(&mut buf, mb.method_idx as i64);
            wu30(&mut buf, mb.max_stack as i64);
            wu30(&mut buf, mb.local_count as i64);
            wu30(&mut buf, mb.init_scope_depth as i64);
            wu30(&mut buf, mb.max_scope_depth as i64);
            wu30(&mut buf, mb.code.len() as i64);
            buf.extend_from_slice(&mb.code);
            wu30(&mut buf, mb.exceptions.len() as i64);
            for ei in &mb.exceptions {
                wu30(&mut buf, ei.from_pos as i64);
                wu30(&mut buf, ei.to_pos as i64);
                wu30(&mut buf, ei.target as i64);
                wu30(&mut buf, ei.exc_type as i64);
                wu30(&mut buf, ei.var_name as i64);
            }
            wu30(&mut buf, mb.traits.len() as i64);
            for t in &mb.traits {
                serialize_trait(&mut buf, t);
            }
        }
        buf
    }
}

fn serialize_trait(buf: &mut Vec<u8>, t: &TraitInfo) {
    wu30(buf, t.name_idx as i64);
    wu8(buf, ((t.kind & 0x0F) | ((t.attr & 0x0F) << 4)) as i64);
    match t.kind {
        TRAIT_SLOT | TRAIT_CONST => {
            wu30(buf, t.slot_id as i64);
            wu30(buf, t.type_name as i64);
            wu30(buf, t.vindex as i64);
            if t.vindex != 0 {
                wu8(buf, t.vkind as i64);
            }
        }
        TRAIT_METHOD | TRAIT_GETTER | TRAIT_SETTER => {
            wu30(buf, t.disp_id as i64);
            wu30(buf, t.method_idx as i64);
        }
        TRAIT_CLASS => {
            wu30(buf, t.slot_id as i64);
            wu30(buf, t.class_idx as i64);
        }
        TRAIT_FUNCTION => {
            wu30(buf, t.slot_id as i64);
            wu30(buf, t.method_idx as i64);
        }
        _ => {}
    }
    if t.attr & 0x04 != 0 {
        wu30(buf, t.metadata.len() as i64);
        for &m in &t.metadata {
            wu30(buf, m as i64);
        }
    }
}

/// `%.{prec}g` as Python/C print it.
pub fn fmt_g(v: f64, prec: usize) -> String {
    if v.is_nan() {
        return "nan".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if v == 0.0 {
        return if v.is_sign_negative() { "-0".into() } else { "0".into() };
    }
    let sci = format!("{:.*e}", prec - 1, v);
    let (mant, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let trim = |s: &str| -> String {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s.to_string()
        }
    };
    if exp < -4 || exp >= prec as i32 {
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{}e{}{:02}", trim(mant), sign, exp.abs())
    } else {
        let dec = (prec as i32 - 1 - exp).max(0) as usize;
        trim(&format!("{:.*}", dec, v))
    }
}

// ---------------------------------------------------------------- opcodes

macro_rules! ops {
    ($($name:ident = $v:expr),* $(,)?) => { $(pub const $name: u8 = $v;)* };
}
ops! {
    OP_BKPT=0x01, OP_NOP=0x02, OP_THROW=0x03, OP_GETSUPER=0x04, OP_SETSUPER=0x05, OP_DXNS=0x06,
    OP_DXNSLATE=0x07, OP_KILL=0x08, OP_LABEL=0x09, OP_IFNLT=0x0C, OP_IFNLE=0x0D, OP_IFNGT=0x0E,
    OP_IFNGE=0x0F, OP_JUMP=0x10, OP_IFTRUE=0x11, OP_IFFALSE=0x12, OP_IFEQ=0x13, OP_IFNE=0x14,
    OP_IFLT=0x15, OP_IFLE=0x16, OP_IFGT=0x17, OP_IFGE=0x18, OP_IFSTRICTEQ=0x19, OP_IFSTRICTNE=0x1A,
    OP_LOOKUPSWITCH=0x1B, OP_PUSHWITH=0x1C, OP_POPSCOPE=0x1D, OP_NEXTNAME=0x1E, OP_HASNEXT=0x1F,
    OP_PUSHNULL=0x20, OP_PUSHUNDEFINED=0x21, OP_NEXTVALUE=0x23, OP_PUSHBYTE=0x24, OP_PUSHSHORT=0x25,
    OP_PUSHTRUE=0x26, OP_PUSHFALSE=0x27, OP_PUSHNAN=0x28, OP_POP=0x29, OP_DUP=0x2A, OP_SWAP=0x2B,
    OP_PUSHSTRING=0x2C, OP_PUSHINT=0x2D, OP_PUSHUINT=0x2E, OP_PUSHDOUBLE=0x2F, OP_PUSHSCOPE=0x30,
    OP_PUSHNAMESPACE=0x31, OP_HASNEXT2=0x32, OP_LI8=0x35, OP_LI16=0x36, OP_LI32=0x37, OP_LF32=0x38,
    OP_LF64=0x39, OP_SI8=0x3A, OP_SI16=0x3B, OP_SI32=0x3C, OP_SF32=0x3D, OP_SF64=0x3E,
    OP_NEWFUNCTION=0x40, OP_CALL=0x41, OP_CONSTRUCT=0x42, OP_CALLMETHOD=0x43, OP_CALLSTATIC=0x44,
    OP_CALLSUPER=0x45, OP_CALLPROPERTY=0x46, OP_RETURNVOID=0x47, OP_RETURNVALUE=0x48,
    OP_CONSTRUCTSUPER=0x49, OP_CONSTRUCTPROP=0x4A, OP_CALLPROPLEX=0x4C, OP_CALLSUPERVOID=0x4E,
    OP_CALLPROPVOID=0x4F, OP_SXI1=0x50, OP_SXI8=0x51, OP_SXI16=0x52, OP_APPLYTYPE=0x53,
    OP_NEWOBJECT=0x55, OP_NEWARRAY=0x56, OP_NEWACTIVATION=0x57, OP_NEWCLASS=0x58,
    OP_GETDESCENDANTS=0x59, OP_NEWCATCH=0x5A, OP_FINDPROPSTRICT=0x5D, OP_FINDPROPERTY=0x5E,
    OP_FINDDEF=0x5F, OP_GETLEX=0x60, OP_SETPROPERTY=0x61, OP_GETLOCAL=0x62, OP_SETLOCAL=0x63,
    OP_GETGLOBALSCOPE=0x64, OP_GETSCOPEOBJECT=0x65, OP_GETPROPERTY=0x66, OP_INITPROPERTY=0x68,
    OP_DELETEPROPERTY=0x6A, OP_GETSLOT=0x6C, OP_SETSLOT=0x6D, OP_GETGLOBALSLOT=0x6E,
    OP_SETGLOBALSLOT=0x6F, OP_CONVERT_S=0x70, OP_ESC_XELEM=0x71, OP_ESC_XATTR=0x72,
    OP_CONVERT_I=0x73, OP_CONVERT_U=0x74, OP_CONVERT_D=0x75, OP_CONVERT_B=0x76, OP_CONVERT_O=0x77,
    OP_CHECKFILTER=0x78, OP_COERCE=0x80, OP_COERCE_B=0x81, OP_COERCE_A=0x82, OP_COERCE_I=0x83,
    OP_COERCE_D=0x84, OP_COERCE_S=0x85, OP_ASTYPE=0x86, OP_ASTYPELATE=0x87, OP_COERCE_U=0x88,
    OP_COERCE_O=0x89, OP_NEGATE=0x90, OP_INCREMENT=0x91, OP_INCLOCAL=0x92, OP_DECREMENT=0x93,
    OP_DECLOCAL=0x94, OP_TYPEOF=0x95, OP_NOT=0x96, OP_BITNOT=0x97, OP_ADD=0xA0, OP_SUBTRACT=0xA1,
    OP_MULTIPLY=0xA2, OP_DIVIDE=0xA3, OP_MODULO=0xA4, OP_LSHIFT=0xA5, OP_RSHIFT=0xA6,
    OP_URSHIFT=0xA7, OP_BITAND=0xA8, OP_BITOR=0xA9, OP_BITXOR=0xAA, OP_EQUALS=0xAB,
    OP_STRICTEQUALS=0xAC, OP_LESSTHAN=0xAD, OP_LESSEQUALS=0xAE, OP_GREATERTHAN=0xAF,
    OP_GREATEREQUALS=0xB0, OP_INSTANCEOF=0xB1, OP_ISTYPE=0xB2, OP_ISTYPELATE=0xB3, OP_IN=0xB4,
    OP_INCREMENT_I=0xC0, OP_DECREMENT_I=0xC1, OP_INCLOCAL_I=0xC2, OP_DECLOCAL_I=0xC3,
    OP_NEGATE_I=0xC4, OP_ADD_I=0xC5, OP_SUBTRACT_I=0xC6, OP_MULTIPLY_I=0xC7, OP_GETLOCAL_0=0xD0,
    OP_GETLOCAL_1=0xD1, OP_GETLOCAL_2=0xD2, OP_GETLOCAL_3=0xD3, OP_SETLOCAL_0=0xD4,
    OP_SETLOCAL_1=0xD5, OP_SETLOCAL_2=0xD6, OP_SETLOCAL_3=0xD7, OP_DEBUG=0xEF, OP_DEBUGLINE=0xF0,
    OP_DEBUGFILE=0xF1,
}

/// Opcode names, indexed by the table order used by the assembler/disassembler.
pub const OPCODE_NAMES: &[(&str, u8)] = &[
    ("bkpt", 0x01), ("nop", 0x02), ("throw", 0x03), ("getsuper", 0x04), ("setsuper", 0x05),
    ("dxns", 0x06), ("dxnslate", 0x07), ("kill", 0x08), ("label", 0x09), ("ifnlt", 0x0C),
    ("ifnle", 0x0D), ("ifngt", 0x0E), ("ifnge", 0x0F), ("jump", 0x10), ("iftrue", 0x11),
    ("iffalse", 0x12), ("ifeq", 0x13), ("ifne", 0x14), ("iflt", 0x15), ("ifle", 0x16),
    ("ifgt", 0x17), ("ifge", 0x18), ("ifstricteq", 0x19), ("ifstrictne", 0x1A),
    ("lookupswitch", 0x1B), ("pushwith", 0x1C), ("popscope", 0x1D), ("nextname", 0x1E),
    ("hasnext", 0x1F), ("pushnull", 0x20), ("pushundefined", 0x21), ("nextvalue", 0x23),
    ("pushbyte", 0x24), ("pushshort", 0x25), ("pushtrue", 0x26), ("pushfalse", 0x27),
    ("pushnan", 0x28), ("pop", 0x29), ("dup", 0x2A), ("swap", 0x2B), ("pushstring", 0x2C),
    ("pushint", 0x2D), ("pushuint", 0x2E), ("pushdouble", 0x2F), ("pushscope", 0x30),
    ("pushnamespace", 0x31), ("hasnext2", 0x32), ("li8", 0x35), ("li16", 0x36), ("li32", 0x37),
    ("lf32", 0x38), ("lf64", 0x39), ("si8", 0x3A), ("si16", 0x3B), ("si32", 0x3C), ("sf32", 0x3D),
    ("sf64", 0x3E), ("newfunction", 0x40), ("call", 0x41), ("construct", 0x42),
    ("callmethod", 0x43), ("callstatic", 0x44), ("callsuper", 0x45), ("callproperty", 0x46),
    ("returnvoid", 0x47), ("returnvalue", 0x48), ("constructsuper", 0x49), ("constructprop", 0x4A),
    ("callproplex", 0x4C), ("callsupervoid", 0x4E), ("callpropvoid", 0x4F), ("sxi1", 0x50),
    ("sxi8", 0x51), ("sxi16", 0x52), ("applytype", 0x53), ("newobject", 0x55), ("newarray", 0x56),
    ("newactivation", 0x57), ("newclass", 0x58), ("getdescendants", 0x59), ("newcatch", 0x5A),
    ("findpropstrict", 0x5D), ("findproperty", 0x5E), ("finddef", 0x5F), ("getlex", 0x60),
    ("setproperty", 0x61), ("getlocal", 0x62), ("setlocal", 0x63), ("getglobalscope", 0x64),
    ("getscopeobject", 0x65), ("getproperty", 0x66), ("initproperty", 0x68),
    ("deleteproperty", 0x6A), ("getslot", 0x6C), ("setslot", 0x6D), ("getglobalslot", 0x6E),
    ("setglobalslot", 0x6F), ("convert_s", 0x70), ("esc_xelem", 0x71), ("esc_xattr", 0x72),
    ("convert_i", 0x73), ("convert_u", 0x74), ("convert_d", 0x75), ("convert_b", 0x76),
    ("convert_o", 0x77), ("checkfilter", 0x78), ("coerce", 0x80), ("coerce_b", 0x81),
    ("coerce_a", 0x82), ("coerce_i", 0x83), ("coerce_d", 0x84), ("coerce_s", 0x85),
    ("astype", 0x86), ("astypelate", 0x87), ("coerce_u", 0x88), ("coerce_o", 0x89),
    ("negate", 0x90), ("increment", 0x91), ("inclocal", 0x92), ("decrement", 0x93),
    ("declocal", 0x94), ("typeof", 0x95), ("not", 0x96), ("bitnot", 0x97), ("add", 0xA0),
    ("subtract", 0xA1), ("multiply", 0xA2), ("divide", 0xA3), ("modulo", 0xA4), ("lshift", 0xA5),
    ("rshift", 0xA6), ("urshift", 0xA7), ("bitand", 0xA8), ("bitor", 0xA9), ("bitxor", 0xAA),
    ("equals", 0xAB), ("strictequals", 0xAC), ("lessthan", 0xAD), ("lessequals", 0xAE),
    ("greaterthan", 0xAF), ("greaterequals", 0xB0), ("instanceof", 0xB1), ("istype", 0xB2),
    ("istypelate", 0xB3), ("in", 0xB4), ("increment_i", 0xC0), ("decrement_i", 0xC1),
    ("inclocal_i", 0xC2), ("declocal_i", 0xC3), ("negate_i", 0xC4), ("add_i", 0xC5),
    ("subtract_i", 0xC6), ("multiply_i", 0xC7), ("getlocal_0", 0xD0), ("getlocal_1", 0xD1),
    ("getlocal_2", 0xD2), ("getlocal_3", 0xD3), ("setlocal_0", 0xD4), ("setlocal_1", 0xD5),
    ("setlocal_2", 0xD6), ("setlocal_3", 0xD7), ("debug", 0xEF), ("debugline", 0xF0),
    ("debugfile", 0xF1),
];

pub fn opcode_by_name(name: &str) -> Option<u8> {
    let n = name.to_lowercase();
    OPCODE_NAMES.iter().find(|(s, _)| *s == n).map(|(_, v)| *v)
}
pub fn opcode_name(op: u8) -> Option<&'static str> {
    OPCODE_NAMES.iter().find(|(_, v)| *v == op).map(|(s, _)| *s)
}

/// Operand descriptors (`_OP_FORMATS` plus the expanded forms that
/// `_parse_instructions` produces for `lookupswitch` and `debug`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum D {
    Multiname,
    Str,
    Int,
    UInt,
    Double,
    Namespace,
    Method,
    Class,
    U30,
    U8,
    S24,
    SwitchDefault,
    SwitchCount,
    SwitchCase,
    DebugType,
    DebugString,
    DebugReg,
    DebugExtra,
}

impl D {
    pub fn is_u30(self) -> bool {
        matches!(
            self,
            D::Multiname | D::Str | D::Int | D::UInt | D::Double | D::Namespace | D::Method | D::Class | D::U30
                | D::SwitchCount | D::DebugString | D::DebugExtra
        )
    }
    pub fn is_u8(self) -> bool {
        matches!(self, D::U8 | D::DebugType | D::DebugReg)
    }
    pub fn is_s24(self) -> bool {
        matches!(self, D::S24 | D::SwitchDefault | D::SwitchCase)
    }
    /// Operand references a scalar constant pool.
    pub fn is_const_pool(self) -> bool {
        matches!(self, D::Str | D::DebugString | D::Int | D::UInt | D::Double)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum F {
    Mn,
    Str,
    Int,
    UInt,
    Dbl,
    Ns,
    Meth,
    Cls,
    U30,
    U8,
    S24,
    Switch,
    Debug,
}

fn op_format(op: u8) -> Option<&'static [F]> {
    use F::*;
    Some(match op {
        0x01 | 0x02 | 0x03 | 0x07 | 0x09 | 0x1C | 0x1D | 0x1E | 0x1F | 0x20 | 0x21 | 0x23 | 0x26 | 0x27
        | 0x28 | 0x29 | 0x2A | 0x2B | 0x30 | 0x35..=0x3E | 0x47 | 0x48 | 0x50..=0x52 | 0x57 | 0x64
        | 0x70..=0x78 | 0x81..=0x85 | 0x87..=0x89 | 0x90 | 0x91 | 0x93 | 0x95..=0x97 | 0xA0..=0xB1
        | 0xB3 | 0xB4 | 0xC0 | 0xC1 | 0xC4..=0xC7 | 0xD0..=0xD7 => &[],
        0x04 | 0x05 | 0x59 | 0x5D | 0x5E | 0x5F | 0x60 | 0x61 | 0x66 | 0x68 | 0x6A | 0x80 | 0x86 | 0xB2 => &[Mn],
        0x06 | 0x2C | 0xF1 => &[Str],
        0x08 | 0x25 | 0x41 | 0x42 | 0x49 | 0x53 | 0x55 | 0x56 | 0x5A | 0x62 | 0x63 | 0x6C | 0x6D | 0x6E
        | 0x6F | 0x92 | 0x94 | 0xC2 | 0xC3 | 0xF0 => &[U30],
        0x0C..=0x1A if op != 0x1B => &[S24],
        0x1B => &[Switch],
        0x24 | 0x65 => &[U8],
        0x2D => &[Int],
        0x2E => &[UInt],
        0x2F => &[Dbl],
        0x31 => &[Ns],
        0x32 | 0x43 => &[U30, U30],
        0x40 => &[Meth],
        0x44 => &[Meth, U30],
        0x45 | 0x46 | 0x4A | 0x4C | 0x4E | 0x4F => &[Mn, U30],
        0x58 => &[Cls],
        0xEF => &[Debug],
        _ => return None,
    })
}

/// One decoded instruction: `(start, end, opcode, operands)`.
#[derive(Clone, Debug, PartialEq)]
pub struct Instr {
    pub start: usize,
    pub end: usize,
    pub op: u8,
    pub operands: Vec<(D, i64)>,
}

/// `_parse_instructions`: decode AVM2 bytecode into instruction records.
pub fn parse_instructions(code: &[u8]) -> Vec<Instr> {
    let mut out = Vec::new();
    let mut p = 0;
    while p < code.len() {
        let start = p;
        let op = code[p];
        p += 1;
        let mut operands: Vec<(D, i64)> = Vec::new();
        let Some(fmt) = op_format(op) else {
            out.push(Instr { start, end: p, op, operands });
            continue;
        };
        for f in fmt {
            match f {
                F::Mn | F::Str | F::Int | F::UInt | F::Dbl | F::Ns | F::Meth | F::Cls | F::U30 => {
                    let (v, q) = ru30(code, p);
                    p = q;
                    let d = match f {
                        F::Mn => D::Multiname,
                        F::Str => D::Str,
                        F::Int => D::Int,
                        F::UInt => D::UInt,
                        F::Dbl => D::Double,
                        F::Ns => D::Namespace,
                        F::Meth => D::Method,
                        F::Cls => D::Class,
                        _ => D::U30,
                    };
                    operands.push((d, v as i64));
                }
                F::U8 => {
                    let (v, q) = ru8(code, p);
                    p = q;
                    operands.push((D::U8, v as i64));
                }
                F::S24 => {
                    let (v, q) = rs24(code, p);
                    p = q;
                    operands.push((D::S24, v as i64));
                }
                F::Switch => {
                    let (def, q) = rs24(code, p);
                    operands.push((D::SwitchDefault, def as i64));
                    let (cc, mut q) = ru30(code, q);
                    operands.push((D::SwitchCount, cc as i64));
                    for _ in 0..=cc {
                        let (co, r) = rs24(code, q);
                        q = r;
                        operands.push((D::SwitchCase, co as i64));
                    }
                    p = q;
                }
                F::Debug => {
                    let (dt, q) = ru8(code, p);
                    operands.push((D::DebugType, dt as i64));
                    let (idx, q) = ru30(code, q);
                    operands.push((D::DebugString, idx as i64));
                    let (reg, q) = ru8(code, q);
                    operands.push((D::DebugReg, reg as i64));
                    let (ex, q) = ru30(code, q);
                    operands.push((D::DebugExtra, ex as i64));
                    p = q;
                }
            }
        }
        out.push(Instr { start, end: p, op, operands });
    }
    out
}

pub fn u30_byte_size(val: i64) -> usize {
    let v = (val & 0xFFFF_FFFF) as u64;
    if v < 0x80 {
        1
    } else if v < 0x4000 {
        2
    } else if v < 0x20_0000 {
        3
    } else if v < 0x1000_0000 {
        4
    } else {
        5
    }
}

pub fn operand_byte_size(d: D, val: i64) -> usize {
    if d.is_u30() {
        u30_byte_size(val)
    } else if d.is_u8() {
        1
    } else if d.is_s24() {
        3
    } else {
        0
    }
}

/// Encode one operand.
pub fn write_operand(buf: &mut Vec<u8>, d: D, val: i64) {
    if d.is_u30() {
        wu30(buf, val);
    } else if d.is_u8() {
        wu8(buf, val);
    } else if d.is_s24() {
        ws24(buf, val);
    }
}

// ---------------------------------------------------------------- disassembler

fn fmt_signed(v: i64) -> String {
    if v >= 0 {
        format!("+{v}")
    } else {
        v.to_string()
    }
}

/// `disassemble`: human readable listing with resolved pool references.
pub fn disassemble(abc: &AbcFile, code: &[u8]) -> String {
    let instrs = parse_instructions(code);
    let mut targets = std::collections::HashSet::new();
    for i in &instrs {
        for &(d, v) in &i.operands {
            if d == D::S24 {
                targets.insert(i.end as i64 + v);
            } else if matches!(d, D::SwitchDefault | D::SwitchCase) {
                targets.insert(i.start as i64 + v);
            }
        }
    }
    let mut lines = Vec::new();
    for i in &instrs {
        let name = opcode_name(i.op).map(|s| s.to_string()).unwrap_or_else(|| format!("0x{:02X}", i.op));
        let prefix = if targets.contains(&(i.start as i64)) { format!("L{}:\n", i.start) } else { String::new() };
        let mut parts = Vec::new();
        for &(d, v) in &i.operands {
            let uv = v as usize;
            match d {
                D::Str | D::DebugString => {
                    let s = if v > 0 && uv < abc.strings.len() { abc.strings[uv].clone() } else { "?".into() };
                    parts.push(format!("\"{s}\"  ; str[{v}]"));
                }
                D::Multiname => {
                    let m = if v > 0 && uv < abc.multinames.len() { abc.mn_full(v as u32) } else { "?".into() };
                    parts.push(format!("{m}  ; mn[{v}]"));
                }
                D::Namespace => {
                    let n = if v > 0 && uv < abc.namespaces.len() { abc.ns_name(v as u32) } else { "?".into() };
                    parts.push(format!("{n}  ; ns[{v}]"));
                }
                D::Int => {
                    let x = if v > 0 && uv < abc.integers.len() { abc.integers[uv].to_string() } else { "?".into() };
                    parts.push(format!("{x}  ; int[{v}]"));
                }
                D::UInt => {
                    let x = if v > 0 && uv < abc.uintegers.len() { abc.uintegers[uv].to_string() } else { "?".into() };
                    parts.push(format!("{x}  ; uint[{v}]"));
                }
                D::Double => {
                    let x = if v > 0 && uv < abc.doubles.len() { py_float_repr(abc.doubles[uv]) } else { "?".into() };
                    parts.push(format!("{x}  ; dbl[{v}]"));
                }
                D::Method => parts.push(format!("method[{v}]")),
                D::Class => parts.push(format!("class[{v}]")),
                D::S24 => parts.push(format!("L{}  ; offset={}", i.end as i64 + v, fmt_signed(v))),
                D::SwitchDefault | D::SwitchCase => {
                    parts.push(format!("L{}  ; offset={}", i.start as i64 + v, fmt_signed(v)))
                }
                D::SwitchCount => parts.push(format!("case_count={v}")),
                _ => parts.push(v.to_string()),
            }
        }
        let op_str = parts.join(", ");
        lines.push(format!("{prefix}{:04}  {:<17}{op_str}", i.start, name));
    }
    lines.join("\n")
}

/// Python's `repr(float)` (shortest round-trip, with `.0` / exponent rules).
pub fn py_float_repr(v: f64) -> String {
    if v.is_nan() {
        return "nan".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if v == 0.0 {
        return if v.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    // Shortest digits via Rust's `{:e}` (round-trip), then lay out like Python.
    let sci = format!("{:e}", v);
    let (mant, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let neg = mant.starts_with('-');
    let digits: String = mant.chars().filter(|c| c.is_ascii_digit()).collect();
    let sign = if neg { "-" } else { "" };
    if (-4..16).contains(&exp) {
        if exp >= 0 {
            let e = exp as usize;
            if digits.len() <= e + 1 {
                format!("{sign}{}{}.0", digits, "0".repeat(e + 1 - digits.len()))
            } else {
                format!("{sign}{}.{}", &digits[..e + 1], &digits[e + 1..])
            }
        } else {
            format!("{sign}0.{}{}", "0".repeat((-exp - 1) as usize), digits)
        }
    } else {
        let m = if digits.len() > 1 { format!("{}.{}", &digits[..1], &digits[1..]) } else { digits.clone() };
        let es = if exp < 0 { '-' } else { '+' };
        format!("{sign}{m}e{es}{:02}", exp.abs())
    }
}

// ---------------------------------------------------------------- assembler

/// An operand handed to [`Assembler::emit`]: a pool/register value or a label.
#[derive(Clone, Debug)]
pub enum Arg {
    Val(i64),
    Label(String),
}

impl From<i64> for Arg {
    fn from(v: i64) -> Self {
        Arg::Val(v)
    }
}
impl From<&str> for Arg {
    fn from(s: &str) -> Self {
        Arg::Label(s.to_string())
    }
}

#[derive(Clone, Debug)]
enum Item {
    Label(String),
    Instr(u8, Vec<(D, Arg)>),
}

/// AVM2 bytecode assembler with label-based branching.
#[derive(Default)]
pub struct Assembler {
    items: Vec<Item>,
    labels: HashMap<String, usize>,
}

impl Assembler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn label(&mut self, name: &str) -> Result<&mut Self, String> {
        if self.labels.contains_key(name) {
            return Err(format!("Duplicate label: '{name}'"));
        }
        self.labels.insert(name.to_string(), self.items.len());
        self.items.push(Item::Label(name.to_string()));
        Ok(self)
    }

    /// Emit one instruction by name. Branch opcodes take a label; for
    /// `lookupswitch` use [`Assembler::emit_switch`], for `debug` pass four values.
    pub fn emit(&mut self, name: &str, operands: &[Arg]) -> Result<&mut Self, String> {
        let op = opcode_by_name(name).ok_or_else(|| format!("Unknown opcode: '{name}'"))?;
        if op == 0x1B {
            return Err("lookupswitch requires emit_switch(default_label, case_labels)".into());
        }
        let mut built = Vec::new();
        if op == 0xEF {
            if operands.len() != 4 {
                return Err("debug requires (debug_type, string_idx, register, extra)".into());
            }
            for (d, a) in [D::DebugType, D::DebugString, D::DebugReg, D::DebugExtra].into_iter().zip(operands) {
                built.push((d, a.clone()));
            }
        } else {
            let fmt = op_format(op).unwrap_or(&[]);
            if fmt.len() != operands.len() {
                return Err(format!("'{name}' expects {} operand(s), got {}", fmt.len(), operands.len()));
            }
            for (f, a) in fmt.iter().zip(operands) {
                let d = match f {
                    F::Mn => D::Multiname,
                    F::Str => D::Str,
                    F::Int => D::Int,
                    F::UInt => D::UInt,
                    F::Dbl => D::Double,
                    F::Ns => D::Namespace,
                    F::Meth => D::Method,
                    F::Cls => D::Class,
                    F::U30 => D::U30,
                    F::U8 => D::U8,
                    F::S24 => D::S24,
                    _ => D::U30,
                };
                built.push((d, a.clone()));
            }
        }
        self.items.push(Item::Instr(op, built));
        Ok(self)
    }

    pub fn emit_switch(&mut self, default_label: &str, cases: &[&str]) -> Result<&mut Self, String> {
        if cases.is_empty() {
            return Err("lookupswitch needs at least one case".into());
        }
        let mut built = vec![
            (D::SwitchDefault, Arg::Label(default_label.to_string())),
            (D::SwitchCount, Arg::Val(cases.len() as i64 - 1)),
        ];
        for c in cases {
            built.push((D::SwitchCase, Arg::Label((*c).to_string())));
        }
        self.items.push(Item::Instr(0x1B, built));
        Ok(self)
    }

    /// Copy of an instruction from [`parse_instructions`] (numeric branch offsets kept).
    pub fn emit_raw(&mut self, op: u8, operands: &[(D, i64)]) -> &mut Self {
        self.items.push(Item::Instr(op, operands.iter().map(|&(d, v)| (d, Arg::Val(v))).collect()));
        self
    }

    pub fn assemble(&self) -> Result<Vec<u8>, String> {
        let size_of = |ops: &[(D, Arg)]| -> usize {
            1 + ops
                .iter()
                .map(|(d, a)| match a {
                    Arg::Label(_) => 3,
                    Arg::Val(v) => operand_byte_size(*d, *v),
                })
                .sum::<usize>()
        };
        let mut positions = Vec::new();
        let mut pos = 0;
        for it in &self.items {
            positions.push(pos);
            if let Item::Instr(_, ops) = it {
                pos += size_of(ops);
            }
        }
        let mut label_pos = HashMap::new();
        for (name, idx) in &self.labels {
            label_pos.insert(name.clone(), positions[*idx]);
        }
        let mut buf = Vec::new();
        for (idx, it) in self.items.iter().enumerate() {
            let Item::Instr(op, ops) = it else { continue };
            let start = positions[idx];
            let end = start + size_of(ops);
            buf.push(*op);
            for (d, a) in ops {
                match a {
                    Arg::Label(l) => {
                        let target = *label_pos.get(l).ok_or_else(|| format!("Undefined label: '{l}'"))?;
                        let off = match d {
                            D::S24 => target as i64 - end as i64,
                            D::SwitchDefault | D::SwitchCase => target as i64 - start as i64,
                            _ => return Err(format!("Label '{l}' used in non-branch operand")),
                        };
                        ws24(&mut buf, off);
                    }
                    Arg::Val(v) => write_operand(&mut buf, *d, *v),
                }
            }
        }
        Ok(buf)
    }
}

// ---------------------------------------------------------------- editor

/// High-level editor for an ABC block: pool deduplication, method creation,
/// trait building and class mutation (`ABCEditor`).
pub struct AbcEditor {
    pub abc: AbcFile,
}

impl AbcEditor {
    pub fn new(data: &[u8]) -> Result<Self, String> {
        Ok(AbcEditor { abc: AbcFile::parse(data)? })
    }

    pub fn ensure_string(&mut self, value: &str) -> u32 {
        if let Some(i) = self.abc.strings.iter().position(|s| s == value) {
            return i as u32;
        }
        self.abc.strings.push(value.to_string());
        (self.abc.strings.len() - 1) as u32
    }
    pub fn ensure_int(&mut self, value: i32) -> u32 {
        if let Some(i) = self.abc.integers.iter().position(|&v| v == value) {
            return i as u32;
        }
        self.abc.integers.push(value);
        (self.abc.integers.len() - 1) as u32
    }
    pub fn ensure_uint(&mut self, value: u32) -> u32 {
        if let Some(i) = self.abc.uintegers.iter().position(|&v| v == value) {
            return i as u32;
        }
        self.abc.uintegers.push(value);
        (self.abc.uintegers.len() - 1) as u32
    }
    /// Bit-exact lookup, so NaN is found again.
    pub fn ensure_double(&mut self, value: f64) -> u32 {
        if let Some(i) = self.abc.doubles.iter().position(|v| v.to_bits() == value.to_bits()) {
            return i as u32;
        }
        self.abc.doubles.push(value);
        (self.abc.doubles.len() - 1) as u32
    }
    pub fn ensure_namespace(&mut self, kind: u8, name: &str) -> u32 {
        let name_idx = self.ensure_string(name);
        if let Some(i) = self.abc.namespaces.iter().position(|&(k, n)| k == kind && n == name_idx) {
            return i as u32;
        }
        self.abc.namespaces.push((kind, name_idx));
        (self.abc.namespaces.len() - 1) as u32
    }
    pub fn ensure_ns_set(&mut self, ns: &[u32]) -> u32 {
        if let Some(i) = self.abc.ns_sets.iter().position(|s| s == ns) {
            return i as u32;
        }
        self.abc.ns_sets.push(ns.to_vec());
        (self.abc.ns_sets.len() - 1) as u32
    }
    pub fn ensure_multiname(&mut self, kind: u8, data: &[u32]) -> u32 {
        if let Some(i) = self.abc.multinames.iter().position(|m| m.kind == kind && m.data == data) {
            return i as u32;
        }
        self.abc.multinames.push(Multiname::new(kind, data));
        (self.abc.multinames.len() - 1) as u32
    }
    pub fn ensure_qname(&mut self, ns_idx: u32, name: &str) -> u32 {
        let n = self.ensure_string(name);
        self.ensure_multiname(MN_QNAME, &[ns_idx, n])
    }
    pub fn ensure_qname_by_name(&mut self, namespace_uri: &str, name: &str, ns_kind: u8) -> u32 {
        let ns = self.ensure_namespace(ns_kind, namespace_uri);
        self.ensure_qname(ns, name)
    }
    pub fn ensure_typename(&mut self, base: u32, params: &[u32]) -> u32 {
        let mut d = vec![base];
        d.extend_from_slice(params);
        self.ensure_multiname(MN_TYPENAME, &d)
    }
    pub fn find_string(&self, value: &str) -> Option<u32> {
        self.abc.strings.iter().position(|s| s == value).map(|i| i as u32)
    }
    pub fn find_multiname(&self, full_name: &str) -> Option<u32> {
        (1..self.abc.multinames.len() as u32).find(|&i| self.abc.mn_full(i) == full_name)
    }
    pub fn find_namespace(&self, kind: u8, name: &str) -> Option<u32> {
        for (i, &(k, name_idx)) in self.abc.namespaces.iter().enumerate() {
            if k == kind {
                if name_idx > 0 && (name_idx as usize) < self.abc.strings.len() {
                    if self.abc.strings[name_idx as usize] == name {
                        return Some(i as u32);
                    }
                } else if name_idx == 0 && name.is_empty() {
                    return Some(i as u32);
                }
            }
        }
        None
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_method(
        &mut self,
        return_type: u32,
        param_types: &[u32],
        name: &str,
        mut flags: u8,
        optional_values: &[(u8, u32)],
        param_names: &[&str],
        code: Option<Vec<u8>>,
        max_stack: u32,
        local_count: u32,
        init_scope_depth: u32,
        max_scope_depth: u32,
        exceptions: Vec<ExceptionInfo>,
        traits: Vec<TraitInfo>,
    ) -> u32 {
        let mut mi = MethodInfo {
            param_count: param_types.len() as u32,
            return_type,
            param_types: param_types.to_vec(),
            name_idx: if name.is_empty() { 0 } else { self.ensure_string(name) },
            ..Default::default()
        };
        if !optional_values.is_empty() {
            flags |= METHOD_HAS_OPTIONAL;
            mi.optional_values = optional_values.to_vec();
        }
        if !param_names.is_empty() {
            flags |= METHOD_HAS_PARAM_NAMES;
            mi.param_names = param_names.iter().map(|n| self.ensure_string(n)).collect();
        }
        mi.flags = flags;
        let method_idx = self.abc.methods.len() as u32;
        self.abc.methods.push(mi);
        self.abc.method_bodies.insert(
            method_idx,
            MethodBody {
                method_idx,
                max_stack,
                local_count,
                init_scope_depth,
                max_scope_depth,
                code: code.unwrap_or_else(|| vec![0x47]),
                exceptions,
                traits,
            },
        );
        method_idx
    }

    pub fn replace_method_body(&mut self, method_idx: u32, code: Vec<u8>) -> &MethodBody {
        let existing = self.abc.method_bodies.get(&method_idx).cloned();
        let mb = match existing {
            Some(e) => MethodBody { code, ..e },
            None => MethodBody { method_idx, code, max_stack: 1, local_count: 1, init_scope_depth: 0, max_scope_depth: 1, ..Default::default() },
        };
        self.abc.method_bodies.insert(method_idx, mb);
        &self.abc.method_bodies[&method_idx]
    }

    pub fn make_slot_trait(name_idx: u32, type_name: u32, slot_id: u32, vindex: u32, vkind: u8, is_const: bool) -> TraitInfo {
        TraitInfo {
            name_idx,
            kind: if is_const { TRAIT_CONST } else { TRAIT_SLOT },
            slot_id,
            type_name,
            vindex,
            vkind,
            ..Default::default()
        }
    }
    pub fn make_method_trait(name_idx: u32, method_idx: u32, disp_id: u32, attr: u8) -> TraitInfo {
        TraitInfo { name_idx, kind: TRAIT_METHOD, attr, disp_id, method_idx, ..Default::default() }
    }
    pub fn make_getter_trait(name_idx: u32, method_idx: u32, disp_id: u32, attr: u8) -> TraitInfo {
        TraitInfo { name_idx, kind: TRAIT_GETTER, attr, disp_id, method_idx, ..Default::default() }
    }
    pub fn make_setter_trait(name_idx: u32, method_idx: u32, disp_id: u32, attr: u8) -> TraitInfo {
        TraitInfo { name_idx, kind: TRAIT_SETTER, attr, disp_id, method_idx, ..Default::default() }
    }

    pub fn find_class(&self, full_name: &str) -> Option<usize> {
        self.abc.instances.iter().position(|i| self.abc.mn_full(i.name_idx) == full_name)
    }
    pub fn list_classes(&self) -> Vec<String> {
        self.abc.instances.iter().map(|i| self.abc.mn_full(i.name_idx)).collect()
    }

    /// Method indices of a class keyed `constructor`, `cinit`, `method:n`,
    /// `get:n`, `set:n`, `static:<kind>:n` (insertion ordered).
    pub fn get_class_methods(&self, class_idx: usize) -> Vec<(String, u32)> {
        let inst = &self.abc.instances[class_idx];
        let cls = &self.abc.classes[class_idx];
        let mut out: Vec<(String, u32)> = vec![("constructor".into(), inst.iinit), ("cinit".into(), cls.cinit)];
        let put = |out: &mut Vec<(String, u32)>, k: String, v: u32| {
            if let Some(e) = out.iter_mut().find(|e| e.0 == k) {
                e.1 = v;
            } else {
                out.push((k, v));
            }
        };
        for t in &inst.traits {
            if let Some(pf) = kind_prefix(t.kind) {
                put(&mut out, format!("{pf}:{}", self.abc.mn_name(t.name_idx)), t.method_idx);
            }
        }
        for t in &cls.traits {
            if let Some(pf) = kind_prefix(t.kind) {
                put(&mut out, format!("static:{pf}:{}", self.abc.mn_name(t.name_idx)), t.method_idx);
            }
        }
        out
    }

    pub fn add_instance_trait(&mut self, class_idx: usize, t: TraitInfo) {
        self.abc.instances[class_idx].traits.push(t);
    }
    pub fn add_class_trait(&mut self, class_idx: usize, t: TraitInfo) {
        self.abc.classes[class_idx].traits.push(t);
    }
    pub fn remove_instance_trait(&mut self, class_idx: usize, name: &str) -> Option<TraitInfo> {
        let abc = &self.abc;
        let pos = abc.instances[class_idx].traits.iter().position(|t| abc.mn_name(t.name_idx) == name)?;
        Some(self.abc.instances[class_idx].traits.remove(pos))
    }
    pub fn remove_class_trait(&mut self, class_idx: usize, name: &str) -> Option<TraitInfo> {
        let abc = &self.abc;
        let pos = abc.classes[class_idx].traits.iter().position(|t| abc.mn_name(t.name_idx) == name)?;
        Some(self.abc.classes[class_idx].traits.remove(pos))
    }
    pub fn set_super_class(&mut self, class_idx: usize, super_mn: u32) {
        self.abc.instances[class_idx].super_idx = super_mn;
    }
    pub fn add_interface(&mut self, class_idx: usize, mn: u32) {
        let i = &mut self.abc.instances[class_idx];
        if !i.interfaces.contains(&mn) {
            i.interfaces.push(mn);
        }
    }
    pub fn remove_interface(&mut self, class_idx: usize, mn: u32) {
        self.abc.instances[class_idx].interfaces.retain(|&x| x != mn);
    }
    pub fn add_metadata(&mut self, name: &str, items: &[(&str, &str)]) -> u32 {
        let n = self.ensure_string(name);
        let its: Vec<(u32, u32)> = items.iter().map(|(k, v)| (self.ensure_string(k), self.ensure_string(v))).collect();
        self.abc.metadata_entries.push((n, its));
        (self.abc.metadata_entries.len() - 1) as u32
    }
    pub fn serialize(&self) -> Vec<u8> {
        self.abc.serialize()
    }

    /// Serialize and re-parse, checking that the structures survive.
    pub fn validate_round_trip(&self) -> Result<(), String> {
        let abc2 = AbcFile::parse(&self.serialize())?;
        let a = &self.abc;
        let chk = |name: &str, x: usize, y: usize| -> Result<(), String> {
            if x == y { Ok(()) } else { Err(format!("{name}: {x} -> {y}")) }
        };
        chk("String pool", a.strings.len(), abc2.strings.len())?;
        chk("Integer pool", a.integers.len(), abc2.integers.len())?;
        chk("UInt pool", a.uintegers.len(), abc2.uintegers.len())?;
        chk("Double pool", a.doubles.len(), abc2.doubles.len())?;
        chk("Namespace pool", a.namespaces.len(), abc2.namespaces.len())?;
        chk("NS set pool", a.ns_sets.len(), abc2.ns_sets.len())?;
        chk("Multiname pool", a.multinames.len(), abc2.multinames.len())?;
        chk("Methods", a.methods.len(), abc2.methods.len())?;
        chk("Instances", a.instances.len(), abc2.instances.len())?;
        chk("Classes", a.classes.len(), abc2.classes.len())?;
        chk("Scripts", a.scripts.len(), abc2.scripts.len())?;
        chk("Method bodies", a.method_bodies.len(), abc2.method_bodies.len())?;
        for (idx, b) in &a.method_bodies {
            match abc2.method_bodies.get(idx) {
                Some(b2) if b2.code == b.code => {}
                Some(b2) => return Err(format!("Method {idx} code: {}B -> {}B", b.code.len(), b2.code.len())),
                None => return Err(format!("Method body {idx} missing after round-trip")),
            }
        }
        Ok(())
    }
}

/// `{Method: method, Getter: get, Setter: set, Function: method}`
pub fn kind_prefix(kind: u8) -> Option<&'static str> {
    match kind {
        TRAIT_METHOD | TRAIT_FUNCTION => Some("method"),
        TRAIT_GETTER => Some("get"),
        TRAIT_SETTER => Some("set"),
        _ => None,
    }
}

// ---------------------------------------------------------------- DoABC tags

/// `(name, abc_bytes)` of a DoABC / DoABC2 tag body, `None` for a malformed DoABC2.
pub fn abc_from_tag(code: u16, body: &[u8]) -> Option<(String, Vec<u8>)> {
    match code {
        TAG_DOABC2 => {
            if body.len() < 5 {
                return None;
            }
            let nul = body[4..].iter().position(|&b| b == 0)? + 4;
            let name = String::from_utf8_lossy(&body[4..nul]).into_owned();
            Some((if name.is_empty() { "(unnamed)".into() } else { name }, body[nul + 1..].to_vec()))
        }
        TAG_DOABC => Some(("DoABC".into(), body.to_vec())),
        _ => None,
    }
}

/// Is `data` a SWF (FWS/CWS/ZWS signature) rather than a bare ABC block?
pub fn looks_like_swf(data: &[u8]) -> bool {
    data.len() >= 2 && matches!(&data[..2], b"FW" | b"CW" | b"ZW")
}

/// `read_abc_blocks`: `(swf_version, [(block_name, abc_bytes)])` of a SWF file image.
pub fn read_abc_blocks(swf: &[u8]) -> Result<(u8, Vec<(String, Vec<u8>)>), String> {
    let s = super::ac_swf::parse(swf)?;
    let blocks = s
        .tags
        .iter()
        .filter(|t| t.code != super::ac_swf::END)
        .filter_map(|t| abc_from_tag(t.code, &t.body))
        .collect();
    Ok((s.version, blocks))
}
