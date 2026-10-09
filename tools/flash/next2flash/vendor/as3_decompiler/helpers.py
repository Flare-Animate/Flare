"""Helper functions for AS3 decompilation."""

from __future__ import annotations

import struct
from typing import Dict, List

from .abc_parser import (
    ABCFile, _ru30,
    MN_QName, MN_QNameA,
    MN_Multiname, MN_MultinameA, MN_MultinameL, MN_MultinameLA, MN_TypeName,
    NS_PackageNamespace, NS_PrivateNs, NS_ProtectedNs,
    NS_StaticProtectedNs, NS_PackageInternalNs,
)
from .opcodes import *  # All OP_* constants used by _skip_operands

__all__ = [
    'INDENT_UNIT',
    '_pop_n', '_is_type_default', '_strip_redundant_cast', '_add_type_cast_if_needed',
    '_fmt_call', '_binop', '_bitwise_binop', '_fmt_hex', '_fmt_hex_const',
    '_to_hex_if_int', '_fmt_uint', '_fmt_int', '_escape_str',
    '_expand_multiline_stmt', '_has_outer_parens', '_needs_ternary_wrap',
    '_find_op_outside_parens', '_wrap_for_logical', '_skip_operands', '_check_mn_ns_set',
    '_check_mn_ns_set_typed', '_access_modifier',
]

# Configurable indent unit used throughout the decompiler.
# Change this single value to switch from 4-space to tab or 2-space, etc.
INDENT_UNIT = '    '


# ═══════════════════════════════════════════════════════════════════════════
#  Helpers
# ═══════════════════════════════════════════════════════════════════════════

def _pop_n(stack: List[str], n: int, error_log: List[str] = None, pos: str = '') -> List[str]:
    """Pop n items from stack, reversed for argument order.
    
    Args:
        stack: The stack to pop from
        n: Number of items to pop
        error_log: Optional error log list to track stack underflow
        pos: Optional position/context string for error messages
    
    Returns:
        List of popped items in argument order (reversed)
    """
    args = []
    for _ in range(n):
        if stack:
            args.append(stack.pop())
        else:
            msg = f'Stack underflow (expected {n} items)'
            if pos:
                msg = f'{msg} at {pos}'
            args.append('?')
            if error_log is not None:
                error_log.append(msg)
    args.reverse()
    return args

def _is_type_default(ltype: str, value: str) -> bool:
    """Check if a value is the implicit default for a given AS3 type."""
    if ltype == 'int' and value == '0':
        return True
    if ltype == 'uint' and value == '0':
        return True
    if ltype == 'Boolean' and value == 'false':
        return True
    if ltype not in ('*', 'int', 'uint', 'Number', 'Boolean', 'String') and value == 'null':
        return True
    return False

def _strip_redundant_cast(ltype: str, value: str) -> str:
    """Strip redundant type casts when the variable is already typed.
    E.g., var x:int = int(expr) → var x:int = expr.
    Note: String/Number/Boolean casts are preserved since they may be explicit."""
    cast_map = {'int': 'int(', 'uint': 'uint('}
    prefix = cast_map.get(ltype)
    if prefix and value.startswith(prefix) and value.endswith(')'):
        # Verify matching parens
        inner = value[len(prefix):-1]
        depth = 0
        for ch in inner:
            if ch == '(': depth += 1
            elif ch == ')': depth -= 1
            if depth < 0:
                return value  # Parens don't match — don't strip
        if depth == 0:
            return inner
    return value

def _add_type_cast_if_needed(ltype: str, value: str, local_types: Dict[int, str],
                              local_names: Dict[int, str]) -> str:
    """Add explicit type cast when the assigned value's type clearly mismatches the var type.

    Only wraps in obvious mismatch cases to avoid excessive casting:
    - String var ← numeric variable → String(var)
    - Number var ← string literal → Number(literal)
    - Boolean var ← numeric literal → Boolean(literal)
    """
    v = value.strip()
    if ltype == 'String' and not v.startswith('String(') and not v.startswith('"'):
        # Check if value is a variable with a known non-String type
        for reg, nm in local_names.items():
            if v == nm:
                vtype = local_types.get(reg)
                if vtype and vtype in ('Number', 'int', 'uint'):
                    return f'String({value})'
                break
    elif ltype == 'Number' and not v.startswith('Number('):
        if v.startswith('"') or v.startswith("'"):
            return f'Number({value})'
        # Check if value is a variable with a known non-Number type
        for reg, nm in local_names.items():
            if v == nm:
                vtype = local_types.get(reg)
                if vtype and vtype == 'String':
                    return f'Number({value})'
                break
    elif ltype == 'Boolean' and not v.startswith('Boolean('):
        if v.lstrip('-').isdigit() and v not in ('true', 'false'):
            return f'Boolean({value})'
    return value

