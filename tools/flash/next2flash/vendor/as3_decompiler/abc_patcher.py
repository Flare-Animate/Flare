"""ABC binary patcher — serializer, pool merger, bytecode translator.

Allows surgical modification of a single class in an ABC block
without touching other classes.  The workflow is:

  1. Parse both the *original* ABC and the *compiled* ABC (from mxmlc)
  2. Merge the compiled ABC's constant-pool entries into the original
  3. Translate the compiled class's method-body bytecode to reference
     the merged pool indices
  4. Replace only the edited class's method bodies in the original ABC
  5. Serialize the patched ABC back to bytes

All other classes keep their original bytecode byte-for-byte.
"""

from __future__ import annotations

import logging
import re
import struct
from typing import Dict, List, Optional, Set, Tuple

log = logging.getLogger(__name__)

from .abc_parser import (
    ABCFile, MethodInfo, MethodBody, ExceptionInfo, TraitInfo,
    InstanceInfo, ClassInfo, ScriptInfo,
    MN_QName, MN_QNameA, MN_RTQName, MN_RTQNameA, MN_RTQNameL,
    MN_RTQNameLA, MN_Multiname, MN_MultinameA, MN_MultinameL,
    MN_MultinameLA, MN_TypeName,
    TRAIT_Slot, TRAIT_Method, TRAIT_Getter, TRAIT_Setter,
    TRAIT_Class, TRAIT_Function, TRAIT_Const,
    METHOD_HasOptional, METHOD_HasParamNames,
    INSTANCE_ProtectedNs,
    NS_PrivateNs,
    _ru8, _ru30, _rs24,
)

__all__ = ['serialize_abc', 'transplant_class', 'extract_method_texts']


# ═══════════════════════════════════════════════════════════════════════════
#  Writer helpers  (inverse of _ru30 etc.)
# ═══════════════════════════════════════════════════════════════════════════

def _wu8(buf: bytearray, v: int):
    buf.append(v & 0xFF)

def _wu16(buf: bytearray, v: int):
    buf.extend(struct.pack('<H', v & 0xFFFF))

def _ws24(buf: bytearray, v: int):
    if v < 0:
        v += 0x1000000
    buf.append(v & 0xFF)
    buf.append((v >> 8) & 0xFF)
    buf.append((v >> 16) & 0xFF)

def _wu30(buf: bytearray, v: int):
    """Encode a u30/u32 variable-length integer."""
    v = v & 0xFFFFFFFF
    while True:
        byte = v & 0x7F
        v >>= 7
        if v:
            buf.append(byte | 0x80)
        else:
            buf.append(byte)
            break

def _wd64(buf: bytearray, v: float):
    buf.extend(struct.pack('<d', v))


# ═══════════════════════════════════════════════════════════════════════════
#  ABC Serializer
# ═══════════════════════════════════════════════════════════════════════════

def _serialize_trait(buf: bytearray, t: TraitInfo):
    _wu30(buf, t.name_idx)
    kb = (t.kind & 0x0F) | ((t.attr & 0x0F) << 4)
    _wu8(buf, kb)
    if t.kind in (TRAIT_Slot, TRAIT_Const):
        _wu30(buf, t.slot_id)
        _wu30(buf, t.type_name)
        _wu30(buf, t.vindex)
        if t.vindex:
            _wu8(buf, t.vkind)
    elif t.kind in (TRAIT_Method, TRAIT_Getter, TRAIT_Setter):
        _wu30(buf, t.disp_id)
        _wu30(buf, t.method_idx)
    elif t.kind == TRAIT_Class:
        _wu30(buf, t.slot_id)
        _wu30(buf, t.class_idx)
    elif t.kind == TRAIT_Function:
        _wu30(buf, t.slot_id)
        _wu30(buf, t.method_idx)
    if t.attr & 0x04:  # ATTR_Metadata
        _wu30(buf, len(t.metadata))
        for mi in t.metadata:
            _wu30(buf, mi)


def serialize_abc(abc: ABCFile) -> bytes:
    """Serialize a parsed ABCFile back to raw bytes."""
    log.debug("serialize_abc: %d strings, %d methods, %d classes",
              len(abc.strings), len(abc.methods), len(abc.instances))
    buf = bytearray()

    # Version
    _wu16(buf, abc.minor)
    _wu16(buf, abc.major)

    # ── Constant Pool ─────────────────────────────────────────────
    # Use original pool counts when available for faithful round-trip.
    # AVM2 spec allows count=0 to mean "only the default entry",
    # so we must preserve the original encoding when the pool is unmodified.
    raw = getattr(abc, '_raw_pool_counts', {})

    def _pool_count(pool_name: str, pool: list) -> int:
        """Determine the count to write for a constant pool.
        
        If the pool was extended (new entries added), use len(pool).
        If the pool hasn't changed and the raw count was 0, preserve 0.
        Otherwise use len(pool).
        """
        raw_count = raw.get(pool_name)
        if raw_count is not None and len(pool) <= max(1, raw_count):
            return raw_count
        return len(pool)

    # integers
    _wu30(buf, _pool_count('integers', abc.integers))
    for v in abc.integers[1:]:
        # Encode as u32 (unsigned encoding of possibly negative value)
        if v < 0:
            v += 0x100000000
        _wu30(buf, v)

    # uintegers
    _wu30(buf, _pool_count('uintegers', abc.uintegers))
    for v in abc.uintegers[1:]:
        _wu30(buf, v)

    # doubles
    _wu30(buf, _pool_count('doubles', abc.doubles))
    for v in abc.doubles[1:]:
        _wd64(buf, v)

    # strings
    _wu30(buf, len(abc.strings))
    for s in abc.strings[1:]:
        encoded = s.encode('utf-8')
        _wu30(buf, len(encoded))
        buf.extend(encoded)

    # namespaces
    _wu30(buf, len(abc.namespaces))
    for kind, name_idx in abc.namespaces[1:]:
        _wu8(buf, kind)
        _wu30(buf, name_idx)

    # namespace sets
    _wu30(buf, len(abc.ns_sets))
    for ns_set in abc.ns_sets[1:]:
        _wu30(buf, len(ns_set))
        for ns_idx in ns_set:
            _wu30(buf, ns_idx)

    # multinames
    _wu30(buf, len(abc.multinames))
    for kind, data in abc.multinames[1:]:
        _wu8(buf, kind)
        if kind in (MN_QName, MN_QNameA):
            _wu30(buf, data[0])  # ns
            _wu30(buf, data[1])  # name
        elif kind in (MN_RTQName, MN_RTQNameA):
            _wu30(buf, data[0])  # name
        elif kind in (MN_RTQNameL, MN_RTQNameLA):
            pass  # no data
        elif kind in (MN_Multiname, MN_MultinameA):
            _wu30(buf, data[0])  # name
            _wu30(buf, data[1])  # ns_set
        elif kind in (MN_MultinameL, MN_MultinameLA):
            _wu30(buf, data[0])  # ns_set
        elif kind == MN_TypeName:
            _wu30(buf, data[0])  # qn
            _wu30(buf, len(data[1]))  # param count
            for pm in data[1]:
                _wu30(buf, pm)

    # ── Methods ───────────────────────────────────────────────────
    _wu30(buf, len(abc.methods))
    for m in abc.methods:
        _wu30(buf, m.param_count)
        _wu30(buf, m.return_type)
        for pt in m.param_types:
            _wu30(buf, pt)
        _wu30(buf, m.name_idx)
        _wu8(buf, m.flags)
        if m.flags & METHOD_HasOptional:
            _wu30(buf, len(m.optional_values))
            for vkind, val in m.optional_values:
                _wu30(buf, val)
                _wu8(buf, vkind)
        if m.flags & METHOD_HasParamNames:
            for nm in m.param_names:
                _wu30(buf, nm)

    # ── Metadata ──────────────────────────────────────────────────
    _wu30(buf, len(abc.metadata_entries))
    for name_idx, items in abc.metadata_entries:
        _wu30(buf, name_idx)
        _wu30(buf, len(items))
        for k, v in items:
            _wu30(buf, k)
            _wu30(buf, v)

    # ── Instances + Classes ───────────────────────────────────────
    _wu30(buf, len(abc.instances))
    for inst in abc.instances:
        _wu30(buf, inst.name_idx)
        _wu30(buf, inst.super_idx)
        _wu8(buf, inst.flags)
        if inst.flags & INSTANCE_ProtectedNs:
            _wu30(buf, inst.protected_ns)
        _wu30(buf, len(inst.interfaces))
        for ii in inst.interfaces:
            _wu30(buf, ii)
        _wu30(buf, inst.iinit)
        _wu30(buf, len(inst.traits))
        for tr in inst.traits:
            _serialize_trait(buf, tr)

    for ci in abc.classes:
        _wu30(buf, ci.cinit)
        _wu30(buf, len(ci.traits))
        for tr in ci.traits:
            _serialize_trait(buf, tr)

    # ── Scripts ───────────────────────────────────────────────────
    _wu30(buf, len(abc.scripts))
    for si in abc.scripts:
        _wu30(buf, si.sinit)
        _wu30(buf, len(si.traits))
        for tr in si.traits:
            _serialize_trait(buf, tr)

    # ── Method Bodies ─────────────────────────────────────────────
    # Preserve original ordering when available
    body_order = getattr(abc, '_method_body_order', None)
    if body_order:
        bodies = [abc.method_bodies[midx] for midx in body_order
                  if midx in abc.method_bodies]
        # Also include any NEW bodies not in the original order
        original_set = set(body_order)
        for midx, mb in sorted(abc.method_bodies.items()):
            if midx not in original_set:
                bodies.append(mb)
    else:
        bodies = sorted(abc.method_bodies.values(), key=lambda b: b.method_idx)
    _wu30(buf, len(bodies))
    for mb in bodies:
        _wu30(buf, mb.method_idx)
        _wu30(buf, mb.max_stack)
        _wu30(buf, mb.local_count)
        _wu30(buf, mb.init_scope_depth)
        _wu30(buf, mb.max_scope_depth)
        _wu30(buf, len(mb.code))
        buf.extend(mb.code)
        _wu30(buf, len(mb.exceptions))
        for ei in mb.exceptions:
            _wu30(buf, ei.from_pos)
            _wu30(buf, ei.to_pos)
            _wu30(buf, ei.target)
            _wu30(buf, ei.exc_type)
            _wu30(buf, ei.var_name)
        _wu30(buf, len(mb.traits))
        for tr in mb.traits:
            _serialize_trait(buf, tr)

    return bytes(buf)


# ═══════════════════════════════════════════════════════════════════════════
#  AVM2 Opcode Operand Table
# ═══════════════════════════════════════════════════════════════════════════
#
# Each opcode maps to a tuple of operand descriptors:
#   'u30'        — generic u30 (register, arg_count, slot, scope, etc.)
#   'multiname'  — u30 index into multinames pool
#   'string'     — u30 index into strings pool
#   'int'        — u30 index into integers pool
#   'uint'       — u30 index into uintegers pool
#   'double'     — u30 index into doubles pool
#   'namespace'  — u30 index into namespaces pool
#   'method'     — u30 index into methods array
#   'class'      — u30 index into classes array
#   'u8'         — single byte
#   's24'        — signed 24-bit offset (branch)
#   'switch'     — lookupswitch (special handling)
#   'debug'      — debug opcode (special encoding)

