"""ABC Editor — high-level API for complex AS3 bytecode modifications.

Provides :class:`ABCEditor` for mutating an ``ABCFile`` and
:class:`Assembler` for building AVM2 bytecode from symbolic instructions.

This module builds on the low-level infrastructure in :mod:`abc_patcher`
(serializer, pool merger, bytecode translator) and provides a clean API for:

  * Constant-pool creation: ``ensure_string``, ``ensure_int``, ``ensure_namespace``,
    ``ensure_multiname``, ``ensure_qname``, etc.
  * Method creation: ``add_method`` with automatic MethodInfo + MethodBody wiring
  * Trait construction: ``make_slot_trait``, ``make_method_trait``, ``make_getter_trait``, etc.
  * Class mutation: ``add_instance_trait``, ``add_class_trait``, ``remove_instance_trait``, etc.
  * Bytecode assembly: symbolic opcode names + label-based branching
  * Disassembly for debugging: ``disassemble(abc, code) → str``

Example
-------
::

    editor = ABCEditor(raw_abc_bytes)

    # Pool operations
    str_idx = editor.ensure_string("hello")
    ns_idx  = editor.ensure_namespace(NS_Package, "com.example")
    mn_idx  = editor.ensure_qname(ns_idx, str_idx)

    # Bytecode assembly
    asm = Assembler()
    asm.emit('getlocal_0')
    asm.emit('pushscope')
    asm.emit('pushstring', str_idx)
    asm.emit('returnvalue')
    code, max_stack = asm.assemble()

    # Add a method
    method_idx = editor.add_method(
        return_type=mn_idx,
        param_types=[],
        code=code,
        max_stack=max_stack,
        local_count=1,
        init_scope_depth=10,
        max_scope_depth=11,
    )

    # Attach it to a class
    trait = editor.make_method_trait(mn_idx, method_idx)
    editor.add_instance_trait("com.example.MyClass", trait)

    # Serialize and get the modified ABC bytes
    patched = editor.serialize()
"""

from __future__ import annotations

import logging
import struct
from typing import Dict, List, Optional, Set, Tuple, Union

log = logging.getLogger(__name__)

from .abc_parser import (
    ABCFile, MethodInfo, MethodBody, ExceptionInfo, TraitInfo,
    MN_QName, MN_TypeName,
    TRAIT_Slot, TRAIT_Method, TRAIT_Getter, TRAIT_Setter,
    TRAIT_Class, TRAIT_Function, TRAIT_Const,
    METHOD_HasOptional, METHOD_HasParamNames,
)
from .abc_patcher import (
    serialize_abc,
    _parse_instructions, _OP_FORMATS,
    _wu8, _wu30, _ws24,
    _U30_ALL_DESCS, _U8_ALL_DESCS, _S24_ALL_DESCS,
    _operand_byte_size,
)

__all__ = [
    'ABCEditor', 'Assembler', 'disassemble',
    # Re-export namespace kind constants for convenience
    'NS_Namespace', 'NS_Package', 'NS_PackageInternal',
    'NS_Protected', 'NS_Explicit', 'NS_StaticProtected', 'NS_Private',
]


# ═══════════════════════════════════════════════════════════════════════════
#  AVM2 Namespace kind constants
# ═══════════════════════════════════════════════════════════════════════════

NS_Namespace        = 0x08
NS_Package          = 0x16
NS_PackageInternal  = 0x17
NS_Protected        = 0x18
NS_Explicit         = 0x19
NS_StaticProtected  = 0x1A
NS_Private          = 0x05


# ═══════════════════════════════════════════════════════════════════════════
#  AVM2 Constant-value kind constants (for default values / trait values)
# ═══════════════════════════════════════════════════════════════════════════




# ═══════════════════════════════════════════════════════════════════════════
#  Opcode name ↔ byte mapping
# ═══════════════════════════════════════════════════════════════════════════

