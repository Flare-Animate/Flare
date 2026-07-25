"""AVM2 ABC bytecode parser — binary readers, constants, data types, and ABCFile."""

from __future__ import annotations

import logging
import struct
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Tuple

log = logging.getLogger(__name__)

__all__ = [
    # Binary readers
    '_ru8', '_ru16', '_rs24', '_ru30', '_ru32', '_rs32', '_rd64',
    # Namespace kind constants
    'NS_Namespace', 'NS_PackageNamespace', 'NS_PackageInternalNs',
    'NS_ProtectedNs', 'NS_ExplicitNs', 'NS_StaticProtectedNs', 'NS_PrivateNs',
    # Multiname kind constants
    'MN_QName', 'MN_QNameA', 'MN_RTQName', 'MN_RTQNameA', 'MN_RTQNameL',
    'MN_RTQNameLA', 'MN_Multiname', 'MN_MultinameA', 'MN_MultinameL',
    'MN_MultinameLA', 'MN_TypeName',
    # Trait kinds
    'TRAIT_Slot', 'TRAIT_Method', 'TRAIT_Getter', 'TRAIT_Setter',
    'TRAIT_Class', 'TRAIT_Function', 'TRAIT_Const',
    # Instance flags
    'INSTANCE_Sealed', 'INSTANCE_Final', 'INSTANCE_Interface', 'INSTANCE_ProtectedNs',
    # Method flags
    'METHOD_NeedArguments', 'METHOD_NeedActivation', 'METHOD_NeedRest',
    'METHOD_HasOptional', 'METHOD_SetDXNS', 'METHOD_HasParamNames',
    # Constant kinds
    'CONSTANT_Int', 'CONSTANT_UInt', 'CONSTANT_Double', 'CONSTANT_Utf8',
    'CONSTANT_True', 'CONSTANT_False', 'CONSTANT_Null', 'CONSTANT_Undefined',
    'CONSTANT_Namespace', 'CONSTANT_PackageNamespace', 'CONSTANT_PackageInternalNs',
    'CONSTANT_ProtectedNamespace', 'CONSTANT_ExplicitNamespace',
    'CONSTANT_StaticProtectedNs', 'CONSTANT_PrivateNs',
    # Data classes
    'MethodInfo', 'TraitInfo', 'InstanceInfo', 'ClassInfo',
    'ScriptInfo', 'MethodBody', 'ExceptionInfo',
    # Main parser
    'ABCFile',
]


# ═══════════════════════════════════════════════════════════════════════════
#  Low-level readers (same as abc_parser.py but standalone)
# ═══════════════════════════════════════════════════════════════════════════

def _ru8(d: bytes, p: int) -> Tuple[int, int]:
    return d[p], p + 1

def _ru16(d: bytes, p: int) -> Tuple[int, int]:
    return struct.unpack_from('<H', d, p)[0], p + 2

def _rs24(d: bytes, p: int) -> Tuple[int, int]:
    v = d[p] | (d[p+1] << 8) | (d[p+2] << 16)
    if v & 0x800000:
        v -= 0x1000000
    return v, p + 3

def _ru30(d: bytes, p: int) -> Tuple[int, int]:
    result = shift = 0
    for _ in range(5):
        b = d[p]; p += 1
        result |= (b & 0x7F) << shift
        if not (b & 0x80):
            break
        shift += 7
    return result & 0x3FFFFFFF, p

def _ru32(d: bytes, p: int) -> Tuple[int, int]:
    """Read unsigned 32-bit variable-length encoded integer (full 32 bits)."""
    result = shift = 0
    for _ in range(5):
        b = d[p]; p += 1
        result |= (b & 0x7F) << shift
        if not (b & 0x80):
            break
        shift += 7
    return result & 0xFFFFFFFF, p

def _rs32(d: bytes, p: int) -> Tuple[int, int]:
    result = shift = 0
    b = 0
    for _ in range(5):
        b = d[p]; p += 1
        result |= (b & 0x7F) << shift
        if not (b & 0x80):
            break
        shift += 7
    if shift < 32 and (b & 0x40):
        result |= -(1 << (shift + 6))
    return result, p

def _rd64(d: bytes, p: int) -> Tuple[float, int]:
    return struct.unpack_from('<d', d, p)[0], p + 8