_OP_FORMATS: Dict[int, tuple] = {
    0x01: (),            # bkpt
    0x02: (),            # nop
    0x03: (),            # throw
    0x04: ('multiname',),  # getsuper
    0x05: ('multiname',),  # setsuper
    0x06: ('string',),     # dxns
    0x07: (),              # dxnslate
    0x08: ('u30',),        # kill
    0x09: (),              # label
    0x0C: ('s24',),        # ifnlt
    0x0D: ('s24',),        # ifnle
    0x0E: ('s24',),        # ifngt
    0x0F: ('s24',),        # ifnge
    0x10: ('s24',),        # jump
    0x11: ('s24',),        # iftrue
    0x12: ('s24',),        # iffalse
    0x13: ('s24',),        # ifeq
    0x14: ('s24',),        # ifne
    0x15: ('s24',),        # iflt
    0x16: ('s24',),        # ifle
    0x17: ('s24',),        # ifgt
    0x18: ('s24',),        # ifge
    0x19: ('s24',),        # ifstricteq
    0x1A: ('s24',),        # ifstrictne
    0x1B: ('switch',),     # lookupswitch
    0x1C: (),              # pushwith
    0x1D: (),              # popscope
    0x1E: (),              # nextname
    0x1F: (),              # hasnext
    0x20: (),              # pushnull
    0x21: (),              # pushundefined
    0x23: (),              # nextvalue
    0x24: ('u8',),         # pushbyte
    0x25: ('u30',),        # pushshort
    0x26: (),              # pushtrue
    0x27: (),              # pushfalse
    0x28: (),              # pushnan
    0x29: (),              # pop
    0x2A: (),              # dup
    0x2B: (),              # swap
    0x2C: ('string',),     # pushstring
    0x2D: ('int',),        # pushint
    0x2E: ('uint',),       # pushuint
    0x2F: ('double',),     # pushdouble
    0x30: (),              # pushscope
    0x31: ('namespace',),  # pushnamespace
    0x32: ('u30', 'u30'),  # hasnext2
    # 0x33, 0x34: unused
    0x35: (),              # li8
    0x36: (),              # li16
    0x37: (),              # li32
    0x38: (),              # lf32
    0x39: (),              # lf64
    0x3A: (),              # si8
    0x3B: (),              # si16
    0x3C: (),              # si32
    0x3D: (),              # sf32
    0x3E: (),              # sf64
    0x40: ('method',),     # newfunction
    0x41: ('u30',),        # call
    0x42: ('u30',),        # construct
    0x43: ('u30', 'u30'),  # callmethod  (disp_id, arg_count)
    0x44: ('method', 'u30'),  # callstatic  (method_idx, arg_count)
    0x45: ('multiname', 'u30'),  # callsuper
    0x46: ('multiname', 'u30'),  # callproperty
    0x47: (),              # returnvoid
    0x48: (),              # returnvalue
    0x49: ('u30',),        # constructsuper
    0x4A: ('multiname', 'u30'),  # constructprop
    0x4C: ('multiname', 'u30'),  # callproplex
    0x4E: ('multiname', 'u30'),  # callsupervoid
    0x4F: ('multiname', 'u30'),  # callpropvoid
    0x50: (),              # sxi1
    0x51: (),              # sxi8
    0x52: (),              # sxi16
    0x53: ('u30',),        # applytype
    0x55: ('u30',),        # newobject
    0x56: ('u30',),        # newarray
    0x57: (),              # newactivation
    0x58: ('class',),      # newclass
    0x59: ('multiname',),  # getdescendants
    0x5A: ('u30',),        # newcatch
    0x5D: ('multiname',),  # findpropstrict
    0x5E: ('multiname',),  # findproperty
    0x5F: ('multiname',),  # finddef
    0x60: ('multiname',),  # getlex
    0x61: ('multiname',),  # setproperty
    0x62: ('u30',),        # getlocal
    0x63: ('u30',),        # setlocal
    0x64: (),              # getglobalscope
    0x65: ('u8',),         # getscopeobject
    0x66: ('multiname',),  # getproperty
    0x68: ('multiname',),  # initproperty
    0x6A: ('multiname',),  # deleteproperty
    0x6C: ('u30',),        # getslot
    0x6D: ('u30',),        # setslot
    0x6E: ('u30',),        # getglobalslot
    0x6F: ('u30',),        # setglobalslot
    0x70: (),              # convert_s
    0x71: (),              # esc_xelem
    0x72: (),              # esc_xattr
    0x73: (),              # convert_i
    0x74: (),              # convert_u
    0x75: (),              # convert_d
    0x76: (),              # convert_b
    0x77: (),              # convert_o
    0x78: (),              # checkfilter
    0x80: ('multiname',),  # coerce
    0x81: (),              # coerce_b
    0x82: (),              # coerce_a
    0x83: (),              # coerce_i
    0x84: (),              # coerce_d
    0x85: (),              # coerce_s
    0x86: ('multiname',),  # astype
    0x87: (),              # astypelate
    0x88: (),              # coerce_u
    0x89: (),              # coerce_o
    0x90: (),              # negate
    0x91: (),              # increment
    0x92: ('u30',),        # inclocal
    0x93: (),              # decrement
    0x94: ('u30',),        # declocal
    0x95: (),              # typeof
    0x96: (),              # not
    0x97: (),              # bitnot
    0xA0: (),              # add
    0xA1: (),              # subtract
    0xA2: (),              # multiply
    0xA3: (),              # divide
    0xA4: (),              # modulo
    0xA5: (),              # lshift
    0xA6: (),              # rshift
    0xA7: (),              # urshift
    0xA8: (),              # bitand
    0xA9: (),              # bitor
    0xAA: (),              # bitxor
    0xAB: (),              # equals
    0xAC: (),              # strictequals
    0xAD: (),              # lessthan
    0xAE: (),              # lessequals
    0xAF: (),              # greaterthan
    0xB0: (),              # greaterequals
    0xB1: (),              # instanceof
    0xB2: ('multiname',),  # istype
    0xB3: (),              # istypelate
    0xB4: (),              # in
    0xC0: (),              # increment_i
    0xC1: (),              # decrement_i
    0xC2: ('u30',),        # inclocal_i
    0xC3: ('u30',),        # declocal_i
    0xC4: (),              # negate_i
    0xC5: (),              # add_i
    0xC6: (),              # subtract_i
    0xC7: (),              # multiply_i
    0xD0: (),              # getlocal_0
    0xD1: (),              # getlocal_1
    0xD2: (),              # getlocal_2
    0xD3: (),              # getlocal_3
    0xD4: (),              # setlocal_0
    0xD5: (),              # setlocal_1
    0xD6: (),              # setlocal_2
    0xD7: (),              # setlocal_3
    0xEF: ('debug',),      # debug  (special)
    0xF0: ('u30',),        # debugline
    0xF1: ('string',),     # debugfile
}


# ═══════════════════════════════════════════════════════════════════════════
#  Constant-pool merger — merge src pools into dst, return index mappings
# ═══════════════════════════════════════════════════════════════════════════

class _PoolMapping:
    """Maps indices from source ABC's pools to destination ABC's pools."""

    def __init__(self):
        self.strings: Dict[int, int] = {0: 0}
        self.integers: Dict[int, int] = {0: 0}
        self.uintegers: Dict[int, int] = {0: 0}
        self.doubles: Dict[int, int] = {0: 0}
        self.namespaces: Dict[int, int] = {0: 0}
        self.ns_sets: Dict[int, int] = {0: 0}
        self.multinames: Dict[int, int] = {0: 0}
        self.methods: Dict[int, int] = {}
        self.classes: Dict[int, int] = {}


def _build_private_ns_overrides(
    orig: ABCFile, compiled: ABCFile,
    orig_ci: int, compiled_ci: int,
) -> Dict[int, int]:
    """Build a mapping from compiled PrivateNs indices → original PrivateNs indices.

    AVM2 private namespaces are **identity-based** (identified by pool index,
    not by name string).  When mxmlc compiles a class standalone, it creates
    PrivateNs entries with descriptive names (e.g. ``"com.mcleodgaming.ssf2:Main"``)
    while the original SWF may use anonymous ``PrivateNs("")`` entries.

    Without this mapping the pool merger creates **new** PrivateNs entries that
    don't match the original class's declared trait namespaces, causing an
    AVM2 VerifyError when the transplanted bytecode tries to access private
    members through the wrong namespace.

    The mapping is discovered by collecting PrivateNs indices from each ABC's
    class traits and matching them by overlapping member names.

    We map ALL matching classes (not just the target) so that pool entries
    from other compiled classes (brought in by mxmlc's full compilation)
    also map correctly — especially important for namespace sets used in
    Multiname late-binding references.
    """

    def _collect_private_ns(abc: ABCFile, ci: int) -> Dict[int, Set[str]]:
        """Return {ns_pool_idx: {member_name, …}} for PrivateNs-qualified traits."""
        inst = abc.instances[ci]
        cls = abc.classes[ci]
        ns_members: Dict[int, Set[str]] = {}
        for traits in (inst.traits, cls.traits):
            for t in traits:
                if t.name_idx == 0 or t.name_idx >= len(abc.multinames):
                    continue
                kind, data = abc.multinames[t.name_idx]
                if kind in (MN_QName, MN_QNameA) and data and len(data) >= 2:
                    ns_idx, name_idx = data[0], data[1]
                    if 0 < ns_idx < len(abc.namespaces):
                        ns_kind, _ = abc.namespaces[ns_idx]
                        if ns_kind == NS_PrivateNs:
                            name_str = (abc.strings[name_idx]
                                        if 0 < name_idx < len(abc.strings)
                                        else "")
                            ns_members.setdefault(ns_idx, set()).add(name_str)
        return ns_members

    overrides: Dict[int, int] = {}
    used_orig: Set[int] = set()

    # Match ALL compiled classes to their original counterparts by name,
    # then map their PrivateNs by trait-member overlap.
    # Build index of original classes by qualified name.
    orig_class_by_name: Dict[str, int] = {}
    for ci in range(len(orig.instances)):
        name = orig.mn_full(orig.instances[ci].name_idx)
        orig_class_by_name[name] = ci

    for comp_ci_iter in range(len(compiled.instances)):
        comp_name = compiled.mn_full(compiled.instances[comp_ci_iter].name_idx)
        o_ci = orig_class_by_name.get(comp_name)
        if o_ci is None:
            continue

        comp_ns = _collect_private_ns(compiled, comp_ci_iter)
        orig_ns = _collect_private_ns(orig, o_ci)

        # Greedily match each compiled PrivateNs to the original with the
        # most overlapping member names (typically exactly one per class).
        for comp_ns_idx, comp_names in sorted(
            comp_ns.items(), key=lambda x: -len(x[1])
        ):
            if comp_ns_idx in overrides:
                continue
            best_orig: Optional[int] = None
            best_overlap = 0
            for orig_ns_idx, orig_names in orig_ns.items():
                if orig_ns_idx in used_orig:
                    continue
                overlap = len(comp_names & orig_names)
                if overlap > best_overlap:
                    best_overlap = overlap
                    best_orig = orig_ns_idx
            if best_orig is not None:
                overrides[comp_ns_idx] = best_orig
                used_orig.add(best_orig)

    return overrides