#: Maps AVM2 opcode name (lowercase) → opcode byte.
OPCODE_BY_NAME: Dict[str, int] = {
    'bkpt': 0x01, 'nop': 0x02, 'throw': 0x03,
    'getsuper': 0x04, 'setsuper': 0x05, 'dxns': 0x06, 'dxnslate': 0x07,
    'kill': 0x08, 'label': 0x09,
    'ifnlt': 0x0C, 'ifnle': 0x0D, 'ifngt': 0x0E, 'ifnge': 0x0F,
    'jump': 0x10, 'iftrue': 0x11, 'iffalse': 0x12,
    'ifeq': 0x13, 'ifne': 0x14, 'iflt': 0x15, 'ifle': 0x16,
    'ifgt': 0x17, 'ifge': 0x18,
    'ifstricteq': 0x19, 'ifstrictne': 0x1A,
    'lookupswitch': 0x1B,
    'pushwith': 0x1C, 'popscope': 0x1D,
    'nextname': 0x1E, 'hasnext': 0x1F,
    'pushnull': 0x20, 'pushundefined': 0x21,
    'nextvalue': 0x23,
    'pushbyte': 0x24, 'pushshort': 0x25,
    'pushtrue': 0x26, 'pushfalse': 0x27, 'pushnan': 0x28,
    'pop': 0x29, 'dup': 0x2A, 'swap': 0x2B,
    'pushstring': 0x2C, 'pushint': 0x2D, 'pushuint': 0x2E,
    'pushdouble': 0x2F,
    'pushscope': 0x30, 'pushnamespace': 0x31,
    'hasnext2': 0x32,
    'li8': 0x35, 'li16': 0x36, 'li32': 0x37,
    'lf32': 0x38, 'lf64': 0x39,
    'si8': 0x3A, 'si16': 0x3B, 'si32': 0x3C,
    'sf32': 0x3D, 'sf64': 0x3E,
    'newfunction': 0x40, 'call': 0x41, 'construct': 0x42,
    'callmethod': 0x43, 'callstatic': 0x44,
    'callsuper': 0x45, 'callproperty': 0x46,
    'returnvoid': 0x47, 'returnvalue': 0x48,
    'constructsuper': 0x49, 'constructprop': 0x4A,
    'callproplex': 0x4C,
    'callsupervoid': 0x4E, 'callpropvoid': 0x4F,
    'sxi1': 0x50, 'sxi8': 0x51, 'sxi16': 0x52,
    'applytype': 0x53,
    'newobject': 0x55, 'newarray': 0x56, 'newactivation': 0x57,
    'newclass': 0x58, 'getdescendants': 0x59, 'newcatch': 0x5A,
    'findpropstrict': 0x5D, 'findproperty': 0x5E, 'finddef': 0x5F,
    'getlex': 0x60, 'setproperty': 0x61,
    'getlocal': 0x62, 'setlocal': 0x63,
    'getglobalscope': 0x64, 'getscopeobject': 0x65,
    'getproperty': 0x66,
    'initproperty': 0x68, 'deleteproperty': 0x6A,
    'getslot': 0x6C, 'setslot': 0x6D,
    'getglobalslot': 0x6E, 'setglobalslot': 0x6F,
    'convert_s': 0x70, 'esc_xelem': 0x71, 'esc_xattr': 0x72,
    'convert_i': 0x73, 'convert_u': 0x74, 'convert_d': 0x75,
    'convert_b': 0x76, 'convert_o': 0x77,
    'checkfilter': 0x78,
    'coerce': 0x80,
    'coerce_b': 0x81, 'coerce_a': 0x82, 'coerce_i': 0x83,
    'coerce_d': 0x84, 'coerce_s': 0x85,
    'astype': 0x86, 'astypelate': 0x87,
    'coerce_u': 0x88, 'coerce_o': 0x89,
    'negate': 0x90, 'increment': 0x91, 'inclocal': 0x92,
    'decrement': 0x93, 'declocal': 0x94,
    'typeof': 0x95, 'not': 0x96, 'bitnot': 0x97,
    'add': 0xA0, 'subtract': 0xA1, 'multiply': 0xA2,
    'divide': 0xA3, 'modulo': 0xA4,
    'lshift': 0xA5, 'rshift': 0xA6, 'urshift': 0xA7,
    'bitand': 0xA8, 'bitor': 0xA9, 'bitxor': 0xAA,
    'equals': 0xAB, 'strictequals': 0xAC,
    'lessthan': 0xAD, 'lessequals': 0xAE,
    'greaterthan': 0xAF, 'greaterequals': 0xB0,
    'instanceof': 0xB1, 'istype': 0xB2, 'istypelate': 0xB3,
    'in': 0xB4,
    'increment_i': 0xC0, 'decrement_i': 0xC1,
    'inclocal_i': 0xC2, 'declocal_i': 0xC3,
    'negate_i': 0xC4, 'add_i': 0xC5,
    'subtract_i': 0xC6, 'multiply_i': 0xC7,
    'getlocal_0': 0xD0, 'getlocal_1': 0xD1,
    'getlocal_2': 0xD2, 'getlocal_3': 0xD3,
    'setlocal_0': 0xD4, 'setlocal_1': 0xD5,
    'setlocal_2': 0xD6, 'setlocal_3': 0xD7,
    'debug': 0xEF, 'debugline': 0xF0, 'debugfile': 0xF1,
}

#: Reverse mapping: opcode byte → name.
NAME_BY_OPCODE: Dict[int, str] = {v: k for k, v in OPCODE_BY_NAME.items()}


# ═══════════════════════════════════════════════════════════════════════════
#  Assembler — build AVM2 bytecode from symbolic instructions
# ═══════════════════════════════════════════════════════════════════════════