def _fmt_call(obj: str, name: str, args: List[str]) -> str:
    a = ', '.join(args)
    if obj in ('', 'global') or obj == name:
        return f'{name}({a})'
    return f'{obj}.{name}({a})'

def _binop(stack: List[str], op: str) -> None:
    b = stack.pop() if stack else '?'
    a = stack.pop() if stack else '?'
    stack.append(f'({a} {op} {b})')

def _bitwise_binop(stack: List[str], op: str) -> None:
    """Binary op with hex formatting for integer literal operands."""
    b = stack.pop() if stack else '?'
    a = stack.pop() if stack else '?'
    stack.append(f'({_to_hex_if_int(a)} {op} {_to_hex_if_int(b)})')

def _fmt_hex(v: int) -> str:
    """Format as hex with byte-aligned (even digit count) padding."""
    h = f'{v:X}'
    if len(h) % 2:
        h = '0' + h
    return f'0x{h}'

def _fmt_hex_const(v: int) -> str:
    """Format as hex for constant declarations (min 4 digits)."""
    h = f'{v:X}'
    if len(h) < 4:
        h = h.zfill(4)
    return f'0x{h}'

def _to_hex_if_int(s: str) -> str:
    """If s is a non-negative decimal integer literal, convert to byte-aligned hex."""
    try:
        v = int(s)
        if v >= 0:
            return _fmt_hex(v)
    except (ValueError, OverflowError):
        pass  # not an integer literal (or out of range) - fall through and return s unchanged
    return s

def _fmt_uint(v: int) -> str:
    """Format an unsigned integer."""
    return str(v)

def _fmt_int(v: int) -> str:
    """Format an integer."""
    return str(v)


def _escape_str(s: str) -> str:
    """Escape special chars in an AS3 string literal.

    Handles all control characters (0x00-0x1F, 0x7F) and Unicode
    line separators (U+2028, U+2029) that would break string literals
    if emitted as raw bytes.
    """
    out = []
    for ch in s:
        cp = ord(ch)
        if ch == '\\':
            out.append('\\\\')
        elif ch == '"':
            out.append('\\"')
        elif ch == '\n':
            out.append('\\n')
        elif ch == '\r':
            out.append('\\r')
        elif ch == '\t':
            out.append('\\t')
        elif cp == 0:
            out.append('\\0')
        elif ch == '\f':
            out.append('\\f')
        elif cp == 0x2028:
            out.append('\\u2028')
        elif cp == 0x2029:
            out.append('\\u2029')
        elif cp < 0x20 or cp == 0x7F:
            out.append(f'\\x{cp:02X}')
        else:
            out.append(ch)
    return ''.join(out)


def _expand_multiline_stmt(stmt: str, base_indent: str) -> list:
    """Expand a statement containing multi-line object literals into
    properly indented output lines.

    Object literals use bare \\n as line separators. This function adds
    context-aware indentation: each line within an object gets indented
    4 spaces deeper than the { that opened it.  The closing } returns to
    the indentation of the { line.
    """
    if '\n' not in stmt:
        return [f'{base_indent}{stmt}']

    result = []
    base = len(base_indent)
    # Calculate the actual starting indent (base + leading spaces in stmt)
    leading_spaces = len(stmt) - len(stmt.lstrip(' '))
    actual_indent = base + leading_spaces
    indent_stack = [actual_indent]  # stack of indent levels for each { depth
    cur_line = base_indent
    indent_width = len(INDENT_UNIT)

    i = 0
    while i < len(stmt):
        ch = stmt[i]
        if ch == '\n':
            result.append(cur_line)
            # Peek ahead: if next non-space char is }, use outer indent
            j = i + 1
            while j < len(stmt) and stmt[j] == ' ':
                j += 1
            if j < len(stmt) and stmt[j] == '}':
                # Closing brace — use the indent of the { that opens it
                if len(indent_stack) > 1:
                    cur_line = ' ' * indent_stack[-2]
                else:
                    cur_line = ' ' * indent_stack[-1]
            else:
                cur_line = ' ' * indent_stack[-1]
        elif ch == '{':
            cur_line += ch
            # Push new indent level (one indent_width more than current)
            indent_stack.append(indent_stack[-1] + indent_width)
        elif ch == '}':
            if len(indent_stack) > 1:
                indent_stack.pop()
            cur_line += ch
        else:
            cur_line += ch
        i += 1

    if cur_line.strip():
        result.append(cur_line)
    return result


def _has_outer_parens(expr: str) -> bool:
    """Check if expression has matching outer parentheses."""
    if not expr.startswith('(') or not expr.endswith(')'):
        return False
    depth = 0
    for i, c in enumerate(expr):
        if c == '(': depth += 1
        elif c == ')': depth -= 1
        if depth == 0 and i < len(expr) - 1:
            return False  # First ( closes before end
    return True