def _merge_pools(dst: ABCFile, src: ABCFile,
                 ns_overrides: Optional[Dict[int, int]] = None) -> _PoolMapping:
    """Merge src's constant-pool entries into dst.  Returns index mapping.

    For each entry in src's pools, we either find an identical one in dst
    (reuse its index) or append it to dst (creating a new index).

    Pool entries that reference other pool entries (namespaces → strings,
    multinames → namespaces/strings/ns_sets) are handled in dependency order.
    """
    log.debug("_merge_pools: dst_strings=%d src_strings=%d", len(dst.strings), len(src.strings))
    pm = _PoolMapping()

    # --- strings ---
    dst_str_lookup = {s: i for i, s in enumerate(dst.strings)}
    for si in range(1, len(src.strings)):
        s = src.strings[si]
        if s in dst_str_lookup:
            pm.strings[si] = dst_str_lookup[s]
        else:
            new_idx = len(dst.strings)
            dst.strings.append(s)
            dst_str_lookup[s] = new_idx
            pm.strings[si] = new_idx

    # --- integers ---
    dst_int_lookup = {v: i for i, v in enumerate(dst.integers)}
    for si in range(1, len(src.integers)):
        v = src.integers[si]
        if v in dst_int_lookup:
            pm.integers[si] = dst_int_lookup[v]
        else:
            new_idx = len(dst.integers)
            dst.integers.append(v)
            dst_int_lookup[v] = new_idx
            pm.integers[si] = new_idx

    # --- uintegers ---
    dst_uint_lookup = {v: i for i, v in enumerate(dst.uintegers)}
    for si in range(1, len(src.uintegers)):
        v = src.uintegers[si]
        if v in dst_uint_lookup:
            pm.uintegers[si] = dst_uint_lookup[v]
        else:
            new_idx = len(dst.uintegers)
            dst.uintegers.append(v)
            dst_uint_lookup[v] = new_idx
            pm.uintegers[si] = new_idx

    # --- doubles ---
    # Special handling for NaN (NaN != NaN, so use bit representation)
    import math
    dst_dbl_lookup: Dict[int, int] = {}
    for i, v in enumerate(dst.doubles):
        bits = struct.pack('<d', v)
        dst_dbl_lookup[bits] = i
    for si in range(1, len(src.doubles)):
        v = src.doubles[si]
        bits = struct.pack('<d', v)
        if bits in dst_dbl_lookup:
            pm.doubles[si] = dst_dbl_lookup[bits]
        else:
            new_idx = len(dst.doubles)
            dst.doubles.append(v)
            dst_dbl_lookup[bits] = new_idx
            pm.doubles[si] = new_idx

    # --- namespaces (depend on strings) ---
    # A namespace is (kind, name_str_idx).  Map name_str_idx first.
    # ns_overrides lets callers force specific source namespace indices
    # to map to specific destination indices (used for PrivateNs identity
    # mapping — private namespaces are identity-based, not name-based).
    dst_ns_lookup: Dict[Tuple[int, int], int] = {}
    for i, (kind, name_idx) in enumerate(dst.namespaces):
        dst_ns_lookup[(kind, name_idx)] = i
    for si in range(1, len(src.namespaces)):
        # Check override FIRST (e.g. PrivateNs identity mapping)
        if ns_overrides and si in ns_overrides:
            pm.namespaces[si] = ns_overrides[si]
            continue
        kind, name_idx = src.namespaces[si]
        mapped_name = pm.strings.get(name_idx, name_idx)
        key = (kind, mapped_name)
        if key in dst_ns_lookup:
            pm.namespaces[si] = dst_ns_lookup[key]
        else:
            new_idx = len(dst.namespaces)
            dst.namespaces.append((kind, mapped_name))
            dst_ns_lookup[key] = new_idx
            pm.namespaces[si] = new_idx

    # --- namespace sets (depend on namespaces) ---
    dst_nss_lookup: Dict[tuple, int] = {}
    for i, ns_set in enumerate(dst.ns_sets):
        dst_nss_lookup[tuple(ns_set)] = i
    for si in range(1, len(src.ns_sets)):
        mapped_set = tuple(pm.namespaces.get(ns, ns) for ns in src.ns_sets[si])
        if mapped_set in dst_nss_lookup:
            pm.ns_sets[si] = dst_nss_lookup[mapped_set]
        else:
            new_idx = len(dst.ns_sets)
            dst.ns_sets.append(list(mapped_set))
            dst_nss_lookup[mapped_set] = new_idx
            pm.ns_sets[si] = new_idx

    # --- multinames (depend on strings, namespaces, ns_sets) ---
    # Build lookup for dst multinames
    dst_mn_lookup: Dict[Tuple[int, tuple], int] = {}
    for i, (kind, data) in enumerate(dst.multinames):
        if data is not None:
            dst_mn_lookup[(kind, data if isinstance(data, tuple) else ())] = i
        else:
            dst_mn_lookup[(kind, ())] = i

    for si in range(1, len(src.multinames)):
        kind, data = src.multinames[si]
        mapped_data: Optional[tuple] = None

        if kind in (MN_QName, MN_QNameA):
            ns = pm.namespaces.get(data[0], data[0])
            name = pm.strings.get(data[1], data[1])
            mapped_data = (ns, name)
        elif kind in (MN_RTQName, MN_RTQNameA):
            name = pm.strings.get(data[0], data[0])
            mapped_data = (name,)
        elif kind in (MN_RTQNameL, MN_RTQNameLA):
            mapped_data = ()
        elif kind in (MN_Multiname, MN_MultinameA):
            name = pm.strings.get(data[0], data[0])
            nss = pm.ns_sets.get(data[1], data[1])
            mapped_data = (name, nss)
        elif kind in (MN_MultinameL, MN_MultinameLA):
            nss = pm.ns_sets.get(data[0], data[0])
            mapped_data = (nss,)
        elif kind == MN_TypeName:
            qn = pm.multinames.get(data[0], data[0])
            params = tuple(pm.multinames.get(p, p) for p in data[1])
            mapped_data = (qn, params)
        else:
            mapped_data = data if isinstance(data, tuple) else ()

        key = (kind, mapped_data)
        if key in dst_mn_lookup:
            pm.multinames[si] = dst_mn_lookup[key]
        else:
            new_idx = len(dst.multinames)
            dst.multinames.append((kind, mapped_data))
            dst_mn_lookup[key] = new_idx
            pm.multinames[si] = new_idx

    return pm


# ═══════════════════════════════════════════════════════════════════════════
#  Instruction-level bytecode processing
# ═══════════════════════════════════════════════════════════════════════════

def _u30_byte_size(val: int) -> int:
    """Number of bytes needed to encode a u30 value."""
    val = val & 0xFFFFFFFF
    if val < 0x80: return 1
    if val < 0x4000: return 2
    if val < 0x200000: return 3
    if val < 0x10000000: return 4
    return 5


# Operand descriptor sets for encoding-size calculation
_U30_POOL_DESCS = frozenset({
    'multiname', 'string', 'int', 'uint', 'double',
    'namespace', 'method', 'class',
})
_U30_ALL_DESCS = _U30_POOL_DESCS | frozenset({
    'u30', 'switch_count', 'debug_string', 'debug_extra',
})
_U8_ALL_DESCS = frozenset({'u8', 'debug_type', 'debug_reg'})
_S24_ALL_DESCS = frozenset({'s24', 'switch_default_s24', 'switch_case_s24'})


def _operand_byte_size(desc: str, val: int) -> int:
    """Byte size of a single encoded operand."""
    if desc in _U30_ALL_DESCS:
        return _u30_byte_size(val)
    if desc in _U8_ALL_DESCS:
        return 1
    if desc in _S24_ALL_DESCS:
        return 3
    return 0


def _parse_instructions(code: bytes) -> list:
    """Parse AVM2 bytecode into instruction records.

    Returns list of ``(old_start, old_end, opcode, operands)``
    where *operands* is ``[(desc, value), ...]``.
    """
    instructions: list = []
    p = 0
    end = len(code)

    while p < end:
        inst_start = p
        op = code[p]; p += 1
        operands: list = []

        fmt = _OP_FORMATS.get(op)
        if fmt is None:
            instructions.append((inst_start, p, op, operands))
            continue

        for desc in fmt:
            if desc in _U30_POOL_DESCS or desc == 'u30':
                val, p = _ru30(code, p)
                operands.append((desc, val))
            elif desc == 'u8':
                val, p = _ru8(code, p)
                operands.append(('u8', val))
            elif desc == 's24':
                val, p = _rs24(code, p)
                operands.append(('s24', val))
            elif desc == 'switch':
                default_off, p = _rs24(code, p)
                operands.append(('switch_default_s24', default_off))
                case_count, p = _ru30(code, p)
                operands.append(('switch_count', case_count))
                for _ in range(case_count + 1):
                    case_off, p = _rs24(code, p)
                    operands.append(('switch_case_s24', case_off))
            elif desc == 'debug':
                dt, p = _ru8(code, p)
                operands.append(('debug_type', dt))
                idx, p = _ru30(code, p)
                operands.append(('debug_string', idx))
                reg, p = _ru8(code, p)
                operands.append(('debug_reg', reg))
                extra, p = _ru30(code, p)
                operands.append(('debug_extra', extra))

        instructions.append((inst_start, p, op, operands))

    return instructions


def _remap_operands(operands: list, pm: _PoolMapping) -> list:
    """Remap constant-pool references in an instruction's operands."""
    new_ops: list = []
    for desc, val in operands:
        if desc == 'multiname':
            new_ops.append((desc, pm.multinames.get(val, val)))
        elif desc in ('string', 'debug_string'):
            new_ops.append((desc, pm.strings.get(val, val)))
        elif desc == 'int':
            new_ops.append((desc, pm.integers.get(val, val)))
        elif desc == 'uint':
            new_ops.append((desc, pm.uintegers.get(val, val)))
        elif desc == 'double':
            new_ops.append((desc, pm.doubles.get(val, val)))
        elif desc == 'namespace':
            new_ops.append((desc, pm.namespaces.get(val, val)))
        elif desc == 'method':
            new_ops.append((desc, pm.methods.get(val, val)))
        elif desc == 'class':
            new_ops.append((desc, pm.classes.get(val, val)))
        else:
            new_ops.append((desc, val))
    return new_ops


def _translate_bytecode(code: bytes, pm: _PoolMapping) -> Tuple[bytes, Dict[int, int]]:
    """Translate AVM2 bytecode: remap pool references and fix branch offsets.

    Returns ``(translated_code, old_to_new_position_mapping)``.
    The position mapping maps every instruction-start byte offset (and the
    end-of-code offset) from the *input* code to the *output* code.
    """
    log.debug("_translate_bytecode: %d bytes", len(code))
    if not code:
        return b'', {0: 0}

    # Phase 1: Parse into instructions
    instructions = _parse_instructions(code)

    # Phase 2: Remap pool indices (branch offsets left unchanged for now)
    remapped = []
    for old_start, old_end, op, operands in instructions:
        new_ops = _remap_operands(operands, pm)
        remapped.append((old_start, old_end, op, new_ops))

    # Phase 3: Compute new byte positions and instruction sizes
    old_to_new: Dict[int, int] = {}
    new_pos = 0
    new_sizes: List[int] = []
    for i, (old_start, old_end, op, new_ops) in enumerate(remapped):
        old_to_new[old_start] = new_pos
        inst_size = 1  # opcode byte
        for desc, val in new_ops:
            inst_size += _operand_byte_size(desc, val)
        new_sizes.append(inst_size)
        new_pos += inst_size
    # Map end-of-code
    if instructions:
        old_to_new[instructions[-1][1]] = new_pos
    old_to_new[len(code)] = new_pos

    # Phase 4: Fix branch / switch offsets using position mapping
    fixed: List[list] = []
    new_pos = 0
    for i, ((old_start, old_end, op, new_ops),
            (orig_start, orig_end, _, _)) in enumerate(
            zip(remapped, instructions)):
        inst_size = new_sizes[i]
        new_inst_end = new_pos + inst_size
        fixed_ops: list = []
        for desc, val in new_ops:
            if desc == 's24':
                # Branch offset is relative to byte AFTER instruction
                old_target = orig_end + val
                new_target = old_to_new.get(old_target)
                if new_target is not None:
                    fixed_ops.append((desc, new_target - new_inst_end))
                else:
                    fixed_ops.append((desc, val))
            elif desc in ('switch_default_s24', 'switch_case_s24'):
                # lookupswitch offsets are relative to instruction START
                old_target = orig_start + val
                new_target = old_to_new.get(old_target)
                if new_target is not None:
                    fixed_ops.append((desc, new_target - new_pos))
                else:
                    fixed_ops.append((desc, val))
            else:
                fixed_ops.append((desc, val))
        fixed.append(fixed_ops)
        new_pos += inst_size

    # Phase 5: Serialize to bytes
    buf = bytearray()
    for i, (_, _, op, _) in enumerate(remapped):
        buf.append(op)
        for desc, val in fixed[i]:
            if desc in _U30_ALL_DESCS:
                _wu30(buf, val)
            elif desc in _U8_ALL_DESCS:
                _wu8(buf, val)
            elif desc in _S24_ALL_DESCS:
                _ws24(buf, val)

    return bytes(buf), old_to_new


# ═══════════════════════════════════════════════════════════════════════════
#  AVM2 stack / scope delta tables  (based on JPEXS FFDec InstructionDef.)
# ═══════════════════════════════════════════════════════════════════════════