class Assembler:
    """AVM2 bytecode assembler with label-based branching.

    Usage::

        asm = Assembler()
        asm.emit('getlocal_0')
        asm.emit('pushscope')
        asm.label('loop')
        asm.emit('getlocal_1')
        asm.emit('pushint', 10)     # pool index
        asm.emit('iflt', 'done')    # branch to label
        asm.emit('inclocal', 1)
        asm.emit('jump', 'loop')
        asm.label('done')
        asm.emit('returnvoid')
        code = asm.assemble()

    For branch instructions (``jump``, ``iftrue``, ``iffalse``, ``ifeq``, etc.),
    pass the target label name as the first argument.  The assembler resolves
    relative offsets automatically.

    For ``lookupswitch``, pass ``(default_label, [case_label_0, case_label_1, ...])``.
    """

    def __init__(self):
        # Each entry: ('instr', opcode_byte, operands, fmt_tuple)
        #          or ('label', label_name)
        self._items: List[tuple] = []
        self._labels: Dict[str, int] = {}  # label → item index

    def label(self, name: str) -> 'Assembler':
        """Insert a label at the current position.

        Labels don't generate bytes; they mark positions for branch targets.
        """
        if name in self._labels:
            raise ValueError(f"Duplicate label: '{name}'")
        self._labels[name] = len(self._items)
        self._items.append(('label', name))
        return self

    def emit(self, opcode_name: str, *operands) -> 'Assembler':
        """Emit one instruction.

        Parameters
        ----------
        opcode_name : str
            AVM2 opcode name (e.g. ``'pushstring'``, ``'getlocal_0'``).
        *operands
            Operand values.  For branch opcodes, pass a label name (str).
            For pool references, pass the pool index (int).
            For ``lookupswitch``, pass ``default_label, [case0, case1, ...]``.
            For ``debug``, pass ``(debug_type, string_idx, register, extra)``.
        """
        name = opcode_name.lower()
        op = OPCODE_BY_NAME.get(name)
        if op is None:
            raise ValueError(f"Unknown opcode: '{opcode_name}'")

        fmt = _OP_FORMATS.get(op, ())

        # Build operand list matching the format
        built_ops: list = []
        if op == 0x1B:  # lookupswitch — special handling
            if len(operands) != 2:
                raise ValueError(
                    "lookupswitch requires (default_label, [case_labels])"
                )
            default_label = operands[0]
            case_labels = list(operands[1])
            built_ops.append(('switch_default_s24', default_label))  # placeholder
            built_ops.append(('switch_count', len(case_labels) - 1))
            for cl in case_labels:
                built_ops.append(('switch_case_s24', cl))
        elif op == 0xEF:  # debug — special handling
            if len(operands) != 4:
                raise ValueError(
                    "debug requires (debug_type, string_idx, register, extra)"
                )
            built_ops.append(('debug_type', operands[0]))
            built_ops.append(('debug_string', operands[1]))
            built_ops.append(('debug_reg', operands[2]))
            built_ops.append(('debug_extra', operands[3]))
        else:
            if len(operands) != len(fmt):
                raise ValueError(
                    f"'{opcode_name}' expects {len(fmt)} operand(s), got {len(operands)}"
                )
            for desc, val in zip(fmt, operands):
                built_ops.append((desc, val))

        self._items.append(('instr', op, built_ops, fmt))
        return self

    def emit_raw(self, opcode: int, operands: List[Tuple[str, object]]) -> 'Assembler':
        """Emit a raw instruction by opcode byte and pre-built operand list.

        Useful when copying instructions from :func:`_parse_instructions`.
        """
        fmt = _OP_FORMATS.get(opcode, ())
        self._items.append(('instr', opcode, list(operands), fmt))
        return self

    def assemble(self) -> bytes:
        """Assemble all emitted instructions into AVM2 bytecode.

        Branch offsets are resolved using the labels defined via :meth:`label`.

        Returns
        -------
        bytes
            The assembled bytecode.

        Raises
        ------
        ValueError
            If a label referenced by a branch instruction is not defined.
        """
        # Phase 1: Compute byte positions of each instruction and label
        # (labels consume 0 bytes)
        item_positions: List[int] = []
        pos = 0
        for item in self._items:
            item_positions.append(pos)
            if item[0] == 'label':
                continue  # 0 bytes
            _, op, built_ops, fmt = item
            size = 1  # opcode byte
            for desc, val in built_ops:
                if isinstance(val, str):
                    # Branch target label — s24 is always 3 bytes
                    size += 3
                else:
                    size += _operand_byte_size(desc, val)
            pos += size

        # Build label → byte position map
        label_pos: Dict[str, int] = {}
        for name, item_idx in self._labels.items():
            label_pos[name] = item_positions[item_idx]

        # Phase 2: Resolve branch offsets and serialize
        buf = bytearray()
        for idx, item in enumerate(self._items):
            if item[0] == 'label':
                continue

            _, op, built_ops, fmt = item
            inst_start = item_positions[idx]

            # Calculate instruction end position
            inst_size = 1
            for desc, val in built_ops:
                if isinstance(val, str):
                    inst_size += 3
                else:
                    inst_size += _operand_byte_size(desc, val)
            inst_end = inst_start + inst_size

            buf.append(op)

            for desc, val in built_ops:
                if isinstance(val, str):
                    # Resolve label
                    target = label_pos.get(val)
                    if target is None:
                        raise ValueError(f"Undefined label: '{val}'")

                    if desc == 's24':
                        # Branch offset relative to end of this instruction
                        offset = target - inst_end
                    elif desc in ('switch_default_s24', 'switch_case_s24'):
                        # Switch offsets relative to start of instruction
                        offset = target - inst_start
                    else:
                        raise ValueError(
                            f"Label '{val}' used in non-branch operand '{desc}'"
                        )
                    _ws24(buf, offset)
                elif desc in _U30_ALL_DESCS:
                    _wu30(buf, val)
                elif desc in _U8_ALL_DESCS:
                    _wu8(buf, val)
                elif desc in _S24_ALL_DESCS:
                    _ws24(buf, val)

        return bytes(buf)


# ═══════════════════════════════════════════════════════════════════════════
#  Disassembler — bytecode → human-readable text
# ═══════════════════════════════════════════════════════════════════════════

def disassemble(abc: ABCFile, code: bytes) -> str:
    """Disassemble AVM2 bytecode into human-readable text.

    Parameters
    ----------
    abc : ABCFile
        The parsed ABC file (used for resolving pool references to names).
    code : bytes
        Raw AVM2 bytecode from a MethodBody.

    Returns
    -------
    str
        Multi-line disassembly listing with byte offsets, opcode names,
        and resolved operand values.

    Example output::

        0000  getlocal_0
        0001  pushscope
        0002  pushstring       "hello"  ; str[4]
        0007  returnvalue
    """
    log.debug("disassemble: code_len=%d", len(code))
    instructions = _parse_instructions(code)
    lines: List[str] = []

    # First pass: find branch targets so we can annotate them
    branch_targets: Set[int] = set()
    for start, end, op, operands in instructions:
        for desc, val in operands:
            if desc == 's24':
                branch_targets.add(end + val)
            elif desc in ('switch_default_s24', 'switch_case_s24'):
                branch_targets.add(start + val)

    for start, end, op, operands in instructions:
        name = NAME_BY_OPCODE.get(op, f'0x{op:02X}')

        # Label annotation
        prefix = ''
        if start in branch_targets:
            prefix = f'L{start}:\n'

        parts: List[str] = []
        for desc, val in operands:
            if desc in ('string', 'debug_string'):
                s = abc.strings[val] if 0 < val < len(abc.strings) else '?'
                parts.append(f'"{s}"  ; str[{val}]')
            elif desc == 'multiname':
                mn = abc.mn_full(val) if 0 < val < len(abc.multinames) else '?'
                parts.append(f'{mn}  ; mn[{val}]')
            elif desc == 'namespace':
                ns = abc.ns_name(val) if 0 < val < len(abc.namespaces) else '?'
                parts.append(f'{ns}  ; ns[{val}]')
            elif desc == 'int':
                v = abc.integers[val] if 0 < val < len(abc.integers) else '?'
                parts.append(f'{v}  ; int[{val}]')
            elif desc == 'uint':
                v = abc.uintegers[val] if 0 < val < len(abc.uintegers) else '?'
                parts.append(f'{v}  ; uint[{val}]')
            elif desc == 'double':
                v = abc.doubles[val] if 0 < val < len(abc.doubles) else '?'
                parts.append(f'{v}  ; dbl[{val}]')
            elif desc == 'method':
                parts.append(f'method[{val}]')
            elif desc == 'class':
                parts.append(f'class[{val}]')
            elif desc == 's24':
                target = end + val
                parts.append(f'L{target}  ; offset={val:+d}')
            elif desc in ('switch_default_s24', 'switch_case_s24'):
                target = start + val
                parts.append(f'L{target}  ; offset={val:+d}')
            elif desc == 'switch_count':
                parts.append(f'case_count={val}')
            elif desc in ('debug_type', 'debug_reg', 'debug_extra'):
                parts.append(str(val))
            else:
                parts.append(str(val))

        op_str = ', '.join(parts)
        line = f'{prefix}{start:04d}  {name:<17s}{op_str}'
        lines.append(line)

    return '\n'.join(lines)


