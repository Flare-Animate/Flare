"""AVM2 opcode constants and increment/decrement pattern matcher."""

from __future__ import annotations

import logging
from typing import Optional, Tuple

log = logging.getLogger(__name__)

from .abc_parser import _ru30


# ═══════════════════════════════════════════════════════════════════════════
#  AVM2 Opcode Definitions
# ═══════════════════════════════════════════════════════════════════════════

OP_BKPT            = 0x01
OP_NOP             = 0x02
OP_THROW           = 0x03
OP_GETSUPER        = 0x04
OP_SETSUPER        = 0x05
OP_DXNS            = 0x06
OP_DXNSLATE        = 0x07
OP_KILL            = 0x08
OP_LABEL           = 0x09
OP_IFNLT           = 0x0C
OP_IFNLE           = 0x0D
OP_IFNGT           = 0x0E
OP_IFNGE           = 0x0F
OP_JUMP            = 0x10
OP_IFTRUE          = 0x11
OP_IFFALSE         = 0x12
OP_IFEQ            = 0x13
OP_IFNE            = 0x14
OP_IFLT            = 0x15
OP_IFLE            = 0x16
OP_IFGT            = 0x17
OP_IFGE            = 0x18
OP_IFSTRICTEQ      = 0x19
OP_IFSTRICTNE      = 0x1A
OP_LOOKUPSWITCH    = 0x1B
OP_PUSHWITH        = 0x1C
OP_POPSCOPE        = 0x1D
OP_NEXTNAME        = 0x1E
OP_HASNEXT         = 0x1F
OP_PUSHNULL        = 0x20
OP_PUSHUNDEFINED   = 0x21
OP_NEXTVALUE       = 0x23
OP_PUSHBYTE        = 0x24
OP_PUSHSHORT       = 0x25
OP_PUSHTRUE        = 0x26
OP_PUSHFALSE       = 0x27
OP_PUSHNAN         = 0x28
OP_POP             = 0x29
OP_DUP             = 0x2A
OP_SWAP            = 0x2B
OP_PUSHSTRING      = 0x2C
OP_PUSHINT         = 0x2D
OP_PUSHUINT        = 0x2E
OP_PUSHDOUBLE      = 0x2F
OP_PUSHSCOPE       = 0x30
OP_PUSHNAMESPACE   = 0x31
OP_HASNEXT2        = 0x32
OP_LI8             = 0x35
OP_LI16            = 0x36
OP_LI32            = 0x37
OP_LF32            = 0x38
OP_LF64            = 0x39
OP_SI8             = 0x3A
OP_SI16            = 0x3B
OP_SI32            = 0x3C
OP_SF32            = 0x3D
OP_SF64            = 0x3E
OP_NEWFUNCTION     = 0x40
OP_CALL            = 0x41
OP_CONSTRUCT       = 0x42
OP_CALLMETHOD      = 0x43
OP_CALLSTATIC      = 0x44
OP_CALLSUPER       = 0x45
OP_CALLPROPERTY    = 0x46
OP_RETURNVOID      = 0x47
OP_RETURNVALUE     = 0x48
OP_CONSTRUCTSUPER  = 0x49
OP_CONSTRUCTPROP   = 0x4A
OP_CALLPROPLEX     = 0x4C
OP_CALLSUPERVOID   = 0x4E
OP_CALLPROPVOID    = 0x4F
OP_SXI1            = 0x50
OP_SXI8            = 0x51
OP_SXI16           = 0x52
OP_APPLYTYPE       = 0x53
OP_NEWOBJECT       = 0x55
OP_NEWARRAY        = 0x56
OP_NEWACTIVATION   = 0x57
OP_NEWCLASS        = 0x58
OP_GETDESCENDANTS  = 0x59
OP_NEWCATCH        = 0x5A
OP_FINDPROPSTRICT  = 0x5D
OP_FINDPROPERTY    = 0x5E
OP_FINDDEF         = 0x5F
OP_GETLEX          = 0x60
OP_SETPROPERTY     = 0x61
OP_GETLOCAL        = 0x62
OP_SETLOCAL        = 0x63
OP_GETGLOBALSCOPE  = 0x64
OP_GETSCOPEOBJECT  = 0x65
OP_GETPROPERTY     = 0x66
OP_INITPROPERTY    = 0x68
OP_DELETEPROPERTY  = 0x6A
OP_GETSLOT         = 0x6C
OP_SETSLOT         = 0x6D
OP_GETGLOBALSLOT   = 0x6E
OP_SETGLOBALSLOT   = 0x6F
OP_CONVERT_S       = 0x70
OP_ESC_XELEM       = 0x71
OP_ESC_XATTR       = 0x72
OP_CONVERT_I       = 0x73
OP_CONVERT_U       = 0x74
OP_CONVERT_D       = 0x75
OP_CONVERT_B       = 0x76
OP_CONVERT_O       = 0x77
OP_CHECKFILTER     = 0x78
OP_COERCE          = 0x80
OP_COERCE_B        = 0x81
OP_COERCE_A        = 0x82
OP_COERCE_I        = 0x83
OP_COERCE_D        = 0x84
OP_COERCE_S        = 0x85
OP_ASTYPE          = 0x86
OP_ASTYPELATE      = 0x87
OP_COERCE_U        = 0x88
OP_COERCE_O        = 0x89
OP_NEGATE          = 0x90
OP_INCREMENT       = 0x91
OP_INCLOCAL        = 0x92
OP_DECREMENT       = 0x93
OP_DECLOCAL        = 0x94
OP_TYPEOF          = 0x95
OP_NOT             = 0x96
OP_BITNOT          = 0x97
OP_ADD             = 0xA0
OP_SUBTRACT        = 0xA1
OP_MULTIPLY        = 0xA2
OP_DIVIDE          = 0xA3
OP_MODULO          = 0xA4
OP_LSHIFT          = 0xA5
OP_RSHIFT          = 0xA6
OP_URSHIFT         = 0xA7
OP_BITAND          = 0xA8
OP_BITOR           = 0xA9
OP_BITXOR          = 0xAA
OP_EQUALS          = 0xAB
OP_STRICTEQUALS    = 0xAC
OP_LESSTHAN        = 0xAD
OP_LESSEQUALS      = 0xAE
OP_GREATERTHAN     = 0xAF
OP_GREATEREQUALS   = 0xB0
OP_INSTANCEOF      = 0xB1
OP_ISTYPE          = 0xB2
OP_ISTYPELATE      = 0xB3
OP_IN              = 0xB4
OP_INCREMENT_I     = 0xC0
OP_DECREMENT_I     = 0xC1
OP_INCLOCAL_I      = 0xC2
OP_DECLOCAL_I      = 0xC3
OP_NEGATE_I        = 0xC4
OP_ADD_I           = 0xC5
OP_SUBTRACT_I      = 0xC6
OP_MULTIPLY_I      = 0xC7
OP_GETLOCAL_0      = 0xD0
OP_GETLOCAL_1      = 0xD1
OP_GETLOCAL_2      = 0xD2
OP_GETLOCAL_3      = 0xD3
OP_SETLOCAL_0      = 0xD4
OP_SETLOCAL_1      = 0xD5
OP_SETLOCAL_2      = 0xD6
OP_SETLOCAL_3      = 0xD7
OP_DEBUG           = 0xEF
OP_DEBUGLINE       = 0xF0
OP_DEBUGFILE       = 0xF1