# Fixed-effect instructions:  opcode → (stack_pop, stack_push)
# Instructions NOT in this table have *variable* effects that depend on
# the multiname kind or an arg-count operand.
_OP_STACK_FIXED: Dict[int, Tuple[int, int]] = {
    # ── No stack effect ──
    0x01: (0, 0),  # bkpt
    0x02: (0, 0),  # nop
    0x06: (0, 0),  # dxns
    0x08: (0, 0),  # kill
    0x09: (0, 0),  # label
    0x10: (0, 0),  # jump
    0x1D: (0, 0),  # popscope  (scope effect only)
    0x2B: (0, 0),  # swap
    0x47: (0, 0),  # returnvoid
    0x92: (0, 0),  # inclocal
    0x94: (0, 0),  # declocal
    0xC2: (0, 0),  # inclocal_i
    0xC3: (0, 0),  # declocal_i
    0xEF: (0, 0),  # debug
    0xF0: (0, 0),  # debugline
    0xF1: (0, 0),  # debugfile
    # ── Push 1 ──
    0x20: (0, 1),  # pushnull
    0x21: (0, 1),  # pushundefined
    0x24: (0, 1),  # pushbyte
    0x25: (0, 1),  # pushshort
    0x26: (0, 1),  # pushtrue
    0x27: (0, 1),  # pushfalse
    0x28: (0, 1),  # pushnan
    0x2A: (0, 1),  # dup
    0x2C: (0, 1),  # pushstring
    0x2D: (0, 1),  # pushint
    0x2E: (0, 1),  # pushuint
    0x2F: (0, 1),  # pushdouble
    0x31: (0, 1),  # pushnamespace
    0x32: (0, 1),  # hasnext2  (reads registers, pushes boolean)
    0x40: (0, 1),  # newfunction
    0x57: (0, 1),  # newactivation
    0x5A: (0, 1),  # newcatch
    0x60: (0, 1),  # getlex    (compile-time multiname only)
    0x62: (0, 1),  # getlocal
    0x64: (0, 1),  # getglobalscope
    0x65: (0, 1),  # getscopeobject
    0x6E: (0, 1),  # getglobalslot
    0xD0: (0, 1),  # getlocal_0
    0xD1: (0, 1),  # getlocal_1
    0xD2: (0, 1),  # getlocal_2
    0xD3: (0, 1),  # getlocal_3
    # ── Pop 1, Push 0 ──
    0x03: (1, 0),  # throw
    0x07: (1, 0),  # dxnslate
    0x11: (1, 0),  # iftrue
    0x12: (1, 0),  # iffalse
    0x29: (1, 0),  # pop
    0x30: (1, 0),  # pushscope   (operand → scope)
    0x1C: (1, 0),  # pushwith    (operand → scope)
    0x48: (1, 0),  # returnvalue
    0x63: (1, 0),  # setlocal
    0x6F: (1, 0),  # setglobalslot
    0xD4: (1, 0),  # setlocal_0
    0xD5: (1, 0),  # setlocal_1
    0xD6: (1, 0),  # setlocal_2
    0xD7: (1, 0),  # setlocal_3
    # ── Pop 1, Push 1  (unary / convert) ──
    0x35: (1, 1),  # li8
    0x36: (1, 1),  # li16
    0x37: (1, 1),  # li32
    0x38: (1, 1),  # lf32
    0x39: (1, 1),  # lf64
    0x50: (1, 1),  # sxi1
    0x51: (1, 1),  # sxi8
    0x52: (1, 1),  # sxi16
    0x58: (1, 1),  # newclass
    0x6C: (1, 1),  # getslot
    0x70: (1, 1),  # convert_s
    0x71: (1, 1),  # esc_xelem
    0x72: (1, 1),  # esc_xattr
    0x73: (1, 1),  # convert_i
    0x74: (1, 1),  # convert_u
    0x75: (1, 1),  # convert_d
    0x76: (1, 1),  # convert_b
    0x77: (1, 1),  # convert_o
    0x78: (1, 1),  # checkfilter
    0x80: (1, 1),  # coerce
    0x81: (1, 1),  # coerce_b
    0x82: (1, 1),  # coerce_a
    0x83: (1, 1),  # coerce_i
    0x84: (1, 1),  # coerce_d
    0x85: (1, 1),  # coerce_s
    0x86: (1, 1),  # astype
    0x88: (1, 1),  # coerce_u
    0x89: (1, 1),  # coerce_o
    0x90: (1, 1),  # negate
    0x91: (1, 1),  # increment
    0x93: (1, 1),  # decrement
    0x95: (1, 1),  # typeof
    0x96: (1, 1),  # not
    0x97: (1, 1),  # bitnot
    0xB2: (1, 1),  # istype
    0xC0: (1, 1),  # increment_i
    0xC1: (1, 1),  # decrement_i
    0xC4: (1, 1),  # negate_i
    # ── Pop 2, Push 0 ──
    0x0C: (2, 0),  # ifnlt
    0x0D: (2, 0),  # ifnle
    0x0E: (2, 0),  # ifngt
    0x0F: (2, 0),  # ifnge
    0x13: (2, 0),  # ifeq
    0x14: (2, 0),  # ifne
    0x15: (2, 0),  # iflt
    0x16: (2, 0),  # ifle
    0x17: (2, 0),  # ifgt
    0x18: (2, 0),  # ifge
    0x19: (2, 0),  # ifstricteq
    0x1A: (2, 0),  # ifstrictne
    0x3A: (2, 0),  # si8
    0x3B: (2, 0),  # si16
    0x3C: (2, 0),  # si32
    0x3D: (2, 0),  # sf32
    0x3E: (2, 0),  # sf64
    0x6D: (2, 0),  # setslot
    # ── Pop 2, Push 1  (binary ops) ──
    0x1E: (2, 1),  # nextname
    0x1F: (2, 1),  # hasnext
    0x23: (2, 1),  # nextvalue
    0x87: (2, 1),  # astypelate
    0xA0: (2, 1),  # add
    0xA1: (2, 1),  # subtract
    0xA2: (2, 1),  # multiply
    0xA3: (2, 1),  # divide
    0xA4: (2, 1),  # modulo
    0xA5: (2, 1),  # lshift
    0xA6: (2, 1),  # rshift
    0xA7: (2, 1),  # urshift
    0xA8: (2, 1),  # bitand
    0xA9: (2, 1),  # bitor
    0xAA: (2, 1),  # bitxor
    0xAB: (2, 1),  # equals
    0xAC: (2, 1),  # strictequals
    0xAD: (2, 1),  # lessthan
    0xAE: (2, 1),  # lessequals
    0xAF: (2, 1),  # greaterthan
    0xB0: (2, 1),  # greaterequals
    0xB1: (2, 1),  # instanceof
    0xB3: (2, 1),  # istypelate
    0xB4: (2, 1),  # in
    0xC5: (2, 1),  # add_i
    0xC6: (2, 1),  # subtract_i
    0xC7: (2, 1),  # multiply_i
    # ── lookupswitch (pops index) ──
    0x1B: (1, 0),
}

# Conditional branch opcodes (fall-through + branch target)
_CONDITIONAL_BRANCH_OPS = frozenset({
    0x0C, 0x0D, 0x0E, 0x0F,  # ifnlt / ifnle / ifngt / ifnge
    0x11, 0x12,                # iftrue / iffalse
    0x13, 0x14, 0x15, 0x16,   # ifeq / ifne / iflt / ifle
    0x17, 0x18, 0x19, 0x1A,   # ifgt / ifge / ifstricteq / ifstrictne
})


def _mn_stack_extras(mn_idx: int, abc) -> int:
    """Extra operand-stack pops required by a runtime multiname."""
    if abc is None or mn_idx <= 0 or mn_idx >= len(abc.multinames):
        return 0
    kind = abc.multinames[mn_idx][0]
    if kind in (0x0F, 0x10):   return 1   # RTQName(A)   — needsNs
    if kind in (0x11, 0x12):   return 2   # RTQNameL(A)  — needsName + needsNs
    if kind in (0x1B, 0x1C):   return 1   # MultinameL(A)— needsName
    return 0


def _variable_stack_delta(op: int, operands: list, abc) -> int:
    """Stack delta for instructions whose effect depends on multiname / argc."""
    mn_ext = 0
    argc = 0
    for desc, val in operands:
        if desc == 'multiname':
            mn_ext = _mn_stack_extras(val, abc)
        elif desc == 'u30':
            argc = val                                     # last u30 = argc
    # Property access (multiname-dependent)
    if op == 0x04:   return -mn_ext                        # getsuper
    if op == 0x05:   return -(2 + mn_ext)                  # setsuper
    if op == 0x59:   return -mn_ext                        # getdescendants
    if op == 0x5D:   return 1 - mn_ext                     # findpropstrict
    if op == 0x5E:   return 1 - mn_ext                     # findproperty
    if op == 0x5F:   return 1 - mn_ext                     # finddef
    if op == 0x61:   return -(2 + mn_ext)                  # setproperty
    if op == 0x66:   return -mn_ext                        # getproperty
    if op == 0x68:   return -(2 + mn_ext)                  # initproperty
    if op == 0x6A:   return -mn_ext                        # deleteproperty
    # Call instructions
    if op == 0x41:   return -(1 + argc)                    # call
    if op == 0x42:   return -argc                          # construct
    if op == 0x43:   return -argc                          # callmethod
    if op == 0x44:   return -argc                          # callstatic
    if op == 0x45:   return -(mn_ext + argc)               # callsuper
    if op == 0x46:   return -(mn_ext + argc)               # callproperty
    if op == 0x49:   return -(1 + argc)                    # constructsuper
    if op == 0x4A:   return -(mn_ext + argc)               # constructprop
    if op == 0x4C:   return -(mn_ext + argc)               # callproplex
    if op == 0x4E:   return -(1 + mn_ext + argc)           # callsupervoid
    if op == 0x4F:   return -(1 + mn_ext + argc)           # callpropvoid
    # Array / object construction
    if op == 0x53:   return -argc                          # applytype
    if op == 0x55:   return 1 - 2 * argc                   # newobject
    if op == 0x56:   return 1 - argc                       # newarray
    return 0                                                # unknown → no effect


# ═══════════════════════════════════════════════════════════════════════════
#  Bytecode metadata analyzer  (JPEXS walkCode / getStats approach)
# ═══════════════════════════════════════════════════════════════════════════

def _compute_body_metadata(
    code: bytes,
    exceptions: list,
    init_scope_depth: int,
    param_count: int,
    abc=None,
) -> Tuple[int, int, int, bool]:
    """Compute ``(max_stack, local_count, max_scope_depth, has_activation)``.

    Uses a **control-flow-aware** walk that follows branches (conditional,
    unconditional, lookupswitch) and walks exception handler entry points
    with ``stack=1`` (the caught exception on the operand stack), matching
    the behaviour of JPEXS FFDec's ``AVM2Code.walkCode()`` + ``getStats()``.

    Parameters
    ----------
    code : bytes
        The (translated) method bytecode.
    exceptions : list
        ``ExceptionInfo`` objects with byte offsets in *code*'s space.
    init_scope_depth : int
        Scope depth at which the method starts executing.
    param_count : int
        Number of declared parameters (minimum local_count).
    abc : ABCFile or None
        ABC whose constant pool *code* references.  Used to look up
        multiname kinds for runtime-multiname stack effects.
        ``None`` ⇒ all multinames treated as compile-time (QName).
    """
    if not code:
        return (0, param_count + 1, init_scope_depth, False)

    instructions = _parse_instructions(code)
    n = len(instructions)

    # Byte offset → instruction index
    off_to_idx: Dict[int, int] = {}
    for i, (start, _end, _op, _ops) in enumerate(instructions):
        off_to_idx[start] = i
    off_to_idx[len(code)] = n  # sentinel

    # Per-instruction state
    seen = [False] * n
    scopepos_after = [0] * n       # needed for exception-handler scope

    # Tracking accumulators (mutable from nested _walk)
    max_stack = [0]
    max_scope = [init_scope_depth]
    max_local = [param_count]      # registers 0 … param_count
    has_activation = [False]

    def _walk(start_pos: int, start_stack: int, start_scope: int):
        """Iterative control-flow walk from *start_pos*."""
        worklist = [(start_pos, start_stack, start_scope)]
        while worklist:
            pos, stack, scope = worklist.pop()
            while 0 <= pos < n:
                if seen[pos]:
                    break
                seen[pos] = True
                _start, _end, op, operands = instructions[pos]

                # Track peak BEFORE instruction
                if stack > max_stack[0]:
                    max_stack[0] = stack
                if scope > max_scope[0]:
                    max_scope[0] = scope

                # ── Register tracking ──
                if 0xD0 <= op <= 0xD3:
                    r = op - 0xD0
                    if r > max_local[0]: max_local[0] = r
                elif 0xD4 <= op <= 0xD7:
                    r = op - 0xD4
                    if r > max_local[0]: max_local[0] = r
                elif op in (0x62, 0x63, 0x92, 0x94, 0xC2, 0xC3):
                    for d, v in operands:
                        if d == 'u30' and v > max_local[0]:
                            max_local[0] = v
                elif op == 0x32:  # hasnext2
                    for d, v in operands:
                        if d == 'u30' and v > max_local[0]:
                            max_local[0] = v

                # ── Flags ──
                if op == 0x57:
                    has_activation[0] = True

                # ── Stack delta ──
                fixed = _OP_STACK_FIXED.get(op)
                if fixed is not None:
                    stack += fixed[1] - fixed[0]
                else:
                    stack += _variable_stack_delta(op, operands, abc)

                # ── Scope delta ──
                if op in (0x30, 0x1C):     # pushscope / pushwith
                    scope += 1
                elif op == 0x1D:           # popscope
                    scope -= 1

                # Track peak AFTER instruction
                if stack > max_stack[0]:
                    max_stack[0] = stack
                if scope > max_scope[0]:
                    max_scope[0] = scope

                scopepos_after[pos] = scope

                # ── Control flow ──
                if op == 0x10:             # jump (unconditional)
                    for d, v in operands:
                        if d == 's24':
                            tgt = off_to_idx.get(_end + v)
                            if tgt is not None:
                                pos = tgt
                                break
                    else:
                        break              # malformed — stop path
                    continue

                if op in _CONDITIONAL_BRANCH_OPS:
                    for d, v in operands:
                        if d == 's24':
                            tgt = off_to_idx.get(_end + v)
                            if tgt is not None:
                                worklist.append((tgt, stack, scope))
                    pos += 1
                    continue

                if op == 0x1B:             # lookupswitch
                    for d, v in operands:
                        if d in ('switch_default_s24', 'switch_case_s24'):
                            tgt = off_to_idx.get(_start + v)
                            if tgt is not None:
                                worklist.append((tgt, stack, scope))
                    break                  # end this path

                if op in (0x47, 0x48, 0x03):  # returnvoid/value, throw
                    break

                pos += 1

    # ── Main code walk ──
    _walk(0, 0, init_scope_depth)

    # ── Exception-handler walks ──
    # Each catch target starts with the caught exception on the operand
    # stack (stack = 1).  The scope depth is the depth *after* the
    # instruction immediately preceding the exception's ``to`` offset
    # (i.e. the scope at the end of the try block).
    for exc in (exceptions or []):
        target_idx = off_to_idx.get(exc.target)
        if target_idx is None:
            continue
        end_idx = off_to_idx.get(exc.to_pos)
        if end_idx is not None and end_idx > 0:
            exc_scope = scopepos_after[end_idx - 1]
        else:
            exc_scope = init_scope_depth
        _walk(target_idx, 1, exc_scope)

    return (
        max_stack[0],
        max_local[0] + 1,
        max_scope[0],
        has_activation[0],
    )