# ═══════════════════════════════════════════════════════════════════════════
#  Namespace kind constants
# ═══════════════════════════════════════════════════════════════════════════
NS_Namespace         = 0x08
NS_PackageNamespace  = 0x16
NS_PackageInternalNs = 0x17
NS_ProtectedNs       = 0x18
NS_ExplicitNs        = 0x19
NS_StaticProtectedNs = 0x1A
NS_PrivateNs         = 0x05

# Multiname kind constants
MN_QName       = 0x07
MN_QNameA      = 0x0D
MN_RTQName     = 0x0F
MN_RTQNameA    = 0x10
MN_RTQNameL    = 0x11
MN_RTQNameLA   = 0x12
MN_Multiname   = 0x09
MN_MultinameA  = 0x0E
MN_MultinameL  = 0x1B
MN_MultinameLA = 0x1C
MN_TypeName    = 0x1D

# Trait kinds
TRAIT_Slot     = 0
TRAIT_Method   = 1
TRAIT_Getter   = 2
TRAIT_Setter   = 3
TRAIT_Class    = 4
TRAIT_Function = 5
TRAIT_Const    = 6

# Instance flags
INSTANCE_Sealed        = 0x01
INSTANCE_Final         = 0x02
INSTANCE_Interface     = 0x04
INSTANCE_ProtectedNs   = 0x08

# Method flags
METHOD_NeedArguments  = 0x01
METHOD_NeedActivation = 0x02
METHOD_NeedRest       = 0x04
METHOD_HasOptional    = 0x08
METHOD_SetDXNS        = 0x40
METHOD_HasParamNames  = 0x80

# Constant kinds for default values
CONSTANT_Int       = 0x03
CONSTANT_UInt      = 0x04
CONSTANT_Double    = 0x06
CONSTANT_Utf8      = 0x01
CONSTANT_True      = 0x0B
CONSTANT_False     = 0x0A
CONSTANT_Null      = 0x0C
CONSTANT_Undefined = 0x00
CONSTANT_Namespace          = 0x08
CONSTANT_PackageNamespace   = 0x16
CONSTANT_PackageInternalNs  = 0x17
CONSTANT_ProtectedNamespace = 0x18
CONSTANT_ExplicitNamespace  = 0x19
CONSTANT_StaticProtectedNs  = 0x1A
CONSTANT_PrivateNs          = 0x05


# ═══════════════════════════════════════════════════════════════════════════
#  ABC File Parser (full)
# ═══════════════════════════════════════════════════════════════════════════

@dataclass
class MethodInfo:
    param_count: int = 0
    return_type: int = 0
    param_types: List[int] = field(default_factory=list)
    name_idx: int = 0
    flags: int = 0
    optional_values: List[Tuple[int, int]] = field(default_factory=list)  # (vkind, vindex)
    param_names: List[int] = field(default_factory=list)

@dataclass
class TraitInfo:
    name_idx: int = 0
    kind: int = 0
    attr: int = 0
    # Slot/Const
    slot_id: int = 0
    type_name: int = 0
    vindex: int = 0
    vkind: int = 0
    # Method/Getter/Setter/Function
    disp_id: int = 0
    method_idx: int = 0
    # Class
    class_idx: int = 0
    # Metadata
    metadata: List[int] = field(default_factory=list)

@dataclass
class InstanceInfo:
    name_idx: int = 0
    super_idx: int = 0
    flags: int = 0
    protected_ns: int = 0
    interfaces: List[int] = field(default_factory=list)
    iinit: int = 0
    traits: List[TraitInfo] = field(default_factory=list)

@dataclass
class ClassInfo:
    cinit: int = 0
    traits: List[TraitInfo] = field(default_factory=list)

@dataclass
class ScriptInfo:
    sinit: int = 0
    traits: List[TraitInfo] = field(default_factory=list)

@dataclass
class MethodBody:
    method_idx: int = 0
    max_stack: int = 0
    local_count: int = 0
    init_scope_depth: int = 0
    max_scope_depth: int = 0
    code: bytes = b''
    exceptions: list = field(default_factory=list)
    traits: List[TraitInfo] = field(default_factory=list)

@dataclass
class ExceptionInfo:
    from_pos: int = 0
    to_pos: int = 0
    target: int = 0
    exc_type: int = 0
    var_name: int = 0