# Sets for increment/decrement detection
_INC_OPS = frozenset({0x91, 0xC0})  # OP_INCREMENT, OP_INCREMENT_I
_INCDEC_OPS = frozenset({0x91, 0xC0, 0x93, 0xC1})  # OP_INCREMENT, INCREMENT_I, DECREMENT, DECREMENT_I

def _match_local_incdec(code: bytes, p: int, reg_idx: int) -> Optional[Tuple[bool, bool, int]]:
    """Detect pre/post increment/decrement pattern after a getlocal instruction.

    Patterns (starting from position p, after the getlocal was consumed):
      Post (a++/a--): dup -> increment/decrement -> setlocal_N
      Pre  (++a/--a): increment/decrement -> dup -> setlocal_N

    Returns (is_pre, is_increment, new_p) or None if no pattern matched.
    """
    if p + 2 > len(code):
        return None

    b0 = code[p]
    b1 = code[p + 1] if p + 1 < len(code) else 0xFF

    def _check_setlocal(pos: int) -> Optional[int]:
        """Check if opcode at pos is setlocal for register reg_idx. Return new pos or None."""
        if pos >= len(code):
            return None
        op = code[pos]
        if 0 <= reg_idx <= 3 and op == OP_SETLOCAL_0 + reg_idx:
            return pos + 1
        if op == OP_SETLOCAL:
            if pos + 1 >= len(code):
                return None
            idx, new_p = _ru30(code, pos + 1)
            if idx == reg_idx:
                return new_p
        return None

    # Post pattern: dup -> inc/dec -> setlocal_N
    if b0 == OP_DUP and b1 in _INCDEC_OPS:
        is_inc = b1 in _INC_OPS
        set_result = _check_setlocal(p + 2)
        if set_result is not None:
            return (False, is_inc, set_result)

    # Pre pattern: inc/dec -> dup -> setlocal_N
    if b0 in _INCDEC_OPS and b1 == OP_DUP:
        is_inc = b0 in _INC_OPS
        set_result = _check_setlocal(p + 2)
        if set_result is not None:
            return (True, is_inc, set_result)

    return None


# Build __all__ to export all OP_* constants and underscore-prefixed names
__all__ = [_n for _n in list(globals()) if _n.startswith('OP_')]
__all__ += ['_INC_OPS', '_INCDEC_OPS', '_match_local_incdec']