def _translate_exceptions(exceptions: list, pm: _PoolMapping,
                          pos_map: Optional[Dict[int, int]] = None) -> list:
    """Translate exception info pool references and remap byte offsets."""
    result = []
    for ei in exceptions:
        new_ei = ExceptionInfo()
        if pos_map is not None:
            new_ei.from_pos = pos_map.get(ei.from_pos, ei.from_pos)
            new_ei.to_pos = pos_map.get(ei.to_pos, ei.to_pos)
            new_ei.target = pos_map.get(ei.target, ei.target)
        else:
            new_ei.from_pos = ei.from_pos
            new_ei.to_pos = ei.to_pos
            new_ei.target = ei.target
        new_ei.exc_type = pm.multinames.get(ei.exc_type, ei.exc_type)
        new_ei.var_name = pm.multinames.get(ei.var_name, ei.var_name)
        result.append(new_ei)
    return result


def _translate_trait(t: TraitInfo, pm: _PoolMapping) -> TraitInfo:
    """Translate a trait's pool references."""
    new_t = TraitInfo()
    new_t.name_idx = pm.multinames.get(t.name_idx, t.name_idx)
    new_t.kind = t.kind
    new_t.attr = t.attr
    new_t.slot_id = t.slot_id
    new_t.disp_id = t.disp_id

    if t.kind in (TRAIT_Slot, TRAIT_Const):
        new_t.type_name = pm.multinames.get(t.type_name, t.type_name)
        new_t.vindex = t.vindex
        new_t.vkind = t.vkind
        # Translate vindex based on vkind
        if t.vindex:
            if t.vkind == 0x01:  # CONSTANT_Utf8
                new_t.vindex = pm.strings.get(t.vindex, t.vindex)
            elif t.vkind == 0x03:  # CONSTANT_Int
                new_t.vindex = pm.integers.get(t.vindex, t.vindex)
            elif t.vkind == 0x04:  # CONSTANT_UInt
                new_t.vindex = pm.uintegers.get(t.vindex, t.vindex)
            elif t.vkind == 0x06:  # CONSTANT_Double
                new_t.vindex = pm.doubles.get(t.vindex, t.vindex)
            elif t.vkind in (0x08, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x05):  # namespace kinds
                new_t.vindex = pm.namespaces.get(t.vindex, t.vindex)
    elif t.kind in (TRAIT_Method, TRAIT_Getter, TRAIT_Setter, TRAIT_Function):
        new_t.method_idx = pm.methods.get(t.method_idx, t.method_idx)
    elif t.kind == TRAIT_Class:
        new_t.class_idx = pm.classes.get(t.class_idx, t.class_idx)

    new_t.metadata = list(t.metadata)
    return new_t


# ═══════════════════════════════════════════════════════════════════════════
#  Inner-function (newfunction) transplant support
# ═══════════════════════════════════════════════════════════════════════════

def _find_newfunction_refs(code: bytes) -> List[int]:
    """Scan bytecode for ``newfunction`` opcodes, return referenced method indices."""
    refs: List[int] = []
    for _, _, op, operands in _parse_instructions(code):
        if op == 0x40:  # newfunction
            for desc, val in operands:
                if desc == 'method':
                    refs.append(val)
    return refs


def _remap_optional_value(vkind: int, val: int, pm: _PoolMapping) -> int:
    """Remap an optional parameter default value based on its value kind."""
    if vkind == 0x01:   # CONSTANT_Utf8
        return pm.strings.get(val, val)
    if vkind == 0x03:   # CONSTANT_Int
        return pm.integers.get(val, val)
    if vkind == 0x04:   # CONSTANT_UInt
        return pm.uintegers.get(val, val)
    if vkind == 0x06:   # CONSTANT_Double
        return pm.doubles.get(val, val)
    if vkind in (0x08, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x05):  # namespace kinds
        return pm.namespaces.get(val, val)
    return val  # true/false/null/undefined — no pool reference


# ═══════════════════════════════════════════════════════════════════════════
#  Inline constant patching — JPEXS-inspired direct bytecode editing
# ═══════════════════════════════════════════════════════════════════════════
#
# When a user makes a simple constant edit (e.g. changing a string literal)
# inside a method that has try-catch blocks, the decompiler may not
# reconstruct the exception handler structure.  If mxmlc recompiles
# without exception handlers, the resulting bytecode is broken.
#
# The fix: detect constant-only edits and apply them directly to the
# *original* bytecode, preserving all control flow, exception handlers,
# scope depths, and local variable counts.
#
# This is the same strategy JPEXS FFDec uses for "P-code editing" —
# modify existing bytecode operands in-place rather than replacing
# the method body wholesale.

import difflib as _difflib
import re as _re

# Operand descriptors that reference scalar constant pools
_CONST_POOL_DESCS = frozenset({'string', 'debug_string', 'int', 'uint', 'double'})


def _extract_string_edits_from_source(
    original_source: str, edited_source: str,
) -> Dict[str, str]:
    """Extract string-literal edits by comparing two AS3 source texts.

    Returns a dict mapping *old_string* → *new_string* for every
    double-quoted string literal that differs between the two sources.
    Only considers changes; strings that are identical are ignored.
    """
    # Regex for double-quoted string literals (simple — no embedded quotes)
    _STR_RE = _re.compile(r'"([^"\\]*(?:\\.[^"\\]*)*)"')

    orig_strings = _STR_RE.findall(original_source)
    edit_strings = _STR_RE.findall(edited_source)

    # Use SequenceMatcher to align the string lists
    matcher = _difflib.SequenceMatcher(None, orig_strings, edit_strings, autojunk=False)
    edits: Dict[str, str] = {}
    for tag, i1, i2, j1, j2 in matcher.get_opcodes():
        if tag == 'replace' and (i2 - i1) == (j2 - j1):
            for k in range(i2 - i1):
                old_s = orig_strings[i1 + k]
                new_s = edit_strings[j1 + k]
                if old_s != new_s:
                    edits[old_s] = new_s
    return edits


def _resolve_const_operand(abc: ABCFile, desc: str, val: int):
    """Resolve a constant-pool operand to its actual Python value."""
    if desc in ('string', 'debug_string'):
        return abc.strings[val] if 0 < val < len(abc.strings) else None
    if desc == 'int':
        return abc.integers[val] if 0 < val < len(abc.integers) else None
    if desc == 'uint':
        return abc.uintegers[val] if 0 < val < len(abc.uintegers) else None
    if desc == 'double':
        return abc.doubles[val] if 0 < val < len(abc.doubles) else None
    return None


def _ensure_pool_value(abc: ABCFile, desc: str, value) -> Optional[int]:
    """Find *value* in the appropriate constant pool, or add it.  Returns pool index."""
    import math as _math
    if desc in ('string', 'debug_string'):
        for i, s in enumerate(abc.strings):
            if s == value:
                return i
        idx = len(abc.strings)
        abc.strings.append(value)
        return idx
    if desc == 'int':
        for i, v in enumerate(abc.integers):
            if v == value:
                return i
        idx = len(abc.integers)
        abc.integers.append(value)
        return idx
    if desc == 'uint':
        for i, v in enumerate(abc.uintegers):
            if v == value:
                return i
        idx = len(abc.uintegers)
        abc.uintegers.append(value)
        return idx
    if desc == 'double':
        for i, v in enumerate(abc.doubles):
            if v == value or (_math.isnan(v) and _math.isnan(value)):
                return i
        idx = len(abc.doubles)
        abc.doubles.append(value)
        return idx
    return None


def _extract_const_sequence(abc: ABCFile, code: bytes) -> List[Tuple[str, object]]:
    """Extract an ordered list of ``(desc, resolved_value)`` for every
    constant-pool operand in *code* (pushstring, pushint, pushdouble …)."""
    result: List[Tuple[str, object]] = []
    for _, _, _, operands in _parse_instructions(code):
        for desc, val in operands:
            if desc in _CONST_POOL_DESCS:
                resolved = _resolve_const_operand(abc, desc, val)
                if resolved is not None:
                    result.append((desc, resolved))
    return result


def _compute_constant_edits(
    orig_abc: ABCFile, compiled_abc: ABCFile,
    orig_code: bytes, comp_code: bytes,
) -> Optional[Dict[Tuple[str, object], object]]:
    """Find constant-value edits between original and compiled bytecode.

    Returns a dict mapping ``(desc, old_value) → new_value`` for each
    constant that changed, or *None* if the constants can't be cleanly
    aligned (structural changes beyond simple constant edits).

    Two strategies are attempted:
      1. Strict sequence alignment (SequenceMatcher) — for bytecode with
         the same instruction ordering.
      2. Bag-diff fallback — compares multisets of constants.  This works
         even when mxmlc generates structurally different bytecode (different
         try-catch compilation, register allocation, etc.), as long as the
         user-visible constants differ only by the edits.
    """
    orig_seq = _extract_const_sequence(orig_abc, orig_code)
    comp_seq = _extract_const_sequence(compiled_abc, comp_code)

    # ── Strategy 1: strict sequence alignment ─────────────────────
    orig_keys = [f"{d}:{v}" for d, v in orig_seq]
    comp_keys = [f"{d}:{v}" for d, v in comp_seq]

    matcher = _difflib.SequenceMatcher(None, orig_keys, comp_keys, autojunk=False)
    edits: Dict[Tuple[str, object], object] = {}
    seq_ok = True

    for tag, i1, i2, j1, j2 in matcher.get_opcodes():
        if tag == 'equal':
            continue
        if tag == 'replace' and (i2 - i1) == (j2 - j1):
            for k in range(i2 - i1):
                old_desc, old_val = orig_seq[i1 + k]
                new_desc, new_val = comp_seq[j1 + k]
                if old_desc != new_desc:
                    seq_ok = False
                    break
                edits[(old_desc, old_val)] = new_val
            if not seq_ok:
                break
        elif tag in ('insert', 'delete', 'replace'):
            seq_ok = False
            break

    if seq_ok:
        return edits

    # ── Strategy 2: bag-diff fallback ─────────────────────────────
    # Compare multisets of constants.  Constants present in the original
    # but missing from compiled are "removals"; constants in compiled but
    # not original are "additions".  If we can pair them 1:1 by descriptor
    # type, those pairs are the edits.
    from collections import Counter
    orig_counter: Counter = Counter(orig_seq)
    comp_counter: Counter = Counter(comp_seq)

    removed = orig_counter - comp_counter   # in original, not in compiled
    added   = comp_counter - orig_counter   # in compiled, not in original

    # Group by descriptor type
    removed_by_type: Dict[str, list] = {}
    for (desc, val), count in removed.items():
        removed_by_type.setdefault(desc, []).extend([val] * count)

    added_by_type: Dict[str, list] = {}
    for (desc, val), count in added.items():
        added_by_type.setdefault(desc, []).extend([val] * count)

    # All descriptor types that have changes
    all_types = set(removed_by_type.keys()) | set(added_by_type.keys())

    bag_edits: Dict[Tuple[str, object], object] = {}
    for desc in all_types:
        rvals = removed_by_type.get(desc, [])
        avals = added_by_type.get(desc, [])
        if len(rvals) != len(avals):
            return None  # can't pair 1:1 → structural change
        # Pair removals with additions in order (stable sort by value)
        rvals.sort(key=str)
        avals.sort(key=str)
        for old_val, new_val in zip(rvals, avals):
            if old_val != new_val:
                bag_edits[(desc, old_val)] = new_val

    return bag_edits if bag_edits else {}