class ABCFile:
    """Complete AVM2 ABC bytecode parser."""

    def __init__(self, data: bytes):
        log.debug("ABCFile.__init__: parsing %d bytes", len(data))
        self.data = data
        p = 0
        self.minor, p = _ru16(data, p)
        self.major, p = _ru16(data, p)

        # Name resolution caches (issue #23): keyed by index, populated lazily
        self._cache_ns_name: Dict[int, str] = {}
        self._cache_mn_name: Dict[int, str] = {}
        self._cache_mn_full: Dict[int, str] = {}
        self._cache_mn_ns: Dict[int, str] = {}
        self._cache_type_name: Dict[int, str] = {}

        # Constant pool
        p = self._parse_cpool(data, p)

        # Methods
        p = self._parse_methods(data, p)

        # Metadata
        p = self._parse_metadata(data, p)

        # Instances + Classes
        p = self._parse_instances_and_classes(data, p)

        # Scripts
        p = self._parse_scripts(data, p)

        # Method bodies
        self._parse_method_bodies(data, p)

    # ── Constant Pool ─────────────────────────────────────────────────
    def _parse_cpool(self, d: bytes, p: int) -> int:
        # Track original pool counts for faithful round-trip serialization
        self._raw_pool_counts: Dict[str, int] = {}

        # integers (encoded as u32, reinterpreted as signed 32-bit)
        count, p = _ru30(d, p)
        self._raw_pool_counts['integers'] = count
        self.integers: List[int] = [0]
        for _ in range(max(0, count - 1)):
            v, p = _ru32(d, p)
            if v >= 0x80000000:
                v -= 0x100000000
            self.integers.append(v)

        # uintegers
        count, p = _ru30(d, p)
        self._raw_pool_counts['uintegers'] = count
        self.uintegers: List[int] = [0]
        for _ in range(max(0, count - 1)):
            v, p = _ru32(d, p)
            self.uintegers.append(v)

        # doubles
        count, p = _ru30(d, p)
        self._raw_pool_counts['doubles'] = count
        self.doubles: List[float] = [0.0]
        for _ in range(max(0, count - 1)):
            v, p = _rd64(d, p)
            self.doubles.append(v)

        # strings
        count, p = _ru30(d, p)
        self._raw_pool_counts['strings'] = count
        self.strings: List[str] = ['']
        for _ in range(max(0, count - 1)):
            slen, p = _ru30(d, p)
            self.strings.append(d[p:p + slen].decode('utf-8', errors='replace'))
            p += slen

        # namespaces (kind, name_str_idx)
        count, p = _ru30(d, p)
        self._raw_pool_counts['namespaces'] = count
        self.namespaces: List[Tuple[int, int]] = [(0, 0)]
        for _ in range(max(0, count - 1)):
            kind, p = _ru8(d, p)
            name_idx, p = _ru30(d, p)
            self.namespaces.append((kind, name_idx))

        # namespace sets
        count, p = _ru30(d, p)
        self._raw_pool_counts['ns_sets'] = count
        self.ns_sets: List[List[int]] = [[]]
        for _ in range(max(0, count - 1)):
            ns_count, p = _ru30(d, p)
            ns_set: List[int] = []
            for _ in range(ns_count):
                ns_idx, p = _ru30(d, p)
                ns_set.append(ns_idx)
            self.ns_sets.append(ns_set)

        # multinames
        count, p = _ru30(d, p)
        self._raw_pool_counts['multinames'] = count
        self.multinames: List[Tuple[int, Optional[tuple]]] = [(0, None)]
        for _ in range(max(0, count - 1)):
            kind, p = _ru8(d, p)
            if kind in (MN_QName, MN_QNameA):
                ns, p = _ru30(d, p)
                name, p = _ru30(d, p)
                self.multinames.append((kind, (ns, name)))
            elif kind in (MN_RTQName, MN_RTQNameA):
                name, p = _ru30(d, p)
                self.multinames.append((kind, (name,)))
            elif kind in (MN_RTQNameL, MN_RTQNameLA):
                self.multinames.append((kind, ()))
            elif kind in (MN_Multiname, MN_MultinameA):
                name, p = _ru30(d, p)
                ns_set, p = _ru30(d, p)
                self.multinames.append((kind, (name, ns_set)))
            elif kind in (MN_MultinameL, MN_MultinameLA):
                ns_set, p = _ru30(d, p)
                self.multinames.append((kind, (ns_set,)))
            elif kind == MN_TypeName:
                qn, p = _ru30(d, p)
                pc, p = _ru30(d, p)
                params = []
                for _ in range(pc):
                    pm, p = _ru30(d, p)
                    params.append(pm)
                self.multinames.append((kind, (qn, tuple(params))))
            else:
                raise ValueError(f"Unknown multiname kind {kind:#x} at offset {p}")
        return p

    # ── Method Info ───────────────────────────────────────────────────
    def _parse_methods(self, d: bytes, p: int) -> int:
        count, p = _ru30(d, p)
        self.methods: List[MethodInfo] = []
        for _ in range(count):
            m = MethodInfo()
            m.param_count, p = _ru30(d, p)
            m.return_type, p = _ru30(d, p)
            m.param_types = []
            for _ in range(m.param_count):
                pt, p = _ru30(d, p)
                m.param_types.append(pt)
            m.name_idx, p = _ru30(d, p)
            m.flags, p = _ru8(d, p)
            if m.flags & METHOD_HasOptional:
                opt_count, p = _ru30(d, p)
                m.optional_values = []
                for _ in range(opt_count):
                    val, p = _ru30(d, p)
                    kind, p = _ru8(d, p)
                    m.optional_values.append((kind, val))
            if m.flags & METHOD_HasParamNames:
                m.param_names = []
                for _ in range(m.param_count):
                    nm, p = _ru30(d, p)
                    m.param_names.append(nm)
            self.methods.append(m)
        return p

    # ── Metadata ──────────────────────────────────────────────────────
    def _parse_metadata(self, d: bytes, p: int) -> int:
        count, p = _ru30(d, p)
        self.metadata_entries: List[Tuple[int, List[Tuple[int, int]]]] = []
        for _ in range(count):
            name_idx, p = _ru30(d, p)
            item_count, p = _ru30(d, p)
            items = []
            for _ in range(item_count):
                k, p = _ru30(d, p)
                v, p = _ru30(d, p)
                items.append((k, v))
            self.metadata_entries.append((name_idx, items))
        return p

    # ── Trait parsing ─────────────────────────────────────────────────
    def _parse_trait(self, d: bytes, p: int) -> Tuple[TraitInfo, int]:
        t = TraitInfo()
        t.name_idx, p = _ru30(d, p)
        kb, p = _ru8(d, p)
        t.kind = kb & 0x0F
        t.attr = (kb >> 4) & 0x0F

        if t.kind in (TRAIT_Slot, TRAIT_Const):
            t.slot_id, p = _ru30(d, p)
            t.type_name, p = _ru30(d, p)
            t.vindex, p = _ru30(d, p)
            if t.vindex:
                t.vkind, p = _ru8(d, p)
        elif t.kind in (TRAIT_Method, TRAIT_Getter, TRAIT_Setter):
            t.disp_id, p = _ru30(d, p)
            t.method_idx, p = _ru30(d, p)
        elif t.kind == TRAIT_Class:
            t.slot_id, p = _ru30(d, p)
            t.class_idx, p = _ru30(d, p)
        elif t.kind == TRAIT_Function:
            t.slot_id, p = _ru30(d, p)
            t.method_idx, p = _ru30(d, p)

        if t.attr & 0x04:  # ATTR_Metadata
            mc, p = _ru30(d, p)
            t.metadata = []
            for _ in range(mc):
                mi, p = _ru30(d, p)
                t.metadata.append(mi)
        return t, p

    # ── Instances + Classes ───────────────────────────────────────────
    def _parse_instances_and_classes(self, d: bytes, p: int) -> int:
        count, p = _ru30(d, p)
        self.instances: List[InstanceInfo] = []
        for _ in range(count):
            inst = InstanceInfo()
            inst.name_idx, p = _ru30(d, p)
            inst.super_idx, p = _ru30(d, p)
            inst.flags, p = _ru8(d, p)
            if inst.flags & INSTANCE_ProtectedNs:
                inst.protected_ns, p = _ru30(d, p)
            intf_count, p = _ru30(d, p)
            inst.interfaces = []
            for _ in range(intf_count):
                ii, p = _ru30(d, p)
                inst.interfaces.append(ii)
            inst.iinit, p = _ru30(d, p)
            trait_count, p = _ru30(d, p)
            inst.traits = []
            for _ in range(trait_count):
                tr, p = self._parse_trait(d, p)
                inst.traits.append(tr)
            self.instances.append(inst)

        self.classes: List[ClassInfo] = []
        for _ in range(count):
            ci = ClassInfo()
            ci.cinit, p = _ru30(d, p)
            trait_count, p = _ru30(d, p)
            ci.traits = []
            for _ in range(trait_count):
                tr, p = self._parse_trait(d, p)
                ci.traits.append(tr)
            self.classes.append(ci)
        return p

    # ── Scripts ───────────────────────────────────────────────────────
    def _parse_scripts(self, d: bytes, p: int) -> int:
        count, p = _ru30(d, p)
        self.scripts: List[ScriptInfo] = []
        for _ in range(count):
            si = ScriptInfo()
            si.sinit, p = _ru30(d, p)
            tc, p = _ru30(d, p)
            si.traits = []
            for _ in range(tc):
                tr, p = self._parse_trait(d, p)
                si.traits.append(tr)
            self.scripts.append(si)
        return p

    # ── Method Bodies ─────────────────────────────────────────────────
    def _parse_method_bodies(self, d: bytes, p: int) -> int:
        count, p = _ru30(d, p)
        self.method_bodies: Dict[int, MethodBody] = {}
        self._method_body_order: List[int] = []  # preserve original ordering
        for _ in range(count):
            mb = MethodBody()
            mb.method_idx, p = _ru30(d, p)
            mb.max_stack, p = _ru30(d, p)
            mb.local_count, p = _ru30(d, p)
            mb.init_scope_depth, p = _ru30(d, p)
            mb.max_scope_depth, p = _ru30(d, p)
            code_len, p = _ru30(d, p)
            mb.code = d[p:p + code_len]
            p += code_len
            exc_count, p = _ru30(d, p)
            mb.exceptions = []
            for _ in range(exc_count):
                ei = ExceptionInfo()
                ei.from_pos, p = _ru30(d, p)
                ei.to_pos, p = _ru30(d, p)
                ei.target, p = _ru30(d, p)
                ei.exc_type, p = _ru30(d, p)
                ei.var_name, p = _ru30(d, p)
                mb.exceptions.append(ei)
            tc, p = _ru30(d, p)
            mb.traits = []
            for _ in range(tc):
                tr, p = self._parse_trait(d, p)
                mb.traits.append(tr)
            self.method_bodies[mb.method_idx] = mb
            self._method_body_order.append(mb.method_idx)
        return p

    # ── Name helpers ──────────────────────────────────────────────────
    def ns_name(self, ns_idx: int) -> str:
        cached = self._cache_ns_name.get(ns_idx)
        if cached is not None:
            return cached
        if ns_idx <= 0 or ns_idx >= len(self.namespaces):
            self._cache_ns_name[ns_idx] = ''
            return ''
        _, name_idx = self.namespaces[ns_idx]
        result = self.strings[name_idx] if 0 < name_idx < len(self.strings) else ''
        self._cache_ns_name[ns_idx] = result
        return result

    def ns_kind(self, ns_idx: int) -> int:
        if ns_idx <= 0 or ns_idx >= len(self.namespaces):
            return 0
        return self.namespaces[ns_idx][0]

    def mn_is_attr(self, idx: int) -> bool:
        """Return True if the multiname is an attribute access (@)."""
        if idx <= 0 or idx >= len(self.multinames):
            return False
        kind, _ = self.multinames[idx]
        return kind in (MN_QNameA, MN_RTQNameA, MN_MultinameA)

    def mn_name(self, idx: int) -> str:
        """Simple unqualified name of a multiname."""
        cached = self._cache_mn_name.get(idx)
        if cached is not None:
            return cached
        if idx <= 0 or idx >= len(self.multinames):
            self._cache_mn_name[idx] = '*'
            return '*'
        kind, data = self.multinames[idx]
        if kind in (MN_QName, MN_QNameA) and data:
            result = self.strings[data[1]] if data[1] < len(self.strings) else ''
        elif kind in (MN_Multiname, MN_MultinameA) and data:
            result = self.strings[data[0]] if data[0] < len(self.strings) else ''
        elif kind in (MN_RTQName, MN_RTQNameA) and data:
            result = self.strings[data[0]] if data[0] < len(self.strings) else ''
        elif kind == MN_TypeName and data:
            result = self.mn_name(data[0])
        else:
            result = '*'
        self._cache_mn_name[idx] = result
        return result

    def mn_full(self, idx: int) -> str:
        """Fully qualified name like flash.display.MovieClip."""
        cached = self._cache_mn_full.get(idx)
        if cached is not None:
            return cached
        if idx <= 0 or idx >= len(self.multinames):
            self._cache_mn_full[idx] = '*'
            return '*'
        kind, data = self.multinames[idx]
        if kind in (MN_QName, MN_QNameA) and data:
            ns = self.ns_name(data[0])
            name = self.strings[data[1]] if data[1] < len(self.strings) else ''
            result = f'{ns}.{name}' if ns else name
        elif kind in (MN_Multiname, MN_MultinameA) and data:
            result = self.strings[data[0]] if data[0] < len(self.strings) else '?'
        elif kind in (MN_RTQName, MN_RTQNameA) and data:
            result = self.strings[data[0]] if data[0] < len(self.strings) else '?'
        elif kind == MN_TypeName and data:
            base = self.mn_full(data[0])
            params = ', '.join(self.mn_full(p) for p in data[1])
            result = f'{base}.<{params}>'
        else:
            result = '*'
        self._cache_mn_full[idx] = result
        return result

    def mn_ns(self, idx: int) -> str:
        """Namespace part of a multiname."""
        cached = self._cache_mn_ns.get(idx)
        if cached is not None:
            return cached
        if idx <= 0 or idx >= len(self.multinames):
            self._cache_mn_ns[idx] = ''
            return ''
        kind, data = self.multinames[idx]
        if kind in (MN_QName, MN_QNameA) and data:
            result = self.ns_name(data[0])
        else:
            result = ''
        self._cache_mn_ns[idx] = result
        return result

    def mn_ns_kind(self, idx: int) -> int:
        """Namespace kind of a multiname."""
        if idx <= 0 or idx >= len(self.multinames):
            return 0
        kind, data = self.multinames[idx]
        if kind in (MN_QName, MN_QNameA) and data:
            return self.ns_kind(data[0])
        return 0

    def mn_needs_rt_name(self, idx: int) -> bool:
        """Check if this multiname needs a runtime name from stack."""
        if idx <= 0 or idx >= len(self.multinames):
            return False
        kind, _ = self.multinames[idx]
        return kind in (MN_MultinameL, MN_MultinameLA, MN_RTQNameL, MN_RTQNameLA)

    def mn_needs_rt_ns(self, idx: int) -> bool:
        """Check if this multiname needs a runtime namespace from stack."""
        if idx <= 0 or idx >= len(self.multinames):
            return False
        kind, _ = self.multinames[idx]
        return kind in (MN_RTQName, MN_RTQNameA, MN_RTQNameL, MN_RTQNameLA)

    def type_name(self, mn_idx: int) -> str:
        """Get a type name for display (e.g. 'int', 'String', 'Vector.<int>', '*')."""
        cached = self._cache_type_name.get(mn_idx)
        if cached is not None:
            return cached
        if mn_idx == 0:
            self._cache_type_name[mn_idx] = '*'
            return '*'
        if mn_idx < len(self.multinames):
            kind, data = self.multinames[mn_idx]
            if kind == MN_TypeName and data:
                base = self.type_name(data[0])
                params = ', '.join(self.type_name(p) for p in data[1])
                result = f'{base}.<{params}>'
                self._cache_type_name[mn_idx] = result
                return result
        name = self.mn_name(mn_idx)
        result = name if name else '*'
        self._cache_type_name[mn_idx] = result
        return result

    def default_value_str(self, vkind: int, vindex: int) -> str:
        """Format a default parameter value."""
        if vkind == CONSTANT_Int:
            return str(self.integers[vindex] if vindex < len(self.integers) else 0)
        if vkind == CONSTANT_UInt:
            return str(self.uintegers[vindex] if vindex < len(self.uintegers) else 0)
        if vkind == CONSTANT_Double:
            v = self.doubles[vindex] if vindex < len(self.doubles) else 0.0
            import math
            if math.isnan(v):
                return 'NaN'
            if math.isinf(v):
                return 'Infinity' if v > 0 else '-Infinity'
            if v == int(v) and abs(v) < 1e15:
                return f'{int(v)}.0'
            return f'{v:.15g}'
        if vkind == CONSTANT_Utf8:
            s = self.strings[vindex] if vindex < len(self.strings) else ''
            return f'"{s}"'
        if vkind == CONSTANT_True:
            return 'true'
        if vkind == CONSTANT_False:
            return 'false'
        if vkind == CONSTANT_Null:
            return 'null'
        if vkind == CONSTANT_Undefined or (vkind == 0 and vindex == 0):
            return 'undefined'
        if vkind in (CONSTANT_Namespace, CONSTANT_PackageNamespace,
                     CONSTANT_PackageInternalNs, CONSTANT_ProtectedNamespace,
                     CONSTANT_ExplicitNamespace, CONSTANT_StaticProtectedNs,
                     CONSTANT_PrivateNs):
            return self.ns_name(vindex) or 'null'
        return 'undefined'