def _needs_ternary_wrap(expr: str) -> bool:
    """Check if a ternary branch expression needs wrapping in parens."""
    if _has_outer_parens(expr):
        return False
    # Wrap if contains top-level binary operators (space + op + space pattern)
    depth = 0
    in_str = False
    for i, c in enumerate(expr):
        if c == '"' and not in_str:
            in_str = True
        elif c == '"' and in_str:
            in_str = False
        if in_str:
            continue
        if c == '(':
            depth += 1
        elif c == ')':
            depth -= 1
        if depth == 0 and c == ' ':
            # Check if followed by operator
            rest = expr[i+1:]
            for op in ('+', '-', '*', '/', '%', '&&', '||', '==', '!=', '===', '!==',
                       '<', '>', '<=', '>=', '&', '|', '^', '<<', '>>', '>>>'):
                if rest.startswith(op + ' ') or rest.startswith(op + '('):
                    return True
    return False

def _wrap_for_logical(expr: str, join_op: str) -> str:
    """Wrap an operand for a logical && or || combination, but only if needed.

    Simple comparisons (a == b) don't need wrapping when joined by || or &&
    because == has higher precedence. Only wrap when the operand itself
    contains a *different* logical operator at depth 0 (mixing && and ||).
    """
    if _has_outer_parens(expr):
        return expr
    # Check if expression contains a different logical operator at depth 0
    other_op = '||' if join_op == '&&' else '&&'
    depth = 0
    i = 0
    while i < len(expr) - 1:
        c = expr[i]
        if c == '(':
            depth += 1
        elif c == ')':
            depth -= 1
        elif c == '"':
            i += 1
            while i < len(expr) and expr[i] != '"':
                if expr[i] == '\\':
                    i += 1
                i += 1
        elif depth == 0 and expr[i:i+2] == other_op:
            return f'({expr})'
        i += 1
    return expr


def _find_op_outside_parens(expr: str, op: str) -> int:
    depth = 0
    i = 0
    while i <= len(expr) - len(op):
        c = expr[i]
        if c == '(':
            depth += 1
        elif c == ')':
            depth -= 1
        elif c == '"':
            # Skip double-quoted string literal
            i += 1
            while i < len(expr) and expr[i] != '"':
                if expr[i] == '\\':
                    i += 1
                i += 1
            i += 1
            continue
        elif c == "'":
            # Skip single-quoted string literal
            i += 1
            while i < len(expr) and expr[i] != "'":
                if expr[i] == '\\':
                    i += 1
                i += 1
            i += 1
            continue
        elif depth == 0 and expr[i:i + len(op)] == op:
            # Make sure it's not part of a longer operator
            if op in ('==', '!=') and i + len(op) < len(expr) and expr[i + len(op)] == '=':
                i += 1
                continue
            if op in ('<', '>') and i + len(op) < len(expr) and expr[i + len(op)] in ('=', '<', '>'):
                i += 1
                continue
            if op == '=' and i > 0 and expr[i - 1] in ('!', '<', '>', '='):
                i += 1
                continue
            return i
        i += 1
    return -1

def _skip_operands(op: int, code: bytes, p: int) -> int:
    """Skip past an instruction's operands.
    
    If bytecode is malformed and bounds are exceeded, returns length of code
    (graceful degradation instead of crash).
    """
    try:
        if op == OP_PUSHBYTE:
            return p + 1
        if op in (OP_PUSHSHORT, OP_PUSHSTRING, OP_PUSHINT, OP_PUSHUINT,
                  OP_PUSHDOUBLE, OP_PUSHNAMESPACE,
                  OP_GETSUPER, OP_SETSUPER, OP_DXNS, OP_KILL,
                  OP_NEWFUNCTION, OP_NEWCLASS, OP_NEWCATCH,
                  OP_FINDPROPSTRICT, OP_FINDPROPERTY, OP_FINDDEF, OP_GETLEX,
                  OP_SETPROPERTY, OP_GETLOCAL, OP_SETLOCAL,
                  OP_GETSCOPEOBJECT, OP_GETPROPERTY, OP_INITPROPERTY,
                  OP_DELETEPROPERTY, OP_GETSLOT, OP_SETSLOT,
                  OP_GETGLOBALSLOT, OP_SETGLOBALSLOT,
                  OP_COERCE, OP_ASTYPE, OP_ISTYPE,
                  OP_INCLOCAL, OP_DECLOCAL, OP_INCLOCAL_I, OP_DECLOCAL_I,
                  OP_GETDESCENDANTS,
                  OP_DEBUGLINE, OP_DEBUGFILE):
            _, p = _ru30(code, p)
            return p
        if op in (OP_CALL, OP_CONSTRUCT, OP_APPLYTYPE,
                  OP_NEWOBJECT, OP_NEWARRAY, OP_CONSTRUCTSUPER):
            _, p = _ru30(code, p)
            return p
        if op in (OP_CALLMETHOD, OP_CALLSTATIC, OP_CALLSUPER,
                  OP_CALLPROPERTY, OP_CONSTRUCTPROP, OP_CALLPROPLEX,
                  OP_CALLSUPERVOID, OP_CALLPROPVOID):
            _, p = _ru30(code, p)
            _, p = _ru30(code, p)
            return p
        if op == OP_HASNEXT2:
            _, p = _ru30(code, p)
            _, p = _ru30(code, p)
            return p
        if op == OP_DEBUG:
            p += 1
            _, p = _ru30(code, p)
            p += 1
            _, p = _ru30(code, p)
            return p
        return p
    except (IndexError, struct.error):
        # Malformed bytecode — stop iteration
        return len(code)