def _try_inline_constant_patch(
    orig: ABCFile, compiled: ABCFile,
    orig_body: MethodBody, comp_body: MethodBody,
    known_string_edits: Optional[Dict[str, str]] = None,
) -> Optional[MethodBody]:
    """Apply constant edits to the original bytecode, preserving structure.

    Inspired by JPEXS FFDec's P-code editing: modify existing bytecode
    operands in-place rather than replacing the entire method body.  This
    preserves exception handlers, branch offsets, scope depths, local
    variable count, and activation traits.

    Parameters
    ----------
    known_string_edits : dict, optional
        Mapping of old_string → new_string extracted from the source diff.
        When provided, these are used directly instead of trying to derive
        edits from the compiled bytecode (which can fail when mxmlc
        generates structurally different code).

    Returns a new :class:`MethodBody` on success, or *None* if inline
    patching isn't feasible (structural changes detected).
    """
    # ── Structural change check ─────────────────────────────────
    # Even when source-level string edits are known, we must verify
    # that the compiled code is structurally identical to the original
    # (same opcodes, same instruction count).  If the user added or
    # removed statements (e.g. logToFile calls), the bytecodes will
    # differ structurally and we must fall through to full transplant
    # to preserve the structural changes.
    orig_instrs = _parse_instructions(orig_body.code)
    comp_instrs = _parse_instructions(comp_body.code)

    if len(orig_instrs) != len(comp_instrs):
        # Different number of instructions — structural change
        return None

    for (_, _, orig_op, _), (_, _, comp_op, _) in zip(orig_instrs, comp_instrs):
        if orig_op != comp_op:
            # Different opcode — structural change
            return None

    # Determine constant edits
    if known_string_edits:
        # Build edits dict from source-level string edits
        edits: Dict[Tuple[str, object], object] = {}
        for old_s, new_s in known_string_edits.items():
            edits[('string', old_s)] = new_s
    else:
        edits_or_none = _compute_constant_edits(orig, compiled, orig_body.code, comp_body.code)
        if edits_or_none is None:
            return None  # can't determine edits cleanly
        edits = edits_or_none

    if not edits:
        # No constant changes detected — clone the original body as-is
        nb = MethodBody()
        nb.method_idx = orig_body.method_idx
        nb.max_stack = orig_body.max_stack
        nb.local_count = orig_body.local_count
        nb.init_scope_depth = orig_body.init_scope_depth
        nb.max_scope_depth = orig_body.max_scope_depth
        nb.code = orig_body.code
        nb.exceptions = list(orig_body.exceptions)
        nb.traits = list(orig_body.traits)
        return nb

    # ── Phase 1: Determine which operands to replace ──────────────
    # (orig_instrs already parsed above for the structural check)

    # Count how many times each edit key appears in the original bytecode.
    # Every occurrence is eligible for replacement.
    orig_seq = _extract_const_sequence(orig, orig_body.code)
    from collections import Counter as _Counter
    orig_counts = _Counter(orig_seq)
    remaining: Dict[Tuple[str, object], int] = {}
    for key in edits:
        remaining[key] = orig_counts.get(key, 0)

    replacements: Dict[Tuple[int, int], int] = {}  # (instr_idx, op_idx) → new pool index
    for instr_idx, (start, end, op, operands) in enumerate(orig_instrs):
        for op_idx, (desc, val) in enumerate(operands):
            if desc in _CONST_POOL_DESCS:
                resolved = _resolve_const_operand(orig, desc, val)
                if resolved is not None:
                    key = (desc, resolved)
                    if key in edits and remaining.get(key, 0) > 0:
                        new_val = edits[key]
                        new_pool_idx = _ensure_pool_value(orig, desc, new_val)
                        if new_pool_idx is not None:
                            replacements[(instr_idx, op_idx)] = new_pool_idx
                            remaining[key] -= 1

    # ── Phase 2: Rebuild bytecode with replaced operands ──────────
    patched_instrs = []
    for instr_idx, (start, end, op, operands) in enumerate(orig_instrs):
        new_ops = []
        for op_idx, (desc, val) in enumerate(operands):
            key = (instr_idx, op_idx)
            if key in replacements:
                new_ops.append((desc, replacements[key]))
            else:
                new_ops.append((desc, val))
        patched_instrs.append((start, end, op, new_ops))

    # Compute new byte positions (u30 encoding may change size)
    old_to_new: Dict[int, int] = {}
    new_pos = 0
    new_sizes: List[int] = []
    for instr_idx, (old_start, old_end, op, new_ops) in enumerate(patched_instrs):
        old_to_new[old_start] = new_pos
        inst_size = 1  # opcode byte
        for desc, val in new_ops:
            inst_size += _operand_byte_size(desc, val)
        new_sizes.append(inst_size)
        new_pos += inst_size
    if orig_instrs:
        old_to_new[orig_instrs[-1][1]] = new_pos
    old_to_new[len(orig_body.code)] = new_pos

    # ── Phase 3: Fix branch offsets ───────────────────────────────
    fixed_ops_list: List[list] = []
    cur = 0
    for instr_idx, ((_, _, op, new_ops),
                     (orig_start, orig_end, _, _)) in enumerate(
                     zip(patched_instrs, orig_instrs)):
        inst_size = new_sizes[instr_idx]
        new_inst_end = cur + inst_size
        fixed_ops: list = []
        for desc, val in new_ops:
            if desc == 's24':
                old_target = orig_end + val
                new_target = old_to_new.get(old_target)
                if new_target is not None:
                    fixed_ops.append((desc, new_target - new_inst_end))
                else:
                    fixed_ops.append((desc, val))
            elif desc in ('switch_default_s24', 'switch_case_s24'):
                old_target = orig_start + val
                new_target = old_to_new.get(old_target)
                if new_target is not None:
                    fixed_ops.append((desc, new_target - cur))
                else:
                    fixed_ops.append((desc, val))
            else:
                fixed_ops.append((desc, val))
        fixed_ops_list.append(fixed_ops)
        cur += inst_size

    # ── Phase 4: Serialize ────────────────────────────────────────
    buf = bytearray()
    for instr_idx, (_, _, op, _) in enumerate(patched_instrs):
        buf.append(op)
        for desc, val in fixed_ops_list[instr_idx]:
            if desc in _U30_ALL_DESCS:
                _wu30(buf, val)
            elif desc in _U8_ALL_DESCS:
                _wu8(buf, val)
            elif desc in _S24_ALL_DESCS:
                _ws24(buf, val)
    new_code = bytes(buf)

    # ── Phase 5: Remap exception handler byte offsets ─────────────
    new_exceptions = []
    for ei in orig_body.exceptions:
        new_ei = ExceptionInfo()
        new_ei.from_pos = old_to_new.get(ei.from_pos, ei.from_pos)
        new_ei.to_pos = old_to_new.get(ei.to_pos, ei.to_pos)
        new_ei.target = old_to_new.get(ei.target, ei.target)
        new_ei.exc_type = ei.exc_type
        new_ei.var_name = ei.var_name
        new_exceptions.append(new_ei)

    # ── Phase 6: Build patched method body ────────────────────────
    nb = MethodBody()
    nb.method_idx = orig_body.method_idx
    nb.max_stack = orig_body.max_stack
    nb.local_count = orig_body.local_count
    nb.init_scope_depth = orig_body.init_scope_depth
    nb.max_scope_depth = orig_body.max_scope_depth
    nb.code = new_code
    nb.exceptions = new_exceptions
    nb.traits = list(orig_body.traits)
    return nb


def _add_compiled_method_info(
    orig: ABCFile, compiled: ABCFile,
    comp_midx: int, pm: _PoolMapping,
) -> int:
    """Copy a compiled MethodInfo into *orig* ABC with remapped pool refs.

    Returns the new method index in *orig*.
    """
    comp_mi = compiled.methods[comp_midx]

    new_mi = MethodInfo()
    new_mi.param_count = comp_mi.param_count
    new_mi.return_type = pm.multinames.get(comp_mi.return_type, comp_mi.return_type)
    new_mi.param_types = [pm.multinames.get(pt, pt) for pt in comp_mi.param_types]
    new_mi.name_idx = pm.strings.get(comp_mi.name_idx, comp_mi.name_idx)
    new_mi.flags = comp_mi.flags

    if comp_mi.flags & METHOD_HasOptional:
        new_mi.optional_values = [
            (vk, _remap_optional_value(vk, val, pm))
            for vk, val in comp_mi.optional_values
        ]
    else:
        new_mi.optional_values = []

    if comp_mi.flags & METHOD_HasParamNames:
        new_mi.param_names = [pm.strings.get(pn, pn) for pn in comp_mi.param_names]
    else:
        new_mi.param_names = []

    new_idx = len(orig.methods)
    orig.methods.append(new_mi)
    pm.methods[comp_midx] = new_idx
    return new_idx


def _transplant_inner_functions(
    orig: ABCFile,
    compiled: ABCFile,
    comp_code: bytes,
    pm: _PoolMapping,
    scope_delta: int,
    visited: Optional[set] = None,
):
    """Recursively transplant inner functions discovered via ``newfunction``.

    Must be called **before** translating the enclosing method's bytecode
    so that all inner-method indices are present in *pm.methods*.
    """
    if visited is None:
        visited = set()

    for comp_inner_midx in _find_newfunction_refs(comp_code):
        if comp_inner_midx in pm.methods or comp_inner_midx in visited:
            continue
        visited.add(comp_inner_midx)

        # Copy MethodInfo into orig
        new_orig_midx = _add_compiled_method_info(
            orig, compiled, comp_inner_midx, pm,
        )

        comp_inner_body = compiled.method_bodies.get(comp_inner_midx)
        if comp_inner_body is None:
            continue

        # Depth-first: process any nested inner functions first
        _transplant_inner_functions(
            orig, compiled, comp_inner_body.code, pm, scope_delta, visited,
        )

        # Translate bytecode (all nested refs are now in pm.methods)
        new_code, pos_map = _translate_bytecode(comp_inner_body.code, pm)
        new_exceptions = _translate_exceptions(
            comp_inner_body.exceptions, pm, pos_map,
        )
        new_traits = [_translate_trait(t, pm) for t in comp_inner_body.traits]

        # Scope depth: apply the outer method's delta
        new_init = comp_inner_body.init_scope_depth + scope_delta

        # Compute metadata from translated bytecode (control-flow-aware)
        inner_param_count = orig.methods[new_orig_midx].param_count
        comp_max_stack, comp_locals, comp_max_scope, _ = _compute_body_metadata(
            new_code, new_exceptions, new_init, inner_param_count, orig,
        )

        new_body = MethodBody()
        new_body.method_idx = new_orig_midx
        new_body.max_stack = max(comp_max_stack, comp_inner_body.max_stack)
        new_body.local_count = max(comp_locals, comp_inner_body.local_count)
        new_body.init_scope_depth = new_init
        new_body.max_scope_depth = max(comp_max_scope, new_init)
        new_body.code = new_code
        new_body.exceptions = new_exceptions
        new_body.traits = new_traits

        orig.method_bodies[new_orig_midx] = new_body


# ═══════════════════════════════════════════════════════════════════════════
#  Main entry point: transplant one class's methods into the original ABC
# ═══════════════════════════════════════════════════════════════════════════

def _find_class_index(abc: ABCFile, full_name: str) -> Optional[int]:
    """Find a class instance index by its fully-qualified name."""
    for i, inst in enumerate(abc.instances):
        if abc.mn_full(inst.name_idx) == full_name:
            return i
    return None


def _collect_method_indices(abc: ABCFile, class_idx: int) -> List[int]:
    """Collect all method indices associated with a class.

    Returns method indices for:
    - Instance constructor (iinit)
    - Class initializer (cinit)
    - Instance method/getter/setter traits
    - Class (static) method/getter/setter traits
    """
    inst = abc.instances[class_idx]
    cls = abc.classes[class_idx]
    methods = set()

    methods.add(inst.iinit)
    methods.add(cls.cinit)

    for t in inst.traits:
        if t.kind in (TRAIT_Method, TRAIT_Getter, TRAIT_Setter, TRAIT_Function):
            methods.add(t.method_idx)

    for t in cls.traits:
        if t.kind in (TRAIT_Method, TRAIT_Getter, TRAIT_Setter, TRAIT_Function):
            methods.add(t.method_idx)

    return sorted(methods)