# ═══════════════════════════════════════════════════════════════════════════
#  ABCEditor — high-level mutation API
# ═══════════════════════════════════════════════════════════════════════════

class ABCEditor:
    """High-level editor for an AVM2 ABC bytecode block.

    Wraps an :class:`ABCFile` and provides methods to:

    * Add constants to the pool (deduplicating automatically)
    * Create methods and method bodies
    * Build traits (slots, methods, getters, setters, classes, functions)
    * Attach / detach traits on instances, classes, and scripts
    * Replace method body bytecode
    * Serialize the modified ABC back to bytes

    Parameters
    ----------
    abc_data : bytes
        Raw ABC binary data to edit.
    """

    def __init__(self, abc_data: bytes):
        log.debug("ABCEditor.__init__: %d bytes", len(abc_data))
        self.abc = ABCFile(abc_data)

    # ── Constant pool: strings ─────────────────────────────────────

    def ensure_string(self, value: str) -> int:
        """Find or add a string to the constant pool.  Returns its index."""
        for i, s in enumerate(self.abc.strings):
            if s == value:
                return i
        idx = len(self.abc.strings)
        self.abc.strings.append(value)
        return idx

    # ── Constant pool: integers ────────────────────────────────────

    def ensure_int(self, value: int) -> int:
        """Find or add a signed integer to the constant pool.  Returns its index."""
        for i, v in enumerate(self.abc.integers):
            if v == value:
                return i
        idx = len(self.abc.integers)
        self.abc.integers.append(value)
        return idx

    # ── Constant pool: unsigned integers ───────────────────────────

    def ensure_uint(self, value: int) -> int:
        """Find or add an unsigned integer to the constant pool.  Returns its index."""
        for i, v in enumerate(self.abc.uintegers):
            if v == value:
                return i
        idx = len(self.abc.uintegers)
        self.abc.uintegers.append(value)
        return idx

    # ── Constant pool: doubles ─────────────────────────────────────

    def ensure_double(self, value: float) -> int:
        """Find or add a double to the constant pool.  Returns its index.

        Handles NaN correctly (NaN != NaN, so uses bit comparison).
        """
        target_bits = struct.pack('<d', value)
        for i, v in enumerate(self.abc.doubles):
            if struct.pack('<d', v) == target_bits:
                return i
        idx = len(self.abc.doubles)
        self.abc.doubles.append(value)
        return idx

    # ── Constant pool: namespaces ──────────────────────────────────

    def ensure_namespace(self, kind: int, name: str) -> int:
        """Find or add a namespace.  Returns its index.

        Parameters
        ----------
        kind : int
            One of ``NS_Namespace``, ``NS_Package``, ``NS_PackageInternal``,
            ``NS_Protected``, ``NS_Explicit``, ``NS_StaticProtected``,
            ``NS_Private``.
        name : str
            Namespace URI string (e.g. ``"com.example"`` or ``""`` for the
            default namespace).
        """
        name_idx = self.ensure_string(name)
        for i, (k, n) in enumerate(self.abc.namespaces):
            if k == kind and n == name_idx:
                return i
        idx = len(self.abc.namespaces)
        self.abc.namespaces.append((kind, name_idx))
        return idx

    # ── Constant pool: namespace sets ──────────────────────────────

    def ensure_ns_set(self, ns_indices: List[int]) -> int:
        """Find or add a namespace set.  Returns its index.

        Parameters
        ----------
        ns_indices : list of int
            List of namespace pool indices.
        """
        key = tuple(ns_indices)
        for i, ns_set in enumerate(self.abc.ns_sets):
            if tuple(ns_set) == key:
                return i
        idx = len(self.abc.ns_sets)
        self.abc.ns_sets.append(list(ns_indices))
        return idx

    # ── Constant pool: multinames ──────────────────────────────────

    def ensure_multiname(self, kind: int, data: tuple) -> int:
        """Find or add a multiname.  Returns its index.

        Parameters
        ----------
        kind : int
            Multiname kind constant (``MN_QName``, ``MN_Multiname``, etc.).
        data : tuple
            Kind-specific data.  For ``MN_QName``: ``(ns_idx, name_str_idx)``.
            For ``MN_Multiname``: ``(name_str_idx, ns_set_idx)``.
            See AVM2 spec for other kinds.
        """
        for i, (k, d) in enumerate(self.abc.multinames):
            if k == kind and d == data:
                return i
        idx = len(self.abc.multinames)
        self.abc.multinames.append((kind, data))
        return idx

    def ensure_qname(self, ns_idx: int, name: str) -> int:
        """Convenience: find or add a QName multiname.

        Parameters
        ----------
        ns_idx : int
            Namespace pool index.
        name : str
            The name string (will be added to string pool if needed).

        Returns
        -------
        int
            Multiname pool index for the QName.
        """
        name_idx = self.ensure_string(name)
        return self.ensure_multiname(MN_QName, (ns_idx, name_idx))

    def ensure_qname_by_name(self, namespace_uri: str, name: str,
                              ns_kind: int = NS_Package) -> int:
        """Convenience: find or add a QName from string parts.

        Parameters
        ----------
        namespace_uri : str
            e.g. ``"com.example"`` or ``""`` for default.
        name : str
            The property/class name.
        ns_kind : int
            Namespace kind (default: ``NS_Package``).

        Returns
        -------
        int
            Multiname pool index.
        """
        ns_idx = self.ensure_namespace(ns_kind, namespace_uri)
        return self.ensure_qname(ns_idx, name)

    def ensure_typename(self, base_mn_idx: int, param_mn_indices: List[int]) -> int:
        """Find or add a TypeName multiname (e.g. ``Vector.<int>``).

        Parameters
        ----------
        base_mn_idx : int
            Multiname index for the base type (e.g. ``__AS3__.vec::Vector``).
        param_mn_indices : list of int
            Multiname indices for the type parameters.

        Returns
        -------
        int
            Multiname pool index.
        """
        data = (base_mn_idx, tuple(param_mn_indices))
        return self.ensure_multiname(MN_TypeName, data)

    # ── Constant pool: lookups ─────────────────────────────────────

    def find_string(self, value: str) -> Optional[int]:
        """Find a string in the pool.  Returns index or None."""
        for i, s in enumerate(self.abc.strings):
            if s == value:
                return i
        return None

    def find_multiname(self, full_name: str) -> Optional[int]:
        """Find a multiname by its resolved full name (e.g. ``"com.example::MyClass"``).

        Returns the multiname pool index, or None if not found.
        """
        for i in range(1, len(self.abc.multinames)):
            try:
                if self.abc.mn_full(i) == full_name:
                    return i
            except (IndexError, KeyError):
                continue
        return None

    def find_namespace(self, kind: int, name: str) -> Optional[int]:
        """Find a namespace by kind and name string.  Returns index or None."""
        for i, (k, name_idx) in enumerate(self.abc.namespaces):
            if k == kind:
                if 0 < name_idx < len(self.abc.strings):
                    if self.abc.strings[name_idx] == name:
                        return i
                elif name_idx == 0 and name == '':
                    return i
        return None

    # ── Method creation ────────────────────────────────────────────

    def add_method(
        self,
        return_type: int = 0,
        param_types: Optional[List[int]] = None,
        name: str = '',
        flags: int = 0,
        optional_values: Optional[List[Tuple[int, int]]] = None,
        param_names: Optional[List[str]] = None,
        code: Optional[bytes] = None,
        max_stack: int = 1,
        local_count: int = 1,
        init_scope_depth: int = 0,
        max_scope_depth: int = 1,
        exceptions: Optional[List[ExceptionInfo]] = None,
        traits: Optional[List[TraitInfo]] = None,
    ) -> int:
        """Add a new method (MethodInfo + MethodBody) to the ABC.

        Parameters
        ----------
        return_type : int
            Multiname index for the return type (0 = ``*``).
        param_types : list of int, optional
            Multiname indices for parameter types.
        name : str
            Method name (added to string pool).
        flags : int
            Method flags bitmask (``METHOD_HasOptional``, etc.).
        optional_values : list of (vkind, vindex), optional
            Default parameter values.  Automatically sets ``METHOD_HasOptional``.
        param_names : list of str, optional
            Parameter name strings.  Automatically sets ``METHOD_HasParamNames``.
        code : bytes, optional
            Bytecode for the method body.  If None, creates a minimal
            ``returnvoid`` body.
        max_stack, local_count, init_scope_depth, max_scope_depth : int
            Method body metadata for the AVM2 verifier.
        exceptions : list of ExceptionInfo, optional
            Exception handlers for the method body.
        traits : list of TraitInfo, optional
            Activation traits for the method body.

        Returns
        -------
        int
            The new method index in ``abc.methods``.
        """
        ptypes = param_types or []

        mi = MethodInfo()
        mi.param_count = len(ptypes)
        mi.return_type = return_type
        mi.param_types = list(ptypes)
        mi.name_idx = self.ensure_string(name) if name else 0

        # Handle optional values
        if optional_values:
            flags |= METHOD_HasOptional
            mi.optional_values = list(optional_values)
        else:
            mi.optional_values = []

        # Handle parameter names
        if param_names:
            flags |= METHOD_HasParamNames
            mi.param_names = [self.ensure_string(n) for n in param_names]
        else:
            mi.param_names = []

        mi.flags = flags
        method_idx = len(self.abc.methods)
        self.abc.methods.append(mi)

        # Create method body
        if code is None:
            code = bytes([0x47])  # returnvoid

        mb = MethodBody()
        mb.method_idx = method_idx
        mb.max_stack = max_stack
        mb.local_count = local_count
        mb.init_scope_depth = init_scope_depth
        mb.max_scope_depth = max_scope_depth
        mb.code = code
        mb.exceptions = exceptions or []
        mb.traits = traits or []

        self.abc.method_bodies[method_idx] = mb

        return method_idx

    def get_method_body(self, method_idx: int) -> Optional[MethodBody]:
        """Get the MethodBody for a method index, or None."""
        return self.abc.method_bodies.get(method_idx)

    def replace_method_body(
        self,
        method_idx: int,
        code: bytes,
        max_stack: Optional[int] = None,
        local_count: Optional[int] = None,
        init_scope_depth: Optional[int] = None,
        max_scope_depth: Optional[int] = None,
        exceptions: Optional[List[ExceptionInfo]] = None,
        traits: Optional[List[TraitInfo]] = None,
    ) -> MethodBody:
        """Replace the bytecode and metadata of an existing method body.

        Parameters not specified are preserved from the existing body.
        If the method has no existing body, one is created.

        Returns the new/updated MethodBody.
        """
        existing = self.abc.method_bodies.get(method_idx)

        mb = MethodBody()
        mb.method_idx = method_idx
        mb.code = code

        if existing is not None:
            mb.max_stack = max_stack if max_stack is not None else existing.max_stack
            mb.local_count = local_count if local_count is not None else existing.local_count
            mb.init_scope_depth = (init_scope_depth if init_scope_depth is not None
                                   else existing.init_scope_depth)
            mb.max_scope_depth = (max_scope_depth if max_scope_depth is not None
                                  else existing.max_scope_depth)
            mb.exceptions = exceptions if exceptions is not None else list(existing.exceptions)
            mb.traits = traits if traits is not None else list(existing.traits)
        else:
            mb.max_stack = max_stack or 1
            mb.local_count = local_count or 1
            mb.init_scope_depth = init_scope_depth or 0
            mb.max_scope_depth = max_scope_depth or 1
            mb.exceptions = exceptions or []
            mb.traits = traits or []

        self.abc.method_bodies[method_idx] = mb
        return mb

    def update_method_info(
        self,
        method_idx: int,
        return_type: Optional[int] = None,
        param_types: Optional[List[int]] = None,
        name: Optional[str] = None,
        flags: Optional[int] = None,
    ):
        """Update fields of an existing MethodInfo.

        Only provided fields are modified; others are untouched.
        """
        mi = self.abc.methods[method_idx]
        if return_type is not None:
            mi.return_type = return_type
        if param_types is not None:
            mi.param_types = list(param_types)
            mi.param_count = len(param_types)
        if name is not None:
            mi.name_idx = self.ensure_string(name) if name else 0
        if flags is not None:
            mi.flags = flags

    # ── Exception creation ─────────────────────────────────────────

    @staticmethod
    def make_exception(
        from_pos: int,
        to_pos: int,
        target: int,
        exc_type: int = 0,
        var_name: int = 0,
    ) -> ExceptionInfo:
        """Create an ExceptionInfo (try-catch handler).

        Parameters
        ----------
        from_pos, to_pos : int
            Byte offsets in the method body code delimiting the try region.
        target : int
            Byte offset where the catch handler begins.
        exc_type : int
            Multiname index for the exception type (0 = catch-all ``*``).
        var_name : int
            Multiname index for the catch variable name (0 = unnamed).

        Returns
        -------
        ExceptionInfo
        """
        ei = ExceptionInfo()
        ei.from_pos = from_pos
        ei.to_pos = to_pos
        ei.target = target
        ei.exc_type = exc_type
        ei.var_name = var_name
        return ei

    # ── Trait creation ─────────────────────────────────────────────

    @staticmethod
    def make_slot_trait(
        name_idx: int,
        type_name: int = 0,
        slot_id: int = 0,
        vindex: int = 0,
        vkind: int = 0,
        is_const: bool = False,
    ) -> TraitInfo:
        """Create a Slot or Const trait (variable declaration).

        Parameters
        ----------
        name_idx : int
            Multiname pool index for the slot name.
        type_name : int
            Multiname pool index for the declared type (0 = ``*``).
        slot_id : int
            Slot ID (0 = auto-assign).
        vindex : int
            Default value pool index (0 = no default).
        vkind : int
            Default value kind (``CONSTANT_Utf8``, ``CONSTANT_Int``, etc.).
        is_const : bool
            If True, create a ``TRAIT_Const`` instead of ``TRAIT_Slot``.

        Returns
        -------
        TraitInfo
        """
        t = TraitInfo()
        t.name_idx = name_idx
        t.kind = TRAIT_Const if is_const else TRAIT_Slot
        t.attr = 0
        t.slot_id = slot_id
        t.type_name = type_name
        t.vindex = vindex
        t.vkind = vkind
        t.metadata = []
        return t

    @staticmethod
    def make_method_trait(
        name_idx: int,
        method_idx: int,
        disp_id: int = 0,
        attr: int = 0,
    ) -> TraitInfo:
        """Create a Method trait.

        Parameters
        ----------
        name_idx : int
            Multiname pool index for the method name.
        method_idx : int
            Index into ``abc.methods``.
        disp_id : int
            Dispatch ID (usually 0).
        attr : int
            Attribute flags (``0x01`` = final, ``0x02`` = override).

        Returns
        -------
        TraitInfo
        """
        t = TraitInfo()
        t.name_idx = name_idx
        t.kind = TRAIT_Method
        t.attr = attr
        t.disp_id = disp_id
        t.method_idx = method_idx
        t.metadata = []
        return t

    @staticmethod
    def make_getter_trait(
        name_idx: int,
        method_idx: int,
        disp_id: int = 0,
        attr: int = 0,
    ) -> TraitInfo:
        """Create a Getter trait."""
        t = TraitInfo()
        t.name_idx = name_idx
        t.kind = TRAIT_Getter
        t.attr = attr
        t.disp_id = disp_id
        t.method_idx = method_idx
        t.metadata = []
        return t

    @staticmethod
    def make_setter_trait(
        name_idx: int,
        method_idx: int,
        disp_id: int = 0,
        attr: int = 0,
    ) -> TraitInfo:
        """Create a Setter trait."""
        t = TraitInfo()
        t.name_idx = name_idx
        t.kind = TRAIT_Setter
        t.attr = attr
        t.disp_id = disp_id
        t.method_idx = method_idx
        t.metadata = []
        return t

    @staticmethod
    def make_class_trait(
        name_idx: int,
        class_idx: int,
        slot_id: int = 0,
    ) -> TraitInfo:
        """Create a Class trait (used in script traits for class definitions)."""
        t = TraitInfo()
        t.name_idx = name_idx
        t.kind = TRAIT_Class
        t.attr = 0
        t.slot_id = slot_id
        t.class_idx = class_idx
        t.metadata = []
        return t

    @staticmethod
    def make_function_trait(
        name_idx: int,
        method_idx: int,
        slot_id: int = 0,
    ) -> TraitInfo:
        """Create a Function trait."""
        t = TraitInfo()
        t.name_idx = name_idx
        t.kind = TRAIT_Function
        t.attr = 0
        t.slot_id = slot_id
        t.method_idx = method_idx
        t.metadata = []
        return t

    # ── Class / instance discovery ─────────────────────────────────

    def find_class(self, full_name: str) -> Optional[int]:
        """Find a class by fully-qualified name.  Returns class index or None.

        The class index can be used with ``abc.instances[idx]`` and
        ``abc.classes[idx]``.
        """
        for i, inst in enumerate(self.abc.instances):
            if self.abc.mn_full(inst.name_idx) == full_name:
                return i
        return None

    def list_classes(self) -> List[str]:
        """Return a list of all fully-qualified class names in the ABC."""
        result = []
        for inst in self.abc.instances:
            try:
                result.append(self.abc.mn_full(inst.name_idx))
            except (IndexError, KeyError):
                result.append(f'<unknown mn[{inst.name_idx}]>')
        return result

    def get_class_methods(self, class_idx: int) -> Dict[str, int]:
        """Get all method indices for a class, keyed by source-key.

        Returns a dict like::

            {
                'constructor': 42,
                'cinit': 43,
                'method:doSomething': 44,
                'get:width': 45,
                'set:width': 46,
            }
        """
        inst = self.abc.instances[class_idx]
        cls = self.abc.classes[class_idx]
        abc = self.abc
        result: Dict[str, int] = {}

        result['constructor'] = inst.iinit
        result['cinit'] = cls.cinit

        _KIND_PREFIX = {
            TRAIT_Method: 'method', TRAIT_Getter: 'get',
            TRAIT_Setter: 'set', TRAIT_Function: 'method',
        }
        for t in inst.traits:
            if t.kind in _KIND_PREFIX:
                name = abc.mn_name(t.name_idx)
                prefix = _KIND_PREFIX[t.kind]
                result[f'{prefix}:{name}'] = t.method_idx

        for t in cls.traits:
            if t.kind in _KIND_PREFIX:
                name = abc.mn_name(t.name_idx)
                prefix = _KIND_PREFIX[t.kind]
                result[f'static:{prefix}:{name}'] = t.method_idx

        return result

    # ── Trait attachment / removal ──────────────────────────────────

    def add_instance_trait(self, class_name_or_idx: Union[str, int],
                           trait: TraitInfo):
        """Add a trait to a class's instance (non-static) traits.

        Parameters
        ----------
        class_name_or_idx : str or int
            Fully-qualified class name or class index.
        trait : TraitInfo
            The trait to add (created via ``make_method_trait`` etc.).
        """
        idx = self._resolve_class_idx(class_name_or_idx)
        self.abc.instances[idx].traits.append(trait)

    def add_class_trait(self, class_name_or_idx: Union[str, int],
                        trait: TraitInfo):
        """Add a trait to a class's static traits."""
        idx = self._resolve_class_idx(class_name_or_idx)
        self.abc.classes[idx].traits.append(trait)

    def remove_instance_trait(self, class_name_or_idx: Union[str, int],
                              trait_name: str) -> Optional[TraitInfo]:
        """Remove an instance trait by its resolved name.

        Returns the removed TraitInfo, or None if not found.
        """
        idx = self._resolve_class_idx(class_name_or_idx)
        traits = self.abc.instances[idx].traits
        return self._remove_trait_by_name(traits, trait_name)

    def remove_class_trait(self, class_name_or_idx: Union[str, int],
                           trait_name: str) -> Optional[TraitInfo]:
        """Remove a static (class-level) trait by its resolved name.

        Returns the removed TraitInfo, or None if not found.
        """
        idx = self._resolve_class_idx(class_name_or_idx)
        traits = self.abc.classes[idx].traits
        return self._remove_trait_by_name(traits, trait_name)

    def add_script_trait(self, script_idx: int, trait: TraitInfo):
        """Add a trait to a script's trait list."""
        self.abc.scripts[script_idx].traits.append(trait)

    def remove_script_trait(self, script_idx: int,
                            trait_name: str) -> Optional[TraitInfo]:
        """Remove a script trait by resolved name."""
        traits = self.abc.scripts[script_idx].traits
        return self._remove_trait_by_name(traits, trait_name)

    # ── Instance / class metadata modification ─────────────────────

    def set_super_class(self, class_name_or_idx: Union[str, int],
                        super_mn_idx: int):
        """Change a class's superclass.

        Parameters
        ----------
        class_name_or_idx : str or int
            Target class.
        super_mn_idx : int
            Multiname index for the new superclass.
        """
        idx = self._resolve_class_idx(class_name_or_idx)
        self.abc.instances[idx].super_idx = super_mn_idx

    def add_interface(self, class_name_or_idx: Union[str, int],
                      interface_mn_idx: int):
        """Add an interface to a class's implements list."""
        idx = self._resolve_class_idx(class_name_or_idx)
        inst = self.abc.instances[idx]
        if interface_mn_idx not in inst.interfaces:
            inst.interfaces.append(interface_mn_idx)

    def remove_interface(self, class_name_or_idx: Union[str, int],
                         interface_mn_idx: int):
        """Remove an interface from a class's implements list."""
        idx = self._resolve_class_idx(class_name_or_idx)
        inst = self.abc.instances[idx]
        if interface_mn_idx in inst.interfaces:
            inst.interfaces.remove(interface_mn_idx)

    def set_instance_flags(self, class_name_or_idx: Union[str, int],
                           flags: int):
        """Set the instance flags (sealed, final, interface, etc.)."""
        idx = self._resolve_class_idx(class_name_or_idx)
        self.abc.instances[idx].flags = flags

    def get_instance_flags(self, class_name_or_idx: Union[str, int]) -> int:
        """Get the current instance flags."""
        idx = self._resolve_class_idx(class_name_or_idx)
        return self.abc.instances[idx].flags

    # ── Constructor / initializer access ───────────────────────────

    def get_constructor_idx(self, class_name_or_idx: Union[str, int]) -> int:
        """Get the method index of a class's instance constructor (iinit)."""
        idx = self._resolve_class_idx(class_name_or_idx)
        return self.abc.instances[idx].iinit

    def get_cinit_idx(self, class_name_or_idx: Union[str, int]) -> int:
        """Get the method index of a class's static initializer (cinit)."""
        idx = self._resolve_class_idx(class_name_or_idx)
        return self.abc.classes[idx].cinit

    def set_constructor(self, class_name_or_idx: Union[str, int],
                        method_idx: int):
        """Replace a class's instance constructor with a different method."""
        idx = self._resolve_class_idx(class_name_or_idx)
        self.abc.instances[idx].iinit = method_idx

    def set_cinit(self, class_name_or_idx: Union[str, int],
                  method_idx: int):
        """Replace a class's static initializer with a different method."""
        idx = self._resolve_class_idx(class_name_or_idx)
        self.abc.classes[idx].cinit = method_idx

    # ── Metadata ───────────────────────────────────────────────────

    def add_metadata(self, name: str, items: List[Tuple[str, str]]) -> int:
        """Add a metadata entry.  Returns the metadata index.

        Parameters
        ----------
        name : str
            Metadata name (e.g. ``"SWF"``, ``"Event"``).
        items : list of (key, value)
            Key-value pairs as strings.
        """
        name_idx = self.ensure_string(name)
        item_indices = [
            (self.ensure_string(k), self.ensure_string(v))
            for k, v in items
        ]
        idx = len(self.abc.metadata_entries)
        self.abc.metadata_entries.append((name_idx, item_indices))
        return idx

    # ── Serialization ──────────────────────────────────────────────

    def serialize(self) -> bytes:
        """Serialize the modified ABC back to raw bytes.

        Uses the battle-tested serializer from :mod:`abc_patcher`.
        """
        return serialize_abc(self.abc)

    # ── Utility: round-trip validation ─────────────────────────────

    def validate_round_trip(self) -> bool:
        """Serialize and re-parse, checking that all key structures survive.

        Returns True if round-trip is clean, raises AssertionError with
        details if not.
        """
        data = self.serialize()
        abc2 = ABCFile(data)

        assert len(abc2.strings) == len(self.abc.strings), \
            f"String pool: {len(self.abc.strings)} → {len(abc2.strings)}"
        assert len(abc2.integers) == len(self.abc.integers), \
            f"Integer pool: {len(self.abc.integers)} → {len(abc2.integers)}"
        assert len(abc2.uintegers) == len(self.abc.uintegers), \
            f"UInt pool: {len(self.abc.uintegers)} → {len(abc2.uintegers)}"
        assert len(abc2.doubles) == len(self.abc.doubles), \
            f"Double pool: {len(self.abc.doubles)} → {len(abc2.doubles)}"
        assert len(abc2.namespaces) == len(self.abc.namespaces), \
            f"Namespace pool: {len(self.abc.namespaces)} → {len(abc2.namespaces)}"
        assert len(abc2.ns_sets) == len(self.abc.ns_sets), \
            f"NS set pool: {len(self.abc.ns_sets)} → {len(abc2.ns_sets)}"
        assert len(abc2.multinames) == len(self.abc.multinames), \
            f"Multiname pool: {len(self.abc.multinames)} → {len(abc2.multinames)}"
        assert len(abc2.methods) == len(self.abc.methods), \
            f"Methods: {len(self.abc.methods)} → {len(abc2.methods)}"
        assert len(abc2.instances) == len(self.abc.instances), \
            f"Instances: {len(self.abc.instances)} → {len(abc2.instances)}"
        assert len(abc2.classes) == len(self.abc.classes), \
            f"Classes: {len(self.abc.classes)} → {len(abc2.classes)}"
        assert len(abc2.scripts) == len(self.abc.scripts), \
            f"Scripts: {len(self.abc.scripts)} → {len(abc2.scripts)}"
        assert len(abc2.method_bodies) == len(self.abc.method_bodies), \
            f"Method bodies: {len(self.abc.method_bodies)} → {len(abc2.method_bodies)}"

        # Verify method body code matches
        for midx, body in self.abc.method_bodies.items():
            body2 = abc2.method_bodies.get(midx)
            assert body2 is not None, f"Method body {midx} missing after round-trip"
            assert body.code == body2.code, \
                f"Method {midx} code: {len(body.code)}B → {len(body2.code)}B"

        return True

    # ── Internal helpers ───────────────────────────────────────────

    def _resolve_class_idx(self, name_or_idx: Union[str, int]) -> int:
        """Resolve a class name or index to a valid class index."""
        if isinstance(name_or_idx, int):
            if name_or_idx < 0 or name_or_idx >= len(self.abc.instances):
                raise IndexError(f"Class index {name_or_idx} out of range")
            return name_or_idx
        idx = self.find_class(name_or_idx)
        if idx is None:
            raise ValueError(f"Class '{name_or_idx}' not found")
        return idx

    def _remove_trait_by_name(self, traits: list,
                              trait_name: str) -> Optional[TraitInfo]:
        """Remove a trait from a list by its resolved name."""
        for i, t in enumerate(traits):
            try:
                resolved = self.abc.mn_name(t.name_idx)
            except (IndexError, KeyError):
                continue
            if resolved == trait_name:
                return traits.pop(i)
        return None