def _check_mn_ns_set(abc: ABCFile, mn_idx: int, result: list) -> None:
    """If multiname at mn_idx uses a namespace set, add package namespaces to result (preserving order)."""
    if mn_idx >= len(abc.multinames):
        return
    kind, data = abc.multinames[mn_idx]
    ns_set_idx = 0
    if kind in (MN_Multiname, MN_MultinameA) and data and len(data) >= 2:
        ns_set_idx = data[1]
    elif kind in (MN_MultinameL, MN_MultinameLA) and data:
        ns_set_idx = data[0]
    if ns_set_idx and ns_set_idx < len(abc.ns_sets):
        for ns_idx in abc.ns_sets[ns_set_idx]:
            if abc.ns_kind(ns_idx) == NS_PackageNamespace:
                ns = abc.ns_name(ns_idx)
                if ns and ns not in result:
                    result.append(ns)


def _check_typename_param(abc: ABCFile, mn_idx: int, result: list) -> None:
    """Check a TypeName parameter multiname and add its package to the wildcard list.

    Handles both QName params (single namespace) and Multiname params (namespace set).
    """
    if mn_idx >= len(abc.multinames):
        return
    kind, data = abc.multinames[mn_idx]
    # Nested TypeName — recurse into its params
    if kind == MN_TypeName and data:
        _qn, params = data
        for px in params:
            _check_typename_param(abc, px, result)
        return
    # QName/QNameA: extract the package from the single namespace
    if kind in (MN_QName, MN_QNameA) and data and len(data) >= 2:
        name_idx = data[1]
        name = abc.strings[name_idx] if name_idx < len(abc.strings) else ''
        if name and name[0].isupper():
            ns_idx = data[0]
            if ns_idx < len(abc.namespaces):
                if abc.ns_kind(ns_idx) == NS_PackageNamespace:
                    ns = abc.ns_name(ns_idx)
                    if ns and ns not in result:
                        result.append(ns)
        return
    # Multiname/MultinameA: delegate to the normal handler
    _check_mn_ns_set_typed(abc, mn_idx, result)


def _check_mn_ns_set_typed(abc: ABCFile, mn_idx: int, result: list) -> None:
    """Like _check_mn_ns_set but only for class-like names (starting with uppercase).

    This prevents property/method access multinames from polluting the wildcard
    import list with packages that aren't actually needed for type imports.
    """
    if mn_idx >= len(abc.multinames):
        return
    kind, data = abc.multinames[mn_idx]
    # For TypeName (e.g. Vector.<T>), recursively check parameter multinames.
    # TypeName params may be QNames with a single namespace — extract the package.
    if kind == MN_TypeName and data:
        _qn, params = data
        for px in params:
            _check_typename_param(abc, px, result)
        return
    # For Multiname/MultinameA we can check the name
    if kind in (MN_Multiname, MN_MultinameA) and data and len(data) >= 2:
        name_idx = data[0]
        name = abc.strings[name_idx] if name_idx < len(abc.strings) else ''
        if not name or not name[0].isupper():
            return  # Skip non-class names
        ns_set_idx = data[1]
    elif kind in (MN_MultinameL, MN_MultinameLA) and data:
        # Late-bound names — can't check the name, include for safety
        ns_set_idx = data[0]
    else:
        return
    if ns_set_idx and ns_set_idx < len(abc.ns_sets):
        for ns_idx in abc.ns_sets[ns_set_idx]:
            if abc.ns_kind(ns_idx) == NS_PackageNamespace:
                ns = abc.ns_name(ns_idx)
                if ns and ns not in result:
                    result.append(ns)



def _access_modifier(ns_kind: int) -> str:
    """Map namespace kind to AS3 access modifier."""
    if ns_kind == NS_PrivateNs:
        return 'private'
    if ns_kind == NS_ProtectedNs or ns_kind == NS_StaticProtectedNs:
        return 'protected'
    if ns_kind == NS_PackageInternalNs:
        return 'internal'
    return 'public'