# ═══════════════════════════════════════════════════════════════════════════
#  Method-level source extraction — used to detect which methods changed
# ═══════════════════════════════════════════════════════════════════════════

_FUNC_RE = re.compile(
    r'(?:(?:public|private|protected|internal)\s+)?'
    r'(?:(?:static|override|final)\s+)*'
    r'function\s+'
    r'(?:(get|set)\s+)?'   # group 1: getter/setter
    r'(\w+)\s*\(',        # group 2: function name
)


def extract_method_texts(source: str, class_name: str = '') -> Dict[str, str]:
    """Extract per-method source text from AS3 class source.

    Returns a dict mapping method identifiers to their full source text.
    Keys are like:
      - ``'constructor'`` for the constructor
      - ``'method:initLogWindow'`` for a regular method
      - ``'get:Width'`` for a getter
      - ``'set:Width'`` for a setter
    """
    methods: Dict[str, str] = {}
    lines = source.split('\n')
    i = 0
    while i < len(lines):
        m = _FUNC_RE.search(lines[i])
        if not m:
            i += 1
            continue

        kind = m.group(1)   # 'get', 'set', or None
        name = m.group(2)

        # Collect the full function text including braces
        start_line = i
        brace_count = 0
        func_lines = []
        # Scan forward to find matching closing brace
        while i < len(lines):
            line = lines[i]
            func_lines.append(line)
            brace_count += line.count('{') - line.count('}')
            i += 1
            if brace_count <= 0 and len(func_lines) > 0 and '{' in ''.join(func_lines):
                break

        text = '\n'.join(func_lines)

        # Determine key
        if kind == 'get':
            key = f'get:{name}'
        elif kind == 'set':
            key = f'set:{name}'
        elif class_name and name == class_name:
            key = 'constructor'
        else:
            key = f'method:{name}'

        methods[key] = text

    return methods


def _detect_changed_methods(
    original_source: str,
    edited_source: str,
    class_name: str,
) -> Set[str]:
    """Compare original and edited class source, return set of changed method keys."""
    orig_methods = extract_method_texts(original_source, class_name)
    edit_methods = extract_method_texts(edited_source, class_name)

    changed: Set[str] = set()

    # Methods whose text differs
    for key in edit_methods:
        if key not in orig_methods or edit_methods[key] != orig_methods[key]:
            changed.add(key)

    # Methods that were removed (present in original, absent in edited)
    # — we don't need to transplant these, but flag for completeness
    for key in orig_methods:
        if key not in edit_methods:
            changed.add(key)

    return changed


def transplant_class(
    orig_abc_data: bytes,
    compiled_abc_data: bytes,
    class_full_name: str,
    changed_methods: Optional[Set[str]] = None,
    source_string_edits: Optional[Dict[str, str]] = None,
) -> bytes:
    """Transplant one class's compiled method bodies into the original ABC.

    Parameters
    ----------
    orig_abc_data : bytes
        Raw ABC data from the original SWF.
    compiled_abc_data : bytes
        Raw ABC data from mxmlc's compilation of all classes.
    class_full_name : str
        Fully-qualified class name, e.g. "com.mcleodgaming.ssf2.Main"
    changed_methods : set of str, optional
        If provided, only transplant methods whose keys are in this set.
        Keys follow the format returned by :func:`extract_method_texts`:
        ``'constructor'``, ``'method:foo'``, ``'get:bar'``, ``'set:bar'``.
        The special key ``'cinit'`` targets the static initializer.
        If *None*, **all** methods of the class are transplanted (legacy
        behavior — not recommended).
    source_string_edits : dict, optional
        Mapping of old_string → new_string extracted from the source diff.
        Passed through to inline constant patching so we don't need to
        derive edits from mxmlc's structurally-different bytecode.

    Returns
    -------
    bytes
        Patched ABC data with the specified class's method bodies replaced,
        all other class bytecode preserved byte-for-byte.
    """
    log.info("transplant_class: %s changed=%s", class_full_name, changed_methods)
    orig = ABCFile(orig_abc_data)
    compiled = ABCFile(compiled_abc_data)

    # Find the class in both ABCs
    orig_ci = _find_class_index(orig, class_full_name)
    if orig_ci is None:
        raise ValueError(f"Class '{class_full_name}' not found in original ABC")

    compiled_ci = _find_class_index(compiled, class_full_name)
    if compiled_ci is None:
        raise ValueError(f"Class '{class_full_name}' not found in compiled ABC")

    # ── PrivateNs identity mapping ──────────────────────────────────
    # AVM2 private namespaces are identity-based (pool index), not
    # name-based.  mxmlc creates descriptive PrivateNs names while the
    # original SWF may use anonymous PrivateNs("").  We must map them
    # to the ORIGINAL class's PrivateNs entries so that property
    # accesses resolve correctly in the verifier.
    ns_overrides = _build_private_ns_overrides(
        orig, compiled, orig_ci, compiled_ci,
    )
    if ns_overrides:
        import sys
        for comp_idx, orig_idx in ns_overrides.items():
            comp_kind, comp_name_idx = compiled.namespaces[comp_idx]
            comp_name = (compiled.strings[comp_name_idx]
                         if 0 < comp_name_idx < len(compiled.strings) else "")
            orig_kind, orig_name_idx = orig.namespaces[orig_idx]
            orig_name = (orig.strings[orig_name_idx]
                         if 0 < orig_name_idx < len(orig.strings) else "")
            print(
                f"[abc_patcher] PrivateNs override: "
                f"compiled ns[{comp_idx}] (\"{comp_name}\") "
                f"→ original ns[{orig_idx}] (\"{orig_name}\")",
                file=sys.stderr,
            )

    # Merge compiled ABC's constant pools into original
    pm = _merge_pools(orig, compiled, ns_overrides=ns_overrides)

    # Map compiled method indices → original method indices by trait name matching
    orig_inst = orig.instances[orig_ci]
    orig_cls = orig.classes[orig_ci]
    comp_inst = compiled.instances[compiled_ci]
    comp_cls = compiled.classes[compiled_ci]

    # ── Build source-key → (comp_midx, orig_midx) mapping ──────────
    # Source keys match extract_method_texts() format:
    #   'constructor', 'cinit', 'method:foo', 'get:bar', 'set:bar'
    _KIND_PREFIX = {
        TRAIT_Method: 'method', TRAIT_Getter: 'get',
        TRAIT_Setter: 'set', TRAIT_Function: 'method',
    }
    _METHOD_TRAIT_KINDS = frozenset(_KIND_PREFIX.keys())
    _SLOT_TRAIT_KINDS = frozenset((TRAIT_Slot, TRAIT_Const))

    def _match_key(abc_obj, name_idx):
        """Normalized multiname key for matching traits across ABCs.

        PrivateNs namespaces have empty names in original SWFs but
        mxmlc produces descriptive names like ``"com.pkg:Class"``.
        This function maps PrivateNs-qualified names to a canonical
        ``"<private>.SimpleName"`` so they always match.
        """
        if name_idx <= 0 or name_idx >= len(abc_obj.multinames):
            return '*'
        kind, data = abc_obj.multinames[name_idx]
        if kind in (MN_QName, MN_QNameA) and data:
            ns_idx = data[0]
            if 0 < ns_idx < len(abc_obj.namespaces):
                ns_kind = abc_obj.namespaces[ns_idx][0]
                if ns_kind == NS_PrivateNs:
                    return f'<private>.{abc_obj.mn_name(name_idx)}'
        return abc_obj.mn_full(name_idx)

    def _trait_maps(traits, abc_obj):
        """Return (full_key_map, source_key_map) for traits."""
        full_map = {}   # (_match_key, kind) → method_idx
        src_map = {}    # source_key → method_idx
        for t in traits:
            if t.kind in _KIND_PREFIX:
                fk = (_match_key(abc_obj, t.name_idx), t.kind)
                full_map[fk] = t.method_idx
                short_name = abc_obj.mn_name(t.name_idx)
                prefix = _KIND_PREFIX[t.kind]
                src_map[f'{prefix}:{short_name}'] = t.method_idx
        return full_map, src_map

    orig_inst_full, orig_inst_src = _trait_maps(orig_inst.traits, orig)
    orig_cls_full, orig_cls_src = _trait_maps(orig_cls.traits, orig)
    comp_inst_full, comp_inst_src = _trait_maps(comp_inst.traits, compiled)
    comp_cls_full, comp_cls_src = _trait_maps(comp_cls.traits, compiled)

    # Map compiled method idx → (original method idx, source_key)
    method_pairs: Dict[int, Tuple[int, str]] = {}

    # Constructor: iinit → iinit
    method_pairs[comp_inst.iinit] = (orig_inst.iinit, 'constructor')
    pm.methods[comp_inst.iinit] = orig_inst.iinit

    # Static initializer: cinit → cinit
    method_pairs[comp_cls.cinit] = (orig_cls.cinit, 'cinit')
    pm.methods[comp_cls.cinit] = orig_cls.cinit

    # Instance methods
    for fk, comp_midx in comp_inst_full.items():
        if fk in orig_inst_full:
            orig_midx = orig_inst_full[fk]
            pm.methods[comp_midx] = orig_midx
            # Recover the source key
            short_name = compiled.mn_name(
                next(t.name_idx for t in comp_inst.traits
                     if t.kind in _KIND_PREFIX and t.method_idx == comp_midx)
            )
            prefix = _KIND_PREFIX[fk[1]]
            method_pairs[comp_midx] = (orig_midx, f'{prefix}:{short_name}')

    # Class (static) methods
    for fk, comp_midx in comp_cls_full.items():
        if fk in orig_cls_full:
            orig_midx = orig_cls_full[fk]
            pm.methods[comp_midx] = orig_midx
            short_name = compiled.mn_name(
                next(t.name_idx for t in comp_cls.traits
                     if t.kind in _KIND_PREFIX and t.method_idx == comp_midx)
            )
            prefix = _KIND_PREFIX[fk[1]]
            method_pairs[comp_midx] = (orig_midx, f'{prefix}:{short_name}')

    # ══════════════════════════════════════════════════════════════════
    #  Feature: Update class inheritance / interfaces
    # ══════════════════════════════════════════════════════════════════
    # If mxmlc's compiled class has a different superclass or interface
    # list, update the original to match.
    comp_super_full = compiled.mn_full(comp_inst.super_idx)
    orig_super_full = orig.mn_full(orig_inst.super_idx)
    if comp_super_full != orig_super_full and comp_inst.super_idx != 0:
        new_super = pm.multinames.get(comp_inst.super_idx, comp_inst.super_idx)
        import sys
        print(
            f"[abc_patcher] Superclass changed: "
            f"{orig_super_full} → {comp_super_full}",
            file=sys.stderr,
        )
        orig_inst.super_idx = new_super

    comp_iface_names = {compiled.mn_full(i) for i in comp_inst.interfaces}
    orig_iface_names = {orig.mn_full(i) for i in orig_inst.interfaces}
    if comp_iface_names != orig_iface_names:
        import sys
        added = comp_iface_names - orig_iface_names
        removed = orig_iface_names - comp_iface_names
        if added:
            print(f"[abc_patcher] Interfaces added: {added}", file=sys.stderr)
        if removed:
            print(f"[abc_patcher] Interfaces removed: {removed}", file=sys.stderr)
        orig_inst.interfaces = [
            pm.multinames.get(i, i) for i in comp_inst.interfaces
        ]

    # Update ProtectedNs if the compiled class declares one
    if (comp_inst.flags & INSTANCE_ProtectedNs) and comp_inst.protected_ns:
        mapped_prot = pm.namespaces.get(
            comp_inst.protected_ns, comp_inst.protected_ns
        )
        if orig_inst.protected_ns != mapped_prot:
            orig_inst.protected_ns = mapped_prot

    # ══════════════════════════════════════════════════════════════════
    #  Feature: Update method signatures
    # ══════════════════════════════════════════════════════════════════
    # When a matched method has different param_count, return_type, or
    # param_types, overwrite the original MethodInfo to match compiled.
    for comp_midx, (orig_midx, src_key) in method_pairs.items():
        comp_mi = compiled.methods[comp_midx]
        orig_mi = orig.methods[orig_midx]
        sig_changed = False
        if comp_mi.param_count != orig_mi.param_count:
            sig_changed = True
        elif comp_mi.return_type != 0 and orig_mi.return_type != 0:
            comp_ret = compiled.mn_full(comp_mi.return_type)
            orig_ret = orig.mn_full(orig_mi.return_type)
            if comp_ret != orig_ret:
                sig_changed = True
        if sig_changed:
            import sys
            print(
                f"[abc_patcher] Signature changed for '{src_key}': "
                f"params {orig_mi.param_count}→{comp_mi.param_count}",
                file=sys.stderr,
            )
            orig_mi.param_count = comp_mi.param_count
            orig_mi.return_type = pm.multinames.get(
                comp_mi.return_type, comp_mi.return_type
            )
            orig_mi.param_types = [
                pm.multinames.get(pt, pt) for pt in comp_mi.param_types
            ]
            orig_mi.flags = comp_mi.flags
            if comp_mi.flags & METHOD_HasOptional:
                orig_mi.optional_values = [
                    (vk, _remap_optional_value(vk, val, pm))
                    for vk, val in comp_mi.optional_values
                ]
            else:
                orig_mi.optional_values = []
            if comp_mi.flags & METHOD_HasParamNames:
                orig_mi.param_names = [
                    pm.strings.get(pn, pn) for pn in comp_mi.param_names
                ]
            else:
                orig_mi.param_names = []

    # ══════════════════════════════════════════════════════════════════
    #  Feature: Add new methods (compiled has, original doesn't)
    # ══════════════════════════════════════════════════════════════════
    new_methods_added = 0

    def _add_new_method_trait(comp_trait, is_static: bool, comp_abc, target_traits):
        """Add a brand-new method trait from compiled to original."""
        nonlocal new_methods_added
        comp_midx = comp_trait.method_idx
        src_key_name = comp_abc.mn_name(comp_trait.name_idx)
        prefix = _KIND_PREFIX.get(comp_trait.kind, 'method')
        src_key = f'{prefix}:{src_key_name}'

        # Skip if not in changed_methods filter
        if changed_methods is not None and src_key not in changed_methods:
            return

        import sys
        where = "static" if is_static else "instance"
        print(
            f"[abc_patcher] Adding new {where} method: {src_key}",
            file=sys.stderr,
        )

        # Create MethodInfo in orig
        new_orig_midx = _add_compiled_method_info(
            orig, compiled, comp_midx, pm,
        )

        # Create translated trait
        new_trait = _translate_trait(comp_trait, pm)
        new_trait.method_idx = new_orig_midx
        target_traits.append(new_trait)

        # Add to method_pairs so the body gets transplanted
        method_pairs[comp_midx] = (new_orig_midx, src_key)
        new_methods_added += 1

    # Check instance methods
    for fk, comp_midx in comp_inst_full.items():
        if fk not in orig_inst_full:
            comp_trait = next(
                t for t in comp_inst.traits
                if t.kind in _METHOD_TRAIT_KINDS and t.method_idx == comp_midx
            )
            _add_new_method_trait(comp_trait, False, compiled, orig_inst.traits)

    # Check class (static) methods
    for fk, comp_midx in comp_cls_full.items():
        if fk not in orig_cls_full:
            comp_trait = next(
                t for t in comp_cls.traits
                if t.kind in _METHOD_TRAIT_KINDS and t.method_idx == comp_midx
            )
            _add_new_method_trait(comp_trait, True, compiled, orig_cls.traits)

    # ══════════════════════════════════════════════════════════════════
    #  Feature: Add new properties (slots / consts)
    # ══════════════════════════════════════════════════════════════════
    new_slots_added = 0

    def _get_slot_traits(traits, abc_obj):
        """Return {_match_key: TraitInfo} for slot/const traits."""
        result = {}
        for t in traits:
            if t.kind in _SLOT_TRAIT_KINDS:
                result[_match_key(abc_obj, t.name_idx)] = t
        return result

    orig_inst_slots = _get_slot_traits(orig_inst.traits, orig)
    orig_cls_slots = _get_slot_traits(orig_cls.traits, orig)
    comp_inst_slots = _get_slot_traits(comp_inst.traits, compiled)
    comp_cls_slots = _get_slot_traits(comp_cls.traits, compiled)

    def _add_new_slot_trait(comp_trait, is_static, target_traits):
        nonlocal new_slots_added
        import sys
        name = compiled.mn_full(comp_trait.name_idx)
        where = "static" if is_static else "instance"
        kind_word = "const" if comp_trait.kind == TRAIT_Const else "slot"
        print(
            f"[abc_patcher] Adding new {where} {kind_word}: {name}",
            file=sys.stderr,
        )
        new_trait = _translate_trait(comp_trait, pm)
        # Assign next available slot_id (0 = auto)
        new_trait.slot_id = 0
        target_traits.append(new_trait)
        new_slots_added += 1

    for name, comp_trait in comp_inst_slots.items():
        if name not in orig_inst_slots:
            _add_new_slot_trait(comp_trait, False, orig_inst.traits)

    for name, comp_trait in comp_cls_slots.items():
        if name not in orig_cls_slots:
            _add_new_slot_trait(comp_trait, True, orig_cls.traits)

    # ══════════════════════════════════════════════════════════════════
    #  Feature: Remove deleted methods / properties
    # ══════════════════════════════════════════════════════════════════
    # If a trait exists in original but NOT in compiled, and the user
    # has changed_methods=None (full transplant), remove it.
    removed_count = 0
    if changed_methods is None:  # Only on full transplant
        # Build sets of compiled trait names for fast lookup
        comp_inst_names = set()
        for t in comp_inst.traits:
            comp_inst_names.add(_match_key(compiled, t.name_idx))
        comp_cls_names = set()
        for t in comp_cls.traits:
            comp_cls_names.add(_match_key(compiled, t.name_idx))

        def _remove_deleted_traits(orig_traits, comp_names, where):
            nonlocal removed_count
            to_keep = []
            for t in orig_traits:
                name = _match_key(orig, t.name_idx)
                if name in comp_names:
                    to_keep.append(t)
                else:
                    import sys
                    kind_str = {
                        TRAIT_Slot: 'slot', TRAIT_Const: 'const',
                        TRAIT_Method: 'method', TRAIT_Getter: 'getter',
                        TRAIT_Setter: 'setter', TRAIT_Function: 'function',
                        TRAIT_Class: 'class',
                    }.get(t.kind, f'kind={t.kind}')
                    print(
                        f"[abc_patcher] Removing {where} {kind_str}: {name}",
                        file=sys.stderr,
                    )
                    removed_count += 1
            return to_keep

        orig_inst.traits = _remove_deleted_traits(
            orig_inst.traits, comp_inst_names, "instance"
        )
        orig_cls.traits = _remove_deleted_traits(
            orig_cls.traits, comp_cls_names, "static"
        )

    # ══════════════════════════════════════════════════════════════════
    #  Feature: Warn on unmatched methods
    # ══════════════════════════════════════════════════════════════════
    for fk, comp_midx in comp_inst_full.items():
        if fk not in orig_inst_full and comp_midx not in pm.methods:
            import sys
            print(
                f"[abc_patcher] WARNING: unmatched instance method "
                f"{fk[0]} (kind={fk[1]}) — not transplanted",
                file=sys.stderr,
            )
    for fk, comp_midx in comp_cls_full.items():
        if fk not in orig_cls_full and comp_midx not in pm.methods:
            import sys
            print(
                f"[abc_patcher] WARNING: unmatched static method "
                f"{fk[0]} (kind={fk[1]}) — not transplanted",
                file=sys.stderr,
            )

    # ── Transplant only changed method bodies ────────────────────────
    transplanted = 0
    skipped = 0
    visited_inner: set = set()

    for comp_midx, (orig_midx, src_key) in method_pairs.items():
        # Filter: if changed_methods is given, only transplant those
        if changed_methods is not None and src_key not in changed_methods:
            skipped += 1
            continue

        comp_body = compiled.method_bodies.get(comp_midx)
        if comp_body is None:
            continue

        orig_body = orig.method_bodies.get(orig_midx)

        # ── Strategy selection ──────────────────────────────────────
        # Always try inline constant patching first when we have an
        # original body.  This preserves the *exact* original bytecode
        # structure (scope depths, local counts, max_stack, exception
        # handlers, branch offsets) — which is critical because the
        # AVM2 verifier is strict about these values and mxmlc may
        # generate slightly different metadata even for semantically
        # identical code.
        if orig_body is not None:
            inline_result = _try_inline_constant_patch(
                orig, compiled, orig_body, comp_body,
                known_string_edits=source_string_edits,
            )
            if inline_result is not None:
                orig.method_bodies[orig_midx] = inline_result
                transplanted += 1
                continue
            # Inline patching failed (structural changes beyond simple
            # constant edits).  If the original has exception handlers
            # that the compiled version lost, warn loudly.
            if (len(orig_body.exceptions) > 0
                    and len(comp_body.exceptions) == 0):
                import sys
                print(
                    f"[abc_patcher] WARNING: method '{src_key}' "
                    f"(#{orig_midx}) lost {len(orig_body.exceptions)} "
                    f"exception handler(s) — inline patching failed, "
                    f"using full transplant",
                    file=sys.stderr,
                )

        # ── Full transplant (existing behaviour) ────────────────────
        # Scope depth adjustment: mxmlc compiles standalone classes
        # with init_scope_depth=0, but the original class sits within
        # a scope chain (global + package + base classes + class) so
        # its methods have init_scope_depth > 0.
        if orig_body is not None:
            scope_delta = orig_body.init_scope_depth - comp_body.init_scope_depth
            new_init = orig_body.init_scope_depth
        else:
            scope_delta = 0
            new_init = comp_body.init_scope_depth

        # ── Transplant inner functions (newfunction) ────────────────
        # Must happen BEFORE translating bytecode so that all inner
        # method indices are mapped in pm.methods.
        _transplant_inner_functions(
            orig, compiled, comp_body.code, pm, scope_delta, visited_inner,
        )

        # Translate the bytecode (inner function refs now in pm.methods)
        new_code, pos_map = _translate_bytecode(comp_body.code, pm)

        # Translate exceptions (with position-mapped byte offsets)
        new_exceptions = _translate_exceptions(comp_body.exceptions, pm, pos_map)

        # ── Fix exception var_name / exc_type ───────────────────────
        # mxmlc may generate catch variable multinames in the wrong
        # namespace (e.g. PackageInternalNs instead of PackageNamespace).
        # When the original body has matching exception handlers,
        # preserve the original var_name and exc_type which are already
        # correct for the target SWF's pool.
        if (orig_body is not None
                and len(orig_body.exceptions) == len(new_exceptions)):
            for new_ei, orig_ei in zip(new_exceptions, orig_body.exceptions):
                new_ei.var_name = orig_ei.var_name
                new_ei.exc_type = orig_ei.exc_type

        # Translate body-level traits
        new_traits = [_translate_trait(t, pm) for t in comp_body.traits]

        # ── Compute metadata from translated bytecode ───────────────
        # Use a control-flow-aware walk (following JPEXS walkCode) to
        # determine accurate max_stack, local_count, and max_scope_depth.
        # Take MAX with the compiled body's values as safety margin.
        param_count = orig.methods[orig_midx].param_count
        computed_max_stack, computed_locals, computed_max_scope, computed_has_act = (
            _compute_body_metadata(
                new_code, new_exceptions, new_init, param_count, orig,
            )
        )

        safe_max_stack = max(computed_max_stack, comp_body.max_stack)
        safe_locals = max(computed_locals, comp_body.local_count)
        safe_max_scope = max(computed_max_scope, new_init)

        # Build the replacement method body
        new_body = MethodBody()
        new_body.method_idx = orig_midx
        new_body.max_stack = safe_max_stack
        new_body.local_count = safe_locals
        new_body.init_scope_depth = new_init
        new_body.max_scope_depth = safe_max_scope
        new_body.code = new_code
        new_body.exceptions = new_exceptions
        new_body.traits = new_traits

        orig.method_bodies[orig_midx] = new_body
        transplanted += 1

    if transplanted == 0 and changed_methods:
        raise ValueError(
            f"No method bodies transplanted for '{class_full_name}' "
            f"(changed_methods={changed_methods}, "
            f"available keys={set(sk for _, sk in method_pairs.values())})"
        )

    # Summary logging
    import sys
    summary_parts = [f"transplanted={transplanted}"]
    if skipped:
        summary_parts.append(f"skipped={skipped}")
    if new_methods_added:
        summary_parts.append(f"new_methods={new_methods_added}")
    if new_slots_added:
        summary_parts.append(f"new_slots={new_slots_added}")
    if removed_count:
        summary_parts.append(f"removed={removed_count}")
    print(
        f"[abc_patcher] {class_full_name}: {', '.join(summary_parts)}",
        file=sys.stderr,
    )

    # Serialize the patched original ABC
    return serialize_abc(orig)
