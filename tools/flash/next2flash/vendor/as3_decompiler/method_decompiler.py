"""Single-method AVM2 bytecode decompiler (stack simulation + control flow)."""

from __future__ import annotations

import logging
import re
import struct
from collections import defaultdict
from typing import Dict, List, Optional, Set, Tuple

log = logging.getLogger(__name__)

from .abc_parser import *
from .opcodes import *
from .helpers import *

__all__ = ['MethodDecompiler', '_GLOBAL_FUNCTIONS']

_MAX_STRUCT_DEPTH = 50  # recursion limit for _struct_block control-flow nesting

# ── Pre-compiled regex patterns (performance: eliminates 600k re._compile calls) ──

# Fixed patterns used in many places
_RE_LABEL_COLON = re.compile(r'^(__label_\d+):$')
_RE_LABEL_NUM_COLON = re.compile(r'^__label_(\d+):$')
_RE_LABEL_WS = re.compile(r'^\s*__label_\d+\s*:\s*$')
_RE_GOTO_LABEL = re.compile(r'^goto (__label_\d+);$')
_RE_GOTO_LABEL_BARE = re.compile(r'^goto __label_\d+;$')
_RE_IF_GOTO = re.compile(r'^if \((.+)\) goto (__label_\d+);$')
_RE_IF_CMP_GOTO = re.compile(r'^if \((.+?) (!==|===) (.+?)\) goto __label_\d+;$')
_RE_DEFAULT_GOTO = re.compile(r'^(?:default: )?goto (__label_(\d+));$')
_RE_CASE_GOTO = re.compile(r'^case \d+: goto __label_(\d+);$')
_RE_CASE_NUM_GOTO = re.compile(r'case (\d+): goto (__label_\d+);')
_RE_DEFAULT_GOTO2 = re.compile(r'default: goto (__label_\d+);')
_RE_EQ_MATCH = re.compile(r'^\((.+) (===?) (.+)\)$')
_RE_INC_DEC = re.compile(r'^(\w[\w.]*(?:\[.+?\])?) = (?:(?:int|uint)\()?\(\1 ([+-]) 1\)\)?;$')
_RE_VAR_LOCAL = re.compile(r'^var (_local_\d+):\S+ = (.+);$')
_RE_SIMPLE_IDENT = re.compile(r'^[a-zA-Z_][\w.]*$')
_RE_NEG_INT = re.compile(r'^-?\d+$')
_RE_WHILE_CLOSE = re.compile(r'^\} while \((.+)\);$')
_RE_LOOP_LABEL = re.compile(r'^(_loop_\d+:\s*)')
_RE_WHILE_COND = re.compile(r'^while \((.+)\)$')
_RE_WHILE_HASNEXT = re.compile(r'^while \(hasnext2\((\w+), (\w+)\)\)$')

# Pre-compiled _fold_compound_assign patterns (11 operators × 2 styles)
_COMPOUND_OPS = ('+', '-', '*', '/', '%', '&', '|', '^', '<<', '>>>', '>>')
_COMPOUND_PAT1 = {}  # op → compiled pattern for X = (X OP val);
_COMPOUND_PAT2 = {}  # op → compiled pattern for X = int((X OP val));
for _op in _COMPOUND_OPS:
    _esc = re.escape(_op)
    _COMPOUND_PAT1[_op] = re.compile(
        r'^(\w[\w.]*(?:\[.+?\])?) = \(\1 ' + _esc + r' (.+)\);$')
    _COMPOUND_PAT2[_op] = re.compile(
        r'^(\w[\w.]*(?:\[.+?\])?) = (?:int|uint)\(\(\1 ' + _esc + r' (.+)\)\);$')
del _op, _esc

# All conditional/unconditional branch opcodes (used by _prescan_branches
# and _prescan_local_types to detect control-flow edges).
_BRANCH_OPS = frozenset({
    OP_IFNLT, OP_IFNLE, OP_IFNGT, OP_IFNGE,
    OP_JUMP, OP_IFTRUE, OP_IFFALSE,
    OP_IFEQ, OP_IFNE, OP_IFLT, OP_IFLE,
    OP_IFGT, OP_IFGE, OP_IFSTRICTEQ, OP_IFSTRICTNE,
})

# ═══════════════════════════════════════════════════════════════════════════
#  Single-method decompiler (stack simulation + control flow)
# ═══════════════════════════════════════════════════════════════════════════

# AS3 global/top-level functions and type constructors that should NOT
# get a 'this.' prefix when the receiver is the implicit scope.
_GLOBAL_FUNCTIONS = frozenset({
    # Top-level functions (flash.utils, global)
    'trace', 'parseInt', 'parseFloat', 'isNaN', 'isFinite', 'isXMLName',
    'escape', 'unescape', 'encodeURI', 'encodeURIComponent',
    'decodeURI', 'decodeURIComponent',
    # Type-casting / constructor calls used as global functions
    'String', 'Number', 'int', 'uint', 'Boolean',
    'Array', 'Object', 'XML', 'XMLList', 'RegExp', 'Date', 'Vector',
    # Error hierarchy (commonly used as global constructor calls)
    'Error', 'TypeError', 'RangeError', 'ReferenceError',
    'ArgumentError', 'EvalError', 'URIError', 'SecurityError',
    'VerifyError', 'DefinitionError', 'SyntaxError', 'UninitializedError',
    # flash.utils top-level helpers
    'getDefinitionByName', 'getQualifiedClassName', 'getQualifiedSuperclassName',
    'getTimer', 'describeType', 'setTimeout', 'setInterval',
    'clearTimeout', 'clearInterval',
})


class _RunContext:
    """Mutable state bag for _run() dispatch handlers.

    Bundles all the local variables from _run() into a single object
    so that dispatch handler methods can read/write shared state.
    """
    def __init__(self):
        self.error_log: List[str] = []


class _EvalContext:
    """Lightweight state bag for _eval_branch() dispatch handlers.

    Used for pure expression evaluation in ternary detection.
    Sets self.bail = True when a side-effect or unhandled opcode is found.
    """
    pass


class MethodDecompiler:
    """Decompile a single AVM2 method body into AS3 source."""

    def __init__(self, abc: ABCFile):
        log.debug("MethodDecompiler.__init__: %d methods, %d bodies",
                  len(abc.methods), len(abc.method_bodies))
        self.abc = abc
        self._build_run_dispatch()

    def _build_run_dispatch(self):
        """Build opcode → handler dispatch table for _run()."""
        d = {}
        # Local variable ops
        for op in (OP_GETLOCAL_0, OP_GETLOCAL_1, OP_GETLOCAL_2, OP_GETLOCAL_3,
                   OP_GETLOCAL, OP_SETLOCAL_0, OP_SETLOCAL_1, OP_SETLOCAL_2, OP_SETLOCAL_3,
                   OP_SETLOCAL, OP_INCLOCAL, OP_INCLOCAL_I, OP_DECLOCAL, OP_DECLOCAL_I):
            d[op] = self._h_local_ops
        # Push constant ops
        for op in (OP_PUSHBYTE, OP_PUSHSHORT, OP_PUSHSTRING, OP_PUSHINT, OP_PUSHUINT,
                   OP_PUSHDOUBLE, OP_PUSHTRUE, OP_PUSHFALSE, OP_PUSHNULL,
                   OP_PUSHUNDEFINED, OP_PUSHNAN, OP_PUSHNAMESPACE):
            d[op] = self._h_push_ops
        # Scope ops
        for op in (OP_PUSHSCOPE, OP_POPSCOPE, OP_PUSHWITH, OP_GETSCOPEOBJECT,
                   OP_GETGLOBALSCOPE):
            d[op] = self._h_scope_ops
        # Property ops
        for op in (OP_GETPROPERTY, OP_SETPROPERTY, OP_INITPROPERTY, OP_DELETEPROPERTY,
                   OP_GETSLOT, OP_SETSLOT, OP_GETSUPER, OP_SETSUPER):
            d[op] = self._h_property_ops
        # Find ops
        for op in (OP_FINDPROPSTRICT, OP_FINDPROPERTY, OP_GETLEX):
            d[op] = self._h_find_ops
        # Call ops
        for op in (OP_CALLPROPERTY, OP_CALLPROPVOID, OP_CALLSUPER, OP_CALLSUPERVOID,
                   OP_CALLPROPLEX, OP_CALL, OP_CALLMETHOD, OP_CALLSTATIC):
            d[op] = self._h_call_ops
        # Construct ops
        for op in (OP_CONSTRUCT, OP_CONSTRUCTSUPER, OP_CONSTRUCTPROP):
            d[op] = self._h_construct_ops
        # Object/array creation ops
        for op in (OP_NEWOBJECT, OP_NEWARRAY, OP_NEWACTIVATION, OP_NEWFUNCTION,
                   OP_NEWCLASS, OP_NEWCATCH, OP_APPLYTYPE, OP_GETDESCENDANTS):
            d[op] = self._h_object_ops
        # Stack manipulation ops
        for op in (OP_POP, OP_DUP, OP_SWAP):
            d[op] = self._h_stack_ops
        # Coerce/type ops
        for op in (OP_CONVERT_S, OP_CONVERT_I, OP_CONVERT_U, OP_CONVERT_D, OP_CONVERT_B,
                   OP_CONVERT_O, OP_COERCE_A, OP_COERCE_S, OP_COERCE_B, OP_COERCE_D,
                   OP_COERCE_I, OP_COERCE_U, OP_COERCE_O, OP_COERCE,
                   OP_ASTYPE, OP_ASTYPELATE, OP_ISTYPE, OP_ISTYPELATE,
                   OP_INSTANCEOF, OP_TYPEOF, OP_CHECKFILTER,
                   OP_ESC_XELEM, OP_ESC_XATTR):
            d[op] = self._h_coerce_ops
        # Arithmetic ops
        for op in (OP_ADD, OP_ADD_I, OP_SUBTRACT, OP_SUBTRACT_I,
                   OP_MULTIPLY, OP_MULTIPLY_I, OP_DIVIDE, OP_MODULO,
                   OP_LSHIFT, OP_RSHIFT, OP_URSHIFT,
                   OP_BITAND, OP_BITOR, OP_BITXOR,
                   OP_NEGATE, OP_NEGATE_I, OP_NOT, OP_BITNOT,
                   OP_INCREMENT, OP_INCREMENT_I, OP_DECREMENT, OP_DECREMENT_I):
            d[op] = self._h_arithmetic_ops
        # Comparison ops
        for op in (OP_EQUALS, OP_STRICTEQUALS, OP_LESSTHAN, OP_LESSEQUALS,
                   OP_GREATERTHAN, OP_GREATEREQUALS, OP_IN):
            d[op] = self._h_comparison_ops
        # Branch/control-flow ops
        for op in (OP_RETURNVOID, OP_RETURNVALUE, OP_JUMP,
                   OP_IFTRUE, OP_IFFALSE,
                   OP_IFEQ, OP_IFNE, OP_IFLT, OP_IFLE, OP_IFGT, OP_IFGE,
                   OP_IFSTRICTEQ, OP_IFSTRICTNE,
                   OP_IFNLT, OP_IFNLE, OP_IFNGT, OP_IFNGE,
                   OP_LOOKUPSWITCH):
            d[op] = self._h_branch_ops
        # Iteration ops
        for op in (OP_NEXTNAME, OP_NEXTVALUE, OP_HASNEXT, OP_HASNEXT2):
            d[op] = self._h_iteration_ops
        # Misc ops
        for op in (OP_THROW, OP_KILL, OP_DXNS, OP_DXNSLATE):
            d[op] = self._h_misc_ops
        # Memory ops
        for op in (OP_LI8, OP_LI16, OP_LI32, OP_LF32, OP_LF64,
                   OP_SI8, OP_SI16, OP_SI32, OP_SF32, OP_SF64,
                   OP_SXI1, OP_SXI8, OP_SXI16):
            d[op] = self._h_memory_ops
        # Debug ops
        for op in (OP_DEBUG, OP_DEBUGLINE, OP_DEBUGFILE):
            d[op] = self._h_debug_ops
        # No-op opcodes
        for op in (OP_BKPT, OP_NOP, OP_LABEL):
            d[op] = self._h_nop
        # Global slot ops
        for op in (OP_GETGLOBALSLOT, OP_SETGLOBALSLOT, OP_FINDDEF):
            d[op] = self._h_global_slot_ops
        self._run_dispatch = d
        # Build eval dispatch table
        e = {}
        for op in (OP_PUSHBYTE, OP_PUSHSHORT, OP_PUSHSTRING, OP_PUSHINT, OP_PUSHUINT,
                   OP_PUSHDOUBLE, OP_PUSHTRUE, OP_PUSHFALSE, OP_PUSHNULL,
                   OP_PUSHUNDEFINED, OP_PUSHNAN, OP_PUSHNAMESPACE):
            e[op] = self._eh_push_ops
        for op in (OP_GETLOCAL_0, OP_GETLOCAL_1, OP_GETLOCAL_2, OP_GETLOCAL_3,
                   OP_GETLOCAL):
            e[op] = self._eh_local_ops
        for op in (OP_GETPROPERTY, OP_GETLEX, OP_GETSLOT):
            e[op] = self._eh_property_ops
        for op in (OP_FINDPROPSTRICT, OP_FINDPROPERTY):
            e[op] = self._eh_find_ops
        for op in (OP_COERCE_A, OP_COERCE_S, OP_CONVERT_S, OP_CONVERT_I,
                   OP_CONVERT_U, OP_CONVERT_D, OP_CONVERT_B, OP_CONVERT_O,
                   OP_COERCE, OP_ASTYPE):
            e[op] = self._eh_coerce_noop
        for op in (OP_ADD, OP_SUBTRACT, OP_MULTIPLY, OP_DIVIDE, OP_MODULO,
                   OP_NEGATE, OP_NEGATE_I, OP_NOT, OP_TYPEOF,
                   OP_BITOR, OP_BITAND, OP_BITXOR, OP_BITNOT,
                   OP_LSHIFT, OP_RSHIFT, OP_URSHIFT,
                   OP_INCREMENT, OP_INCREMENT_I, OP_DECREMENT, OP_DECREMENT_I):
            e[op] = self._eh_arithmetic_ops
        for op in (OP_EQUALS, OP_STRICTEQUALS, OP_LESSTHAN, OP_LESSEQUALS,
                   OP_GREATERTHAN, OP_GREATEREQUALS, OP_IN,
                   OP_INSTANCEOF, OP_ISTYPELATE, OP_ASTYPELATE):
            e[op] = self._eh_comparison_ops
        for op in (OP_NEWOBJECT, OP_NEWARRAY):
            e[op] = self._eh_object_ops
        for op in (OP_CALLPROPERTY, OP_CALLPROPLEX, OP_CALLMETHOD, OP_CALLSTATIC, OP_CALLSUPER):
            e[op] = self._eh_call_ops
        for op in (OP_CONSTRUCT, OP_CONSTRUCTPROP, OP_APPLYTYPE):
            e[op] = self._eh_construct_ops
        for op in (OP_GETDESCENDANTS,):
            e[op] = self._eh_property_ops
        for op in (OP_DUP, OP_SWAP, OP_POP):
            e[op] = self._eh_stack_ops
        for op in (OP_IFFALSE, OP_IFTRUE, OP_JUMP,
                   OP_IFEQ, OP_IFNE, OP_IFLT, OP_IFLE, OP_IFGT, OP_IFGE,
                   OP_IFSTRICTEQ, OP_IFSTRICTNE,
                   OP_IFNLT, OP_IFNLE, OP_IFNGT, OP_IFNGE):
            e[op] = self._eh_branch_ops
        # Scope and side-effect ops: bail out in eval mode
        for op in (OP_PUSHSCOPE, OP_POPSCOPE, OP_PUSHWITH, OP_GETSCOPEOBJECT,
                   OP_GETGLOBALSCOPE, OP_DXNS, OP_DXNSLATE):
            e[op] = self._eh_bail
        self._eval_dispatch = e

    def decompile(self, method_idx: int, indent: str = '    ', class_idx: int = -1,
                  is_static: bool = False, class_name: str = '') -> str:
        log.debug("decompile: method_idx=%d class_idx=%d", method_idx, class_idx)
        body = self.abc.method_bodies.get(method_idx)
        if not body:
            return f'{indent}// (no method body)\n'
        code = body.code
        try:
            stmts = self._run(code, body, method_idx, class_idx, is_static, class_name)
            stmts = self._fold_increments(stmts)
            # Fold compound assignments: X = (X + val) → X += val
            stmts = self._fold_compound_assign(stmts)
            # Fold inline assignments: var tmp = expr; this.prop = tmp; return tmp;
            # → return (this.prop = expr);
            stmts = self._fold_inline_assignment(stmts)
            # Combine consecutive if-gotos targeting the same label into && conditions
            stmts = self._fold_short_circuit_conditions(stmts)
            # Reconstruct try/catch blocks from exception info
            if body.exceptions:
                stmts = self._fold_try_catch(stmts, body, code)
            # Reconstruct switch/case from lookupswitch patterns
            stmts = self._fold_switch(stmts)
            # Post-process: structure control flow
            stmts = self._structure_flow(stmts)
            # Convert goto + do-while → while
            stmts = self._fold_goto_dowhile(stmts)
            # Convert while-with-init-and-step → for
            stmts = self._fold_while_to_for(stmts)
            # Reconstruct for-each / for-in from hasnext2 + nextvalue/nextname
            stmts = self._fold_for_each_in(stmts)
            # Reconstruct if/else-if chains from sequential if-return blocks
            stmts = self._fold_if_else_return_chains(stmts)
            # Fold new RegExp("pattern", "flags") → /pattern/flags
            stmts = self._fold_regexp_literals(stmts)
            # Strip redundant int()/uint() casts on assignments to typed variables
            stmts = self._fold_redundant_casts(stmts)
            
            # FINAL PASS: Remove any remaining malformed gotos (issue #25 final cleanup)
            # These are decompilation artifacts that couldn't be properly restructured
            final_stmts = []
            for line in stmts:
                stripped = line.strip()
                # Skip any line containing unresolved goto __label_
                if 'goto __label_' in stripped:
                    continue
                # Skip orphaned labels
                if _RE_LABEL_WS.match(stripped):
                    continue
                final_stmts.append(line)
            stmts = final_stmts

            # FINAL PASS 2: Remove stray 'break;' outside loop/switch contexts
            # These arise from try/catch mis-reconstruction where the try block
            # jump-over becomes 'break;' instead of being restructured.
            stmts = self._remove_stray_breaks(stmts)
            
        except (IndexError, ValueError, KeyError, AttributeError) as exc:
            stmts = [f'// decompile error: {exc}']
        lines = []
        for s in stmts:
            if s:
                # Expand multi-line expressions with proper indentation
                expanded = _expand_multiline_stmt(s, indent)
                lines.extend(expanded)
        return '\n'.join(lines) + '\n' if lines else ''

    def _run(self, code: bytes, body: MethodBody, method_idx: int = -1, class_idx: int = -1,
             is_static: bool = False, class_name: str = '') -> List[str]:
        abc = self.abc
        stmts: List[str] = []
        stack: List[str] = []
        scope: List[Tuple[str, str]] = []
        # In static methods, local0 is the class object; in instance methods, it's 'this'
        local0_name = class_name if (is_static and class_name) else 'this'
        local_names: Dict[int, str] = {0: local0_name}
        declared_locals: Set[int] = set()  # track which locals got 'var' declarations
        param_count = 0

        # Initialize param names from method info
        if 0 <= method_idx < len(abc.methods):
            m = abc.methods[method_idx]
            param_count = m.param_count
            for i in range(m.param_count):
                pname = ''
                if i < len(m.param_names):
                    pname = abc.strings[m.param_names[i]] if m.param_names[i] < len(abc.strings) else ''
                if not pname:
                    pname = f'_arg_{i+1}'
                local_names[i + 1] = pname

            # Register the rest parameter name (occupies register param_count + 1)
            if m.flags & METHOD_NeedRest:
                local_names[m.param_count + 1] = 'rest'

        # Build slot map for this class (slot_id -> trait_name)
        slot_map: Dict[int, str] = {}
        static_trait_names: Set[str] = set()  # static member names for self-qualification
        if 0 <= class_idx < len(abc.instances):
            for t in abc.instances[class_idx].traits:
                if t.kind in (TRAIT_Slot, TRAIT_Const) and t.slot_id:
                    slot_map[t.slot_id] = abc.mn_name(t.name_idx)
            for t in abc.classes[class_idx].traits:
                if t.kind in (TRAIT_Slot, TRAIT_Const) and t.slot_id:
                    slot_map[t.slot_id] = abc.mn_name(t.name_idx)
                # Collect static variable/const names (not methods) for self-qualification
                if t.kind in (TRAIT_Slot, TRAIT_Const):
                    static_trait_names.add(abc.mn_name(t.name_idx))

        # Build activation object slot map from method body traits
        # (used for methods with OP_NEWACTIVATION — closures, try/catch, with, etc.)
        activation_slots: Dict[int, str] = {}
        activation_slot_types: Dict[int, str] = {}
        for bt in body.traits:
            if bt.slot_id:
                activation_slots[bt.slot_id] = abc.mn_name(bt.name_idx)
                activation_slot_types[bt.slot_id] = abc.type_name(bt.type_name) if bt.type_name else '*'
        activation_reg: int = -1  # register holding the activation object
        declared_activation_slots: Set[int] = set()  # track which activation slots got var declarations

        p = 0
        targets: Set[int] = set()
        self._prescan_branches(code, targets)

        # Add exception table offsets to targets so they get labels
        for ex in body.exceptions:
            targets.add(ex.from_pos)
            targets.add(ex.to_pos)
            targets.add(ex.target)

        # Build catch handler entry point info (target offset → exception index + var name)
        catch_entry_info: Dict[int, Tuple[int, str]] = {}
        for ei_idx, ex in enumerate(body.exceptions):
            vn = abc.mn_name(ex.var_name) if ex.var_name else 'e'
            catch_entry_info[ex.target] = (ei_idx, vn)

        # Catch scope tracking: marker string → exception variable name
        catch_scope_vars: Dict[str, str] = {}

        # Pre-scan for local variable types (coerce → setlocal patterns)
        local_types: Dict[int, str] = self._prescan_local_types(code, body, abc)

        # Short-circuit && / || combine points: target_offset -> list of (operator, left_expr)
        logical_combines: Dict[int, list] = {}
        last_was_dup = False  # Track dup for dup+setlocal pattern


        # ═══ Create dispatch context ═══
        ctx = _RunContext()
        ctx.abc = abc
        ctx.code = code
        ctx.body = body
        ctx.method_idx = method_idx
        ctx.class_idx = class_idx
        ctx.is_static = is_static
        ctx.class_name = class_name
        ctx.stmts = stmts
        ctx.stack = stack
        ctx.scope = scope
        ctx.local0_name = local0_name
        ctx.local_names = local_names
        ctx.declared_locals = declared_locals
        ctx.param_count = param_count
        ctx.slot_map = slot_map
        ctx.static_trait_names = static_trait_names
        ctx.activation_slots = activation_slots
        ctx.activation_slot_types = activation_slot_types
        ctx.activation_reg = activation_reg
        ctx.declared_activation_slots = declared_activation_slots
        ctx.p = p
        ctx.targets = targets
        ctx.catch_entry_info = catch_entry_info
        ctx.catch_scope_vars = catch_scope_vars
        ctx.local_types = local_types
        ctx.logical_combines = logical_combines
        ctx.last_was_dup = last_was_dup
        ctx.was_dup = False

        while ctx.p < len(code):
            # Check for logical combine point (&&/|| target)
            if ctx.p in ctx.logical_combines and ctx.stack:
                entries = ctx.logical_combines.pop(ctx.p)
                right = ctx.stack[-1]
                # Apply in reverse order (innermost/most-recent first)
                for op_str, left in reversed(entries):
                    # Only wrap operands in parens when they contain a different
                    # logical operator at depth 0 (prevents unnecessary parens
                    # around simple comparisons like mode == Mode.XXX)
                    wl = _wrap_for_logical(left, op_str)
                    wr = _wrap_for_logical(right, op_str)
                    right = f'{wl} {op_str} {wr}'
                ctx.stack[-1] = right

            if ctx.p in ctx.targets and ctx.p > 0:
                ctx.stmts.append(f'__label_{ctx.p}:')

            # At catch handler entry points, AVM2 clears stack and pushes exception
            if ctx.p in ctx.catch_entry_info:
                _ei_idx, _ei_var = ctx.catch_entry_info[ctx.p]
                ctx.stack.clear()
                ctx.stack.append(_ei_var)
                ctx.scope.clear()  # AVM2 resets scope chain at exception handler entry

            op = code[ctx.p]; ctx.p += 1
            # Reset dup flag each iteration; transparent ops re-carry it
            ctx.was_dup = ctx.last_was_dup
            ctx.last_was_dup = False

            handler = self._run_dispatch.get(op)
            if handler:
                handler(op, ctx)
            else:
                ctx.stmts.append(f'// unknown opcode 0x{op:02X}')

        # Add any collected errors to the statement list as comments
        if ctx.error_log:
            ctx.stmts.append('')  # blank line for readability
            for error_msg in ctx.error_log:
                ctx.stmts.append(f'// ERROR: {error_msg}')

        return ctx.stmts

    # ═══════════════════════════════════════════════════════════════════════
    #  _run() opcode dispatch handlers
    # ═══════════════════════════════════════════════════════════════════════

    # ═══════════════════════════════════════════════════════════════════════
    #  _run() opcode handler methods — grouped by category
    # ═══════════════════════════════════════════════════════════════════════

    def _h_local_ops(self, op, ctx):
        """Handle OP_GETLOCAL*, OP_SETLOCAL*, OP_INCLOCAL*, OP_DECLOCAL*."""
        if op in (OP_GETLOCAL_0, OP_GETLOCAL_1, OP_GETLOCAL_2, OP_GETLOCAL_3):
            _reg = op - OP_GETLOCAL_0
            _default = 'this' if _reg == 0 else f'_local_{_reg}'
            _incdec = _match_local_incdec(ctx.code, ctx.p, _reg)
            if _incdec:
                _pre, _inc, ctx.p = _incdec
                _nm = ctx.local_names.get(_reg, _default)
                _ops = '++' if _inc else '--'
                ctx.stack.append(f'{_ops}{_nm}' if _pre else f'{_nm}{_ops}')
            else:
                ctx.stack.append(ctx.local_names.get(_reg, _default))
        elif op == OP_GETLOCAL:
            idx, ctx.p = _ru30(ctx.code, ctx.p)
            _incdec = _match_local_incdec(ctx.code, ctx.p, idx)
            if _incdec:
                _pre, _inc, ctx.p = _incdec
                _nm = ctx.local_names.get(idx, f'_local_{idx}')
                _ops = '++' if _inc else '--'
                ctx.stack.append(f'{_ops}{_nm}' if _pre else f'{_nm}{_ops}')
            else:
                ctx.stack.append(ctx.local_names.get(idx, f'_local_{idx}'))
        elif op in (OP_SETLOCAL_0, OP_SETLOCAL_1, OP_SETLOCAL_2, OP_SETLOCAL_3):
            self._do_setlocal(op - OP_SETLOCAL_0, ctx)
        elif op == OP_SETLOCAL:
            idx, ctx.p = _ru30(ctx.code, ctx.p)
            self._do_setlocal(idx, ctx)
        elif op in (OP_INCLOCAL, OP_INCLOCAL_I):
            idx, ctx.p = _ru30(ctx.code, ctx.p)
            nm = ctx.local_names.get(idx, f'_local_{idx}')
            ctx.stmts.append(f'{nm}++;')
        elif op in (OP_DECLOCAL, OP_DECLOCAL_I):
            idx, ctx.p = _ru30(ctx.code, ctx.p)
            nm = ctx.local_names.get(idx, f'_local_{idx}')
            ctx.stmts.append(f'{nm}--;')

    def _do_setlocal(self, reg, ctx):
        """Shared setlocal logic for both short (0-3) and long forms."""
        v = ctx.stack.pop() if ctx.stack else '?'
        # Detect storing activation object — suppress the var declaration
        if v == '__activation__' and ctx.activation_slots:
            ctx.activation_reg = reg
            ctx.local_names[reg] = '__activation__'
            ctx.last_was_dup = False
            return
        # Detect storing catch scope — suppress and track register
        if v.startswith('__catch_scope_') and v in ctx.catch_scope_vars:
            ctx.local_names[reg] = v
            ctx.last_was_dup = False
            return
        nm = ctx.local_names.get(reg, f'_local_{reg}')
        if reg not in ctx.local_names:
            ctx.local_names[reg] = nm
        # dup+setlocal pattern: replace remaining dup on stack with var name
        if ctx.was_dup and ctx.stack and ctx.stack[-1] == v and not _RE_SIMPLE_IDENT.match(v):
            ctx.stack[-1] = nm
        if reg > 0 and v != '':
            if reg not in ctx.declared_locals and reg > ctx.param_count:
                ctx.declared_locals.add(reg)
                ltype = ctx.local_types.get(reg, '*')
                v = _strip_redundant_cast(ltype, v)
                v = _add_type_cast_if_needed(ltype, v, ctx.local_types, ctx.local_names)
                # Append .0 for Number-typed locals with integer values
                if ltype == 'Number' and _RE_NEG_INT.match(v):
                    v += '.0'
                # Suppress default initializers that match the type's implicit default
                if _is_type_default(ltype, v):
                    ctx.stmts.append(f'var {nm}:{ltype};')
                else:
                    ctx.stmts.append(f'var {nm}:{ltype} = {v};')
            else:
                ctx.stmts.append(f'{nm} = {v};')

    def _h_push_ops(self, op, ctx):
        """Handle OP_PUSHBYTE through OP_PUSHNAMESPACE."""
        abc = ctx.abc
        if op == OP_PUSHBYTE:
            val = ctx.code[ctx.p]
            if val > 127: val -= 256
            ctx.p += 1
            ctx.stack.append(str(val))
        elif op == OP_PUSHSHORT:
            val, ctx.p = _ru30(ctx.code, ctx.p)
            if val >= 0x20000000: val -= 0x40000000
            ctx.stack.append(_fmt_int(val))
        elif op == OP_PUSHSTRING:
            idx, ctx.p = _ru30(ctx.code, ctx.p)
            s = abc.strings[idx] if idx < len(abc.strings) else '?'
            ctx.stack.append(f'"{_escape_str(s)}"')
        elif op == OP_PUSHINT:
            idx, ctx.p = _ru30(ctx.code, ctx.p)
            ctx.stack.append(_fmt_int(abc.integers[idx] if idx < len(abc.integers) else 0))
        elif op == OP_PUSHUINT:
            idx, ctx.p = _ru30(ctx.code, ctx.p)
            ctx.stack.append(_fmt_uint(abc.uintegers[idx] if idx < len(abc.uintegers) else 0))
        elif op == OP_PUSHDOUBLE:
            idx, ctx.p = _ru30(ctx.code, ctx.p)
            v = abc.doubles[idx] if idx < len(abc.doubles) else 0.0
            if v == int(v) and abs(v) < 1e15:
                iv = int(v)
                if iv >= 256 and iv == (iv & 0xFFFFFFFF):
                    ctx.stack.append(_fmt_hex(iv))
                else:
                    ctx.stack.append(str(iv))
            else:
                ctx.stack.append(f'{v:.15g}')
        elif op == OP_PUSHTRUE:
            ctx.stack.append('true')
        elif op == OP_PUSHFALSE:
            ctx.stack.append('false')
        elif op == OP_PUSHNULL:
            ctx.stack.append('null')
        elif op == OP_PUSHUNDEFINED:
            ctx.stack.append('undefined')
        elif op == OP_PUSHNAN:
            ctx.stack.append('NaN')
        elif op == OP_PUSHNAMESPACE:
            _, ctx.p = _ru30(ctx.code, ctx.p)
            ctx.stack.append('<namespace>')

    def _h_scope_ops(self, op, ctx):
        """Handle OP_PUSHSCOPE, OP_POPSCOPE, OP_PUSHWITH, OP_GETSCOPEOBJECT, OP_GETGLOBALSCOPE."""
        if op == OP_PUSHSCOPE:
            v = ctx.stack.pop() if ctx.stack else '?'
            if v.startswith('__catch_scope_'):
                ctx.scope.append(('catch', v))
            else:
                ctx.scope.append(('scope', v))
        elif op == OP_POPSCOPE:
            if ctx.scope:
                kind, val = ctx.scope.pop()
                if kind == 'with':
                    ctx.stmts.append('}')
                elif kind == 'catch' and val in ctx.catch_scope_vars:
                    del ctx.catch_scope_vars[val]
        elif op == OP_PUSHWITH:
            v = ctx.stack.pop() if ctx.stack else '?'
            ctx.scope.append(('with', v))
            ctx.stmts.append(f'with ({v})')
            ctx.stmts.append('{')
        elif op == OP_GETSCOPEOBJECT:
            idx, ctx.p = _ru30(ctx.code, ctx.p)
            if idx < len(ctx.scope):
                ctx.stack.append(ctx.scope[idx][1])
            elif ctx.class_name:
                # Scope tracking lost (e.g. after try/catch); use class name as
                # best-effort fallback — scope[0]=global, scope[1+]=class/activation.
                ctx.stack.append(ctx.class_name)
            else:
                ctx.stack.append(f'scope{idx}')
        elif op == OP_GETGLOBALSCOPE:
            ctx.stack.append('')

    def _h_property_ops(self, op, ctx):
        """Handle OP_GETPROPERTY, OP_SETPROPERTY, OP_INITPROPERTY, OP_DELETEPROPERTY,
        OP_GETSLOT, OP_SETSLOT, OP_GETSUPER, OP_SETSUPER."""
        abc = ctx.abc
        if op == OP_GETPROPERTY:
            mn, ctx.p = _ru30(ctx.code, ctx.p)
            rt_name = ctx.stack.pop() if (ctx.stack and abc.mn_needs_rt_name(mn)) else None
            rt_ns = ctx.stack.pop() if (ctx.stack and abc.mn_needs_rt_ns(mn)) else None
            obj = ctx.stack.pop() if ctx.stack else '?'
            is_attr = abc.mn_is_attr(mn)
            if rt_name is not None:
                if is_attr:
                    ctx.stack.append(f'{obj}.@[{rt_name}]')
                else:
                    ctx.stack.append(f'{obj}[{rt_name}]')
            else:
                name = abc.mn_name(mn)
                # E4X wildcard: empty name or '*' means all child elements
                if name == '' or name == '*':
                    name = '*'
                attr_prefix = '@' if is_attr else ''
                if obj in ('', 'global') or obj == name:
                    ctx.stack.append(f'{attr_prefix}{name}')
                elif obj == 'this' and name not in _GLOBAL_FUNCTIONS:
                    ctx.stack.append(f'this.{attr_prefix}{name}')
                elif obj == 'this':
                    ctx.stack.append(f'{attr_prefix}{name}')
                elif obj == ctx.local0_name and ctx.is_static:
                    # Own static scope — just use bare name
                    ctx.stack.append(f'{attr_prefix}{name}')
                else:
                    ctx.stack.append(f'{obj}.{attr_prefix}{name}')
        elif op == OP_SETPROPERTY:
            mn, ctx.p = _ru30(ctx.code, ctx.p)
            val = ctx.stack.pop() if ctx.stack else '?'
            if val.startswith('!('):
                val = f'({val})'
            rt_name = ctx.stack.pop() if (ctx.stack and abc.mn_needs_rt_name(mn)) else None
            rt_ns = ctx.stack.pop() if (ctx.stack and abc.mn_needs_rt_ns(mn)) else None
            obj = ctx.stack.pop() if ctx.stack else '?'
            if rt_name is not None:
                ctx.stmts.append(f'{obj}[{rt_name}] = {val};')
            else:
                name = abc.mn_name(mn)
                if obj in ('', 'global') or obj == name:
                    prop = name
                elif obj == 'this' and name not in _GLOBAL_FUNCTIONS:
                    prop = f'this.{name}'
                elif obj == 'this':
                    prop = name
                elif obj == ctx.local0_name and ctx.is_static:
                    prop = name
                else:
                    prop = f'{obj}.{name}'
                ctx.stmts.append(f'{prop} = {val};')
        elif op == OP_INITPROPERTY:
            mn, ctx.p = _ru30(ctx.code, ctx.p)
            val = ctx.stack.pop() if ctx.stack else '?'
            if val.startswith('!('):
                val = f'({val})'
            rt_name = ctx.stack.pop() if (ctx.stack and abc.mn_needs_rt_name(mn)) else None
            rt_ns = ctx.stack.pop() if (ctx.stack and abc.mn_needs_rt_ns(mn)) else None
            obj = ctx.stack.pop() if ctx.stack else '?'
            if rt_name is not None:
                ctx.stmts.append(f'{obj}[{rt_name}] = {val};')
            else:
                name = abc.mn_name(mn)
                if obj in ('', 'global') or obj == name:
                    prop = name
                elif obj == 'this' and name not in _GLOBAL_FUNCTIONS:
                    prop = f'this.{name}'
                elif obj == 'this':
                    prop = name
                elif obj == ctx.local0_name and ctx.is_static:
                    prop = name
                else:
                    prop = f'{obj}.{name}'
                ctx.stmts.append(f'{prop} = {val};')
        elif op == OP_DELETEPROPERTY:
            mn, ctx.p = _ru30(ctx.code, ctx.p)
            rt_name = ctx.stack.pop() if (ctx.stack and abc.mn_needs_rt_name(mn)) else None
            rt_ns = ctx.stack.pop() if (ctx.stack and abc.mn_needs_rt_ns(mn)) else None
            obj = ctx.stack.pop() if ctx.stack else '?'
            if rt_name is not None:
                ctx.stack.append(f'delete {obj}[{rt_name}]')
            else:
                name = abc.mn_name(mn)
                ctx.stack.append(f'delete {obj}.{name}' if obj != 'this' else f'delete {name}')
        elif op == OP_GETSLOT:
            idx, ctx.p = _ru30(ctx.code, ctx.p)
            obj = ctx.stack.pop() if ctx.stack else '?'
            if obj == '__activation__' and idx in ctx.activation_slots:
                ctx.stack.append(ctx.activation_slots[idx])
            elif obj in ctx.catch_scope_vars:
                ctx.stack.append(ctx.catch_scope_vars[obj])
            else:
                sname = ctx.slot_map.get(idx) if obj in ('this', '', 'global') or (ctx.is_static and obj == ctx.local0_name) or (ctx.class_name and obj == ctx.class_name) else None
                if sname:
                    if obj == 'this' and not ctx.is_static:
                        ctx.stack.append(f'this.{sname}')
                    else:
                        ctx.stack.append(sname)
                elif (obj in ('', 'global')) and ctx.class_name:
                    # Unresolved slot on global/empty scope — use class name as
                    # best-effort fallback (common for static self-references
                    # where getslot on the global scope refers to the class).
                    ctx.stack.append(ctx.class_name)
                else:
                    ctx.stack.append(f'{obj}.slot{idx}')
        elif op == OP_SETSLOT:
            idx, ctx.p = _ru30(ctx.code, ctx.p)
            val = ctx.stack.pop() if ctx.stack else '?'
            obj = ctx.stack.pop() if ctx.stack else '?'
            if obj == '__activation__' and idx in ctx.activation_slots:
                vname = ctx.activation_slots[idx]
                vtype = ctx.activation_slot_types.get(idx, '*')
                if idx not in ctx.declared_activation_slots:
                    ctx.declared_activation_slots.add(idx)
                    val = _strip_redundant_cast(vtype, val)
                    if _is_type_default(vtype, val):
                        ctx.stmts.append(f'var {vname}:{vtype};')
                    else:
                        ctx.stmts.append(f'var {vname}:{vtype} = {val};')
                else:
                    ctx.stmts.append(f'{vname} = {val};')
            elif obj in ctx.catch_scope_vars:
                pass
            else:
                sname = ctx.slot_map.get(idx) if obj in ('this', '', 'global') or (ctx.is_static and obj == ctx.local0_name) or (ctx.class_name and obj == ctx.class_name) else None
                if sname:
                    if obj == 'this' and not ctx.is_static:
                        ctx.stmts.append(f'this.{sname} = {val};')
                    else:
                        ctx.stmts.append(f'{sname} = {val};')
                else:
                    ctx.stmts.append(f'{obj}.slot{idx} = {val};')
        elif op == OP_GETSUPER:
            mn, ctx.p = _ru30(ctx.code, ctx.p)
            name = abc.mn_name(mn)
            _ = ctx.stack.pop() if ctx.stack else '?'
            ctx.stack.append(f'super.{name}')
        elif op == OP_SETSUPER:
            mn, ctx.p = _ru30(ctx.code, ctx.p)
            name = abc.mn_name(mn)
            val = ctx.stack.pop() if ctx.stack else '?'
            _ = ctx.stack.pop() if ctx.stack else '?'
            ctx.stmts.append(f'super.{name} = {val};')

    def _h_find_ops(self, op, ctx):
        """Handle OP_FINDPROPSTRICT, OP_FINDPROPERTY, OP_GETLEX."""
        abc = ctx.abc
        if op == OP_FINDPROPSTRICT:
            mn, ctx.p = _ru30(ctx.code, ctx.p)
            rt_name = ctx.stack.pop() if (ctx.stack and abc.mn_needs_rt_name(mn)) else None
            rt_ns = ctx.stack.pop() if (ctx.stack and abc.mn_needs_rt_ns(mn)) else None
            if rt_name is not None:
                ctx.stack.append(rt_name)
            else:
                name = abc.mn_name(mn)
                if ctx.is_static and ctx.class_name and name in ctx.static_trait_names:
                    # Own static member — push empty so getproperty/setproperty
                    # produces bare name (e.g. 'statesArr') not 'ClassName.statesArr'.
                    ctx.stack.append('')
                elif ctx.is_static and ctx.class_name and name == ctx.class_name:
                    # findpropstrict for the class itself (e.g. ClassName in cinit)
                    ctx.stack.append('')
                else:
                    ctx.stack.append(name)
        elif op == OP_FINDPROPERTY:
            mn, ctx.p = _ru30(ctx.code, ctx.p)
            rt_name = ctx.stack.pop() if (ctx.stack and abc.mn_needs_rt_name(mn)) else None
            rt_ns = ctx.stack.pop() if (ctx.stack and abc.mn_needs_rt_ns(mn)) else None
            if rt_name is not None:
                ctx.stack.append(rt_name)
            else:
                ctx.stack.append(abc.mn_name(mn))
        elif op == OP_GETLEX:
            mn, ctx.p = _ru30(ctx.code, ctx.p)
            ctx.stack.append(abc.mn_name(mn))

    def _h_call_ops(self, op, ctx):
        """Handle OP_CALLPROPERTY, OP_CALLPROPVOID, OP_CALLSUPER, OP_CALLSUPERVOID,
        OP_CALLPROPLEX, OP_CALL, OP_CALLMETHOD, OP_CALLSTATIC."""
        abc = ctx.abc
        if op == OP_CALLPROPERTY:
            mn, ctx.p = _ru30(ctx.code, ctx.p)
            argc, ctx.p = _ru30(ctx.code, ctx.p)
            args = _pop_n(ctx.stack, argc, ctx.error_log, f'0x{op:02X}')
            rt_name = ctx.stack.pop() if (ctx.stack and abc.mn_needs_rt_name(mn)) else None
            rt_ns = ctx.stack.pop() if (ctx.stack and abc.mn_needs_rt_ns(mn)) else None
            obj = ctx.stack.pop() if ctx.stack else '?'
            if rt_name is not None:
                ctx.stack.append(f'{obj}[{rt_name}]({", ".join(args)})')
            else:
                name = abc.mn_name(mn)
                ctx.stack.append(_fmt_call(obj, name, args))
        elif op == OP_CALLPROPVOID:
            mn, ctx.p = _ru30(ctx.code, ctx.p)
            argc, ctx.p = _ru30(ctx.code, ctx.p)
            args = _pop_n(ctx.stack, argc, ctx.error_log, f'0x{op:02X}')
            rt_name = ctx.stack.pop() if (ctx.stack and abc.mn_needs_rt_name(mn)) else None
            rt_ns = ctx.stack.pop() if (ctx.stack and abc.mn_needs_rt_ns(mn)) else None
            obj = ctx.stack.pop() if ctx.stack else '?'
            if rt_name is not None:
                ctx.stmts.append(f'{obj}[{rt_name}]({", ".join(args)});')
            else:
                name = abc.mn_name(mn)
                ctx.stmts.append(f'{_fmt_call(obj, name, args)};')
        elif op == OP_CALLSUPER:
            mn, ctx.p = _ru30(ctx.code, ctx.p)
            argc, ctx.p = _ru30(ctx.code, ctx.p)
            name = abc.mn_name(mn)
            args = _pop_n(ctx.stack, argc, ctx.error_log, f'0x{op:02X}')
            _ = ctx.stack.pop() if ctx.stack else '?'
            ctx.stack.append(f'super.{name}({", ".join(args)})')
        elif op == OP_CALLSUPERVOID:
            mn, ctx.p = _ru30(ctx.code, ctx.p)
            argc, ctx.p = _ru30(ctx.code, ctx.p)
            name = abc.mn_name(mn)
            args = _pop_n(ctx.stack, argc, ctx.error_log, f'0x{op:02X}')
            _ = ctx.stack.pop() if ctx.stack else '?'
            ctx.stmts.append(f'super.{name}({", ".join(args)});')
        elif op == OP_CALLPROPLEX:
            mn, ctx.p = _ru30(ctx.code, ctx.p)
            argc, ctx.p = _ru30(ctx.code, ctx.p)
            args = _pop_n(ctx.stack, argc, ctx.error_log, f'0x{op:02X}')
            rt_name = ctx.stack.pop() if (ctx.stack and abc.mn_needs_rt_name(mn)) else None
            rt_ns = ctx.stack.pop() if (ctx.stack and abc.mn_needs_rt_ns(mn)) else None
            obj = ctx.stack.pop() if ctx.stack else '?'
            if rt_name is not None:
                ctx.stack.append(f'{obj}[{rt_name}]({", ".join(args)})')
            else:
                name = abc.mn_name(mn)
                ctx.stack.append(_fmt_call(obj, name, args))
        elif op == OP_CALL:
            argc, ctx.p = _ru30(ctx.code, ctx.p)
            args = _pop_n(ctx.stack, argc, ctx.error_log, f'0x{op:02X}')
            func = ctx.stack.pop() if ctx.stack else '?'
            recv = ctx.stack.pop() if ctx.stack else '?'
            if func in ('', 'this', 'global'):
                ctx.stack.append(f'{recv}({", ".join(args)})')
            elif recv in ('', 'this', 'global') or recv == func:
                ctx.stack.append(f'{func}({", ".join(args)})')
            else:
                ctx.stack.append(f'{recv}.{func}({", ".join(args)})')
        elif op == OP_CALLMETHOD:
            disp, ctx.p = _ru30(ctx.code, ctx.p)
            argc, ctx.p = _ru30(ctx.code, ctx.p)
            args = _pop_n(ctx.stack, argc, ctx.error_log, f'0x{op:02X}')
            recv = ctx.stack.pop() if ctx.stack else '?'
            ctx.stack.append(f'{recv}.<method{disp}>({", ".join(args)})')
        elif op == OP_CALLSTATIC:
            mi, ctx.p = _ru30(ctx.code, ctx.p)
            argc, ctx.p = _ru30(ctx.code, ctx.p)
            args = _pop_n(ctx.stack, argc, ctx.error_log, f'0x{op:02X}')
            recv = ctx.stack.pop() if ctx.stack else '?'
            ctx.stack.append(f'{recv}.<static{mi}>({", ".join(args)})')

    def _h_construct_ops(self, op, ctx):
        """Handle OP_CONSTRUCT, OP_CONSTRUCTSUPER, OP_CONSTRUCTPROP."""
        abc = ctx.abc
        if op == OP_CONSTRUCT:
            argc, ctx.p = _ru30(ctx.code, ctx.p)
            args = _pop_n(ctx.stack, argc, ctx.error_log, f'0x{op:02X}')
            obj = ctx.stack.pop() if ctx.stack else '?'
            # When obj is a method call result (e.g. Foo.getClass(x)),
            # `new Foo.getClass(x)()` is invalid AS3. Split into temp var.
            if '(' in obj and obj.endswith(')') and not obj.startswith('new '):
                if not hasattr(ctx, '_construct_tmp_counter'):
                    ctx._construct_tmp_counter = 0
                ctx._construct_tmp_counter += 1
                tmp = f'_construct_cls_{ctx._construct_tmp_counter}'
                ctx.stmts.append(f'var {tmp}:Class = {obj};')
                ctx.stack.append(f'new {tmp}({", ".join(args)})')
            else:
                ctx.stack.append(f'new {obj}({", ".join(args)})')
        elif op == OP_CONSTRUCTSUPER:
            argc, ctx.p = _ru30(ctx.code, ctx.p)
            args = _pop_n(ctx.stack, argc, ctx.error_log, f'0x{op:02X}')
            _ = ctx.stack.pop() if ctx.stack else '?'
            ctx.stmts.append(f'super({", ".join(args)});')
        elif op == OP_CONSTRUCTPROP:
            mn, ctx.p = _ru30(ctx.code, ctx.p)
            argc, ctx.p = _ru30(ctx.code, ctx.p)
            args = _pop_n(ctx.stack, argc, ctx.error_log, f'0x{op:02X}')
            rt_name = ctx.stack.pop() if (ctx.stack and abc.mn_needs_rt_name(mn)) else None
            rt_ns = ctx.stack.pop() if (ctx.stack and abc.mn_needs_rt_ns(mn)) else None
            obj = ctx.stack.pop() if ctx.stack else '?'
            if rt_name is not None:
                ctx.stack.append(f'new {obj}[{rt_name}]({", ".join(args)})')
            else:
                name = abc.mn_name(mn)
                if obj == 'this' or obj == name:
                    ctx.stack.append(f'new {name}({", ".join(args)})')
                else:
                    ctx.stack.append(f'new {obj}.{name}({", ".join(args)})')

    def _h_object_ops(self, op, ctx):
        """Handle OP_NEWOBJECT, OP_NEWARRAY, OP_NEWACTIVATION, OP_NEWFUNCTION,
        OP_NEWCLASS, OP_NEWCATCH, OP_APPLYTYPE, OP_GETDESCENDANTS."""
        abc = ctx.abc
        if op == OP_NEWOBJECT:
            np2, ctx.p = _ru30(ctx.code, ctx.p)
            items = _pop_n(ctx.stack, np2 * 2, ctx.error_log, f'0x{op:02X}')
            pairs = [f'{items[i]}:{items[i+1]}' for i in range(0, len(items), 2)]
            if len(pairs) >= 2:
                inner = ',\n'.join(pairs)
                ctx.stack.append('{\n' + inner + '\n}')
            else:
                ctx.stack.append('{' + ', '.join(pairs) + '}')
        elif op == OP_NEWARRAY:
            count, ctx.p = _ru30(ctx.code, ctx.p)
            items = _pop_n(ctx.stack, count, ctx.error_log, f'0x{op:02X}')
            ctx.stack.append('[' + ', '.join(items) + ']')
        elif op == OP_NEWACTIVATION:
            ctx.stack.append('__activation__')
        elif op == OP_NEWFUNCTION:
            mi, ctx.p = _ru30(ctx.code, ctx.p)
            func_str = self._decompile_inline_function(mi)
            ctx.stack.append(func_str)
        elif op == OP_NEWCLASS:
            ci, ctx.p = _ru30(ctx.code, ctx.p)
            _ = ctx.stack.pop() if ctx.stack else '?'
            ctx.stack.append(f'<class#{ci}>')
        elif op == OP_NEWCATCH:
            idx, ctx.p = _ru30(ctx.code, ctx.p)
            marker = f'__catch_scope_{idx}__'
            if idx < len(ctx.body.exceptions):
                vn = ctx.body.exceptions[idx].var_name
                ctx.catch_scope_vars[marker] = abc.mn_name(vn) if vn else 'e'
            else:
                ctx.catch_scope_vars[marker] = 'e'
            ctx.stack.append(marker)
        elif op == OP_APPLYTYPE:
            argc, ctx.p = _ru30(ctx.code, ctx.p)
            args = _pop_n(ctx.stack, argc, ctx.error_log, f'0x{op:02X}')
            # In type parameter context, null represents * (the any type)
            args = ['*' if a == 'null' else a for a in args]
            base = ctx.stack.pop() if ctx.stack else '?'
            ctx.stack.append(f'{base}.<{", ".join(args)}>')
        elif op == OP_GETDESCENDANTS:
            mn, ctx.p = _ru30(ctx.code, ctx.p)
            rt_name = ctx.stack.pop() if (ctx.stack and abc.mn_needs_rt_name(mn)) else None
            rt_ns = ctx.stack.pop() if (ctx.stack and abc.mn_needs_rt_ns(mn)) else None
            obj = ctx.stack.pop() if ctx.stack else '?'
            if rt_name is not None:
                ctx.stack.append(f'{obj}..{rt_name}')
            else:
                name = abc.mn_name(mn)
                ctx.stack.append(f'{obj}..{name}')

    # Transparent opcodes that can appear between dup and iffalse/iftrue
    # in short-circuit &&/|| patterns without changing branch semantics.
    _SC_TRANSPARENT_OPS = frozenset({
        OP_CONVERT_B, OP_COERCE_A, OP_COERCE_B,
        OP_CONVERT_O, OP_COERCE_I, OP_COERCE_U, OP_COERCE_O,
    })

    def _h_stack_ops(self, op, ctx):
        """Handle OP_POP, OP_DUP, OP_SWAP."""
        if op == OP_POP:
            if ctx.stack:
                v = ctx.stack.pop()
                if ('(' in v or v.startswith('delete ') or '++' in v or '--' in v) and not v.startswith('"'):
                    ctx.stmts.append(f'{v};')
        elif op == OP_DUP:
            sc_detected = False
            # Look ahead past transparent opcodes (convert_b, coerce_a, etc.)
            # to find the iffalse/iftrue that indicates a short-circuit &&/|| pattern.
            look_p = ctx.p
            while look_p < len(ctx.code) and ctx.code[look_p] in self._SC_TRANSPARENT_OPS:
                look_p += 1
            if look_p < len(ctx.code) and ctx.code[look_p] in (OP_IFFALSE, OP_IFTRUE):
                next_op = ctx.code[look_p]
                off, p_after_branch = _rs24(ctx.code, look_p + 1)
                target = p_after_branch + off
                # Also skip transparent opcodes between iffalse/iftrue and pop
                pop_p = p_after_branch
                while pop_p < len(ctx.code) and ctx.code[pop_p] in self._SC_TRANSPARENT_OPS:
                    pop_p += 1
                if pop_p < len(ctx.code) and ctx.code[pop_p] == OP_POP:
                    sc_detected = True
                    op_str = '&&' if next_op == OP_IFFALSE else '||'
                    left = ctx.stack[-1] if ctx.stack else '?'
                    if target not in ctx.logical_combines:
                        ctx.logical_combines[target] = []
                    entries = ctx.logical_combines[target]
                    if entries and entries[-1][0] == op_str:
                        prev_op, prev_left = entries[-1]
                        wl = prev_left if prev_left.startswith('(') else f'({prev_left})'
                        wr = left if left.startswith('(') else f'({left})'
                        entries[-1] = (op_str, f'({wl} {prev_op} {wr})')
                    else:
                        entries.append((op_str, left))
                    if ctx.stack:
                        ctx.stack.pop()
                    ctx.p = pop_p + 1  # skip past all transparent ops + pop
            if not sc_detected:
                ctx.stack.append(ctx.stack[-1] if ctx.stack else '?')
                ctx.last_was_dup = True
        elif op == OP_SWAP:
            if len(ctx.stack) >= 2:
                ctx.stack[-1], ctx.stack[-2] = ctx.stack[-2], ctx.stack[-1]

    def _h_coerce_ops(self, op, ctx):
        """Handle type conversion and coercion opcodes."""
        abc = ctx.abc
        if op == OP_CONVERT_S:
            if ctx.stack and not ctx.stack[-1].startswith('"'):
                ctx.stack[-1] = f'String({ctx.stack[-1]})'
        elif op == OP_CONVERT_I:
            if ctx.stack and not ctx.stack[-1].lstrip('-').isdigit():
                ctx.stack[-1] = f'int({ctx.stack[-1]})'
        elif op == OP_CONVERT_U:
            if ctx.stack and not ctx.stack[-1].lstrip('-').isdigit():
                ctx.stack[-1] = f'uint({ctx.stack[-1]})'
        elif op == OP_CONVERT_D:
            if ctx.stack:
                v = ctx.stack[-1]
                if v.startswith('"') or v.startswith("'"):
                    ctx.stack[-1] = f'Number({v})'
            ctx.last_was_dup = ctx.was_dup
        elif op == OP_CONVERT_B:
            if ctx.stack:
                v = ctx.stack[-1]
                if v.lstrip('-').isdigit():
                    ctx.stack[-1] = f'Boolean({v})'
            ctx.last_was_dup = ctx.was_dup
        elif op == OP_COERCE_S:
            if ctx.stack:
                v = ctx.stack[-1]
                if v.lstrip('-').replace('.', '', 1).isdigit():
                    ctx.stack[-1] = f'String({v})'
            ctx.last_was_dup = ctx.was_dup
        elif op == OP_COERCE_B:
            if ctx.stack:
                v = ctx.stack[-1]
                if v.lstrip('-').isdigit():
                    ctx.stack[-1] = f'Boolean({v})'
            ctx.last_was_dup = ctx.was_dup
        elif op == OP_COERCE_D:
            if ctx.stack:
                v = ctx.stack[-1]
                if v.startswith('"') or v.startswith("'"):
                    ctx.stack[-1] = f'Number({v})'
            ctx.last_was_dup = ctx.was_dup
        elif op in (OP_CONVERT_O, OP_COERCE_A, OP_COERCE_I, OP_COERCE_U,
                    OP_COERCE_O, OP_CHECKFILTER):
            ctx.last_was_dup = ctx.was_dup
        elif op == OP_COERCE:
            _, ctx.p = _ru30(ctx.code, ctx.p)
            ctx.last_was_dup = ctx.was_dup
        elif op == OP_ASTYPE:
            mn, ctx.p = _ru30(ctx.code, ctx.p)
            name = abc.mn_name(mn)
            if ctx.stack:
                ctx.stack[-1] = f'({ctx.stack[-1]} as {name})'
        elif op == OP_ASTYPELATE:
            t = ctx.stack.pop() if ctx.stack else '?'
            v = ctx.stack.pop() if ctx.stack else '?'
            ctx.stack.append(f'({v} as {t})')
        elif op == OP_ISTYPE:
            mn, ctx.p = _ru30(ctx.code, ctx.p)
            name = abc.mn_name(mn)
            if ctx.stack:
                ctx.stack[-1] = f'({ctx.stack[-1]} is {name})'
        elif op == OP_ISTYPELATE:
            t = ctx.stack.pop() if ctx.stack else '?'
            v = ctx.stack.pop() if ctx.stack else '?'
            ctx.stack.append(f'({v} is {t})')
        elif op == OP_INSTANCEOF:
            t = ctx.stack.pop() if ctx.stack else '?'
            v = ctx.stack.pop() if ctx.stack else '?'
            ctx.stack.append(f'({v} instanceof {t})')
        elif op == OP_TYPEOF:
            if ctx.stack:
                ctx.stack[-1] = f'typeof({ctx.stack[-1]})'
        elif op in (OP_ESC_XELEM, OP_ESC_XATTR):
            pass

    def _h_arithmetic_ops(self, op, ctx):
        """Handle arithmetic, bitwise, NOT, increment/decrement opcodes."""
        stack = ctx.stack
        if op in (OP_ADD, OP_ADD_I):
            _binop(stack, '+')
        elif op in (OP_SUBTRACT, OP_SUBTRACT_I):
            _binop(stack, '-')
        elif op in (OP_MULTIPLY, OP_MULTIPLY_I):
            _binop(stack, '*')
        elif op == OP_DIVIDE:
            _binop(stack, '/')
        elif op == OP_MODULO:
            _binop(stack, '%')
        elif op == OP_LSHIFT:
            _binop(stack, '<<')
        elif op == OP_RSHIFT:
            _binop(stack, '>>')
        elif op == OP_URSHIFT:
            _binop(stack, '>>>')
        elif op == OP_BITAND:
            _bitwise_binop(stack, '&')
        elif op == OP_BITOR:
            _bitwise_binop(stack, '|')
        elif op == OP_BITXOR:
            _bitwise_binop(stack, '^')
        elif op in (OP_NEGATE, OP_NEGATE_I):
            if stack:
                v = stack[-1]
                if v.startswith('('):
                    stack[-1] = f'-{v}'
                else:
                    stack[-1] = f'-({v})'
        elif op == OP_NOT:
            if stack:
                val = stack[-1]
                _eq_match = _RE_EQ_MATCH.match(val)
                if _eq_match:
                    _left, _eqop, _right = _eq_match.groups()
                    _negop = '!==' if _eqop == '===' else '!='
                    stack[-1] = f'({_left} {_negop} {_right})'
                elif val.startswith('(') or ').' in val:
                    stack[-1] = f'!{val}'
                else:
                    stack[-1] = f'!({val})'
        elif op == OP_BITNOT:
            if stack: stack[-1] = f'(~({_to_hex_if_int(stack[-1])}))'
        elif op in (OP_INCREMENT, OP_INCREMENT_I):
            if stack: stack[-1] = f'({stack[-1]} + 1)'
        elif op in (OP_DECREMENT, OP_DECREMENT_I):
            if stack: stack[-1] = f'({stack[-1]} - 1)'

    def _h_comparison_ops(self, op, ctx):
        """Handle OP_EQUALS, OP_STRICTEQUALS, OP_LESSTHAN, OP_LESSEQUALS,
        OP_GREATERTHAN, OP_GREATEREQUALS, OP_IN."""
        stack = ctx.stack
        if op == OP_EQUALS:
            _binop(stack, '==')
        elif op == OP_STRICTEQUALS:
            _binop(stack, '===')
        elif op == OP_LESSTHAN:
            _binop(stack, '<')
        elif op == OP_LESSEQUALS:
            _binop(stack, '<=')
        elif op == OP_GREATERTHAN:
            _binop(stack, '>')
        elif op == OP_GREATEREQUALS:
            _binop(stack, '>=')
        elif op == OP_IN:
            name = stack.pop() if stack else '?'
            obj = stack.pop() if stack else '?'
            stack.append(f'({obj} in {name})')

    def _h_branch_ops(self, op, ctx):
        """Handle control flow: return, jump, if-branches, lookupswitch."""
        if op == OP_RETURNVOID:
            ctx.stmts.append('return;')
        elif op == OP_RETURNVALUE:
            val = ctx.stack.pop() if ctx.stack else '?'
            if _has_outer_parens(val):
                val = val[1:-1]
            ctx.stmts.append(f'return {val};')
        elif op == OP_JUMP:
            off, ctx.p = _rs24(ctx.code, ctx.p)
            target = ctx.p + off
            ctx.stmts.append(f'goto __label_{target};')
        elif op == OP_IFTRUE:
            off, ctx.p = _rs24(ctx.code, ctx.p)
            target = ctx.p + off
            cond = ctx.stack.pop() if ctx.stack else '?'
            # Ternary detection for OP_IFTRUE:
            # For iftrue, fall-through is when cond is FALSE, target is when cond is TRUE.
            # _try_ternary treats fall-through as true_val and target as false_val,
            # so we swap them: ternary is (cond) ? target_val : fallthrough_val.
            ternary_result = self._try_ternary(ctx.code, ctx.p, target, list(ctx.stack),
                                                ctx.local_names, ctx.abc, ctx.slot_map,
                                                ctx.local0_name, ctx.is_static, ctx.class_idx)
            if ternary_result is not None:
                fallthrough_val, target_val, end_pos = ternary_result
                c = cond if _has_outer_parens(cond) else f'({cond})'
                tv = f'({target_val})' if _needs_ternary_wrap(target_val) else target_val
                fv = f'({fallthrough_val})' if _needs_ternary_wrap(fallthrough_val) else fallthrough_val
                ctx.stack.append(f'({c} ? {tv} : {fv})')
                ctx.p = end_pos
            else:
                ctx.stmts.append(f'if ({cond}) goto __label_{target};')
        elif op == OP_IFFALSE:
            off, ctx.p = _rs24(ctx.code, ctx.p)
            target = ctx.p + off
            cond = ctx.stack.pop() if ctx.stack else '?'
            # Ternary detection
            ternary_result = self._try_ternary(ctx.code, ctx.p, target, list(ctx.stack),
                                                ctx.local_names, ctx.abc, ctx.slot_map,
                                                ctx.local0_name, ctx.is_static, ctx.class_idx)
            if ternary_result is not None:
                true_val, false_val, end_pos = ternary_result
                c = cond if _has_outer_parens(cond) else f'({cond})'
                tv = f'({true_val})' if _needs_ternary_wrap(true_val) else true_val
                fv = f'({false_val})' if _needs_ternary_wrap(false_val) else false_val
                ctx.stack.append(f'({c} ? {tv} : {fv})')
                ctx.p = end_pos
            else:
                ctx.stmts.append(f'if (!({cond})) goto __label_{target};')
        elif op in (OP_IFEQ, OP_IFNE, OP_IFLT, OP_IFLE, OP_IFGT, OP_IFGE,
                    OP_IFSTRICTEQ, OP_IFSTRICTNE,
                    OP_IFNLT, OP_IFNLE, OP_IFNGT, OP_IFNGE):
            off, ctx.p = _rs24(ctx.code, ctx.p)
            target = ctx.p + off
            b = ctx.stack.pop() if ctx.stack else '?'
            a = ctx.stack.pop() if ctx.stack else '?'
            op_map = {
                OP_IFEQ: '==', OP_IFNE: '!=', OP_IFLT: '<', OP_IFLE: '<=',
                OP_IFGT: '>', OP_IFGE: '>=', OP_IFSTRICTEQ: '===',
                OP_IFSTRICTNE: '!==', OP_IFNLT: '!<', OP_IFNLE: '!<=',
                OP_IFNGT: '!>', OP_IFNGE: '!>=',
            }
            not_cond_map = {
                OP_IFNGT: '>', OP_IFNLT: '<', OP_IFNLE: '<=', OP_IFNGE: '>=',
            }
            pos_neg_map = {
                OP_IFEQ: '!=', OP_IFNE: '==', OP_IFLT: '>=', OP_IFLE: '>',
                OP_IFGT: '<=', OP_IFGE: '<', OP_IFSTRICTEQ: '!==',
                OP_IFSTRICTNE: '===',
            }
            if op in not_cond_map and target > ctx.p:
                cond_str = f'{a} {not_cond_map[op]} {b}'
                ternary_result = self._try_ternary(ctx.code, ctx.p, target, list(ctx.stack),
                                                   ctx.local_names, ctx.abc, ctx.slot_map,
                                                   ctx.local0_name, ctx.is_static, ctx.class_idx)
                if ternary_result is not None:
                    true_val, false_val, end_pos = ternary_result
                    c = f'({cond_str})'
                    tv = f'({true_val})' if _needs_ternary_wrap(true_val) else true_val
                    fv = f'({false_val})' if _needs_ternary_wrap(false_val) else false_val
                    ctx.stack.append(f'({c} ? {tv} : {fv})')
                    ctx.p = end_pos
                    return
            elif op in pos_neg_map and target > ctx.p:
                cond_str = f'{a} {pos_neg_map[op]} {b}'
                ternary_result = self._try_ternary(ctx.code, ctx.p, target, list(ctx.stack),
                                                   ctx.local_names, ctx.abc, ctx.slot_map,
                                                   ctx.local0_name, ctx.is_static, ctx.class_idx)
                if ternary_result is not None:
                    true_val, false_val, end_pos = ternary_result
                    c = f'({cond_str})'
                    tv = f'({true_val})' if _needs_ternary_wrap(true_val) else true_val
                    fv = f'({false_val})' if _needs_ternary_wrap(false_val) else false_val
                    ctx.stack.append(f'({c} ? {tv} : {fv})')
                    ctx.p = end_pos
                    return
            ctx.stmts.append(f'if ({a} {op_map[op]} {b}) goto __label_{target};')
        elif op == OP_LOOKUPSWITCH:
            base = ctx.p - 1
            default_off, ctx.p = _rs24(ctx.code, ctx.p)
            case_count, ctx.p = _ru30(ctx.code, ctx.p)
            offsets = []
            for _ in range(case_count + 1):
                o, ctx.p = _rs24(ctx.code, ctx.p)
                offsets.append(o)
            val = ctx.stack.pop() if ctx.stack else '?'
            ctx.stmts.append(f'switch ({val}) {{')
            for i, o in enumerate(offsets):
                ctx.stmts.append(f'  case {i}: goto __label_{base + o};')
            ctx.stmts.append(f'  default: goto __label_{base + default_off};')
            ctx.stmts.append('}')

    def _h_iteration_ops(self, op, ctx):
        """Handle OP_NEXTNAME, OP_NEXTVALUE, OP_HASNEXT, OP_HASNEXT2."""
        if op == OP_NEXTNAME:
            idx = ctx.stack.pop() if ctx.stack else '?'
            obj = ctx.stack.pop() if ctx.stack else '?'
            ctx.stack.append(f'nextname({obj}, {idx})')
        elif op == OP_NEXTVALUE:
            idx = ctx.stack.pop() if ctx.stack else '?'
            obj = ctx.stack.pop() if ctx.stack else '?'
            ctx.stack.append(f'nextvalue({obj}, {idx})')
        elif op == OP_HASNEXT:
            idx = ctx.stack.pop() if ctx.stack else '?'
            obj = ctx.stack.pop() if ctx.stack else '?'
            ctx.stack.append(f'hasnext({obj}, {idx})')
        elif op == OP_HASNEXT2:
            obj_reg, ctx.p = _ru30(ctx.code, ctx.p)
            idx_reg, ctx.p = _ru30(ctx.code, ctx.p)
            ctx.stack.append(f'hasnext2({ctx.local_names.get(obj_reg, f"_local_{obj_reg}")}, {ctx.local_names.get(idx_reg, f"_local_{idx_reg}")})')

    def _h_misc_ops(self, op, ctx):
        """Handle OP_THROW, OP_KILL, OP_DXNS, OP_DXNSLATE."""
        if op == OP_THROW:
            val = ctx.stack.pop() if ctx.stack else '?'
            ctx.stmts.append(f'throw {val};')
        elif op == OP_KILL:
            idx, ctx.p = _ru30(ctx.code, ctx.p)
            if idx not in ctx.local_names or idx > (ctx.abc.methods[ctx.method_idx].param_count if 0 <= ctx.method_idx < len(ctx.abc.methods) else 0):
                ctx.local_names.pop(idx, None)
        elif op == OP_DXNS:
            _, ctx.p = _ru30(ctx.code, ctx.p)
        elif op == OP_DXNSLATE:
            if ctx.stack: ctx.stack.pop()

    def _h_memory_ops(self, op, ctx):
        """Handle memory load/store opcodes."""
        if op in (OP_LI8, OP_LI16, OP_LI32, OP_LF32, OP_LF64):
            addr = ctx.stack.pop() if ctx.stack else '?'
            names = {OP_LI8: 'li8', OP_LI16: 'li16', OP_LI32: 'li32',
                     OP_LF32: 'lf32', OP_LF64: 'lf64'}
            ctx.stack.append(f'{names[op]}({addr})')
        elif op in (OP_SI8, OP_SI16, OP_SI32, OP_SF32, OP_SF64):
            val = ctx.stack.pop() if ctx.stack else '?'
            addr = ctx.stack.pop() if ctx.stack else '?'
            names = {OP_SI8: 'si8', OP_SI16: 'si16', OP_SI32: 'si32',
                     OP_SF32: 'sf32', OP_SF64: 'sf64'}
            ctx.stmts.append(f'{names[op]}({val}, {addr});')
        elif op in (OP_SXI1, OP_SXI8, OP_SXI16):
            pass

    def _h_debug_ops(self, op, ctx):
        """Handle OP_DEBUG, OP_DEBUGLINE, OP_DEBUGFILE.
        
        OP_DEBUG with debug_type=1 (DI_LOCAL) maps a register to a variable name.
        We use this to recover original local variable names.
        """
        if op == OP_DEBUG:
            debug_type, ctx.p = _ru8(ctx.code, ctx.p)
            name_idx, ctx.p = _ru30(ctx.code, ctx.p)
            reg, ctx.p = _ru8(ctx.code, ctx.p)
            _, ctx.p = _ru30(ctx.code, ctx.p)
            # debug_type=1 → DI_LOCAL: register `reg` holds variable named strings[name_idx]
            if debug_type == 1 and name_idx < len(ctx.abc.strings):
                var_name = ctx.abc.strings[name_idx]
                if var_name and reg > ctx.param_count:
                    # Only set if not already a named parameter and name isn't already used
                    existing = ctx.local_names.get(reg)
                    if existing is None or existing.startswith('_local_'):
                        ctx.local_names[reg] = var_name
        elif op == OP_DEBUGLINE:
            _, ctx.p = _ru30(ctx.code, ctx.p)
        elif op == OP_DEBUGFILE:
            _, ctx.p = _ru30(ctx.code, ctx.p)

    def _h_nop(self, op, ctx):
        """Handle no-op opcodes: OP_BKPT, OP_NOP, OP_LABEL."""
        pass

    def _h_global_slot_ops(self, op, ctx):
        """Handle OP_GETGLOBALSLOT, OP_SETGLOBALSLOT, OP_FINDDEF."""
        abc = ctx.abc
        if op == OP_GETGLOBALSLOT:
            idx, ctx.p = _ru30(ctx.code, ctx.p)
            ctx.stack.append(f'globalSlot{idx}')
        elif op == OP_SETGLOBALSLOT:
            idx, ctx.p = _ru30(ctx.code, ctx.p)
            val = ctx.stack.pop() if ctx.stack else '?'
            ctx.stmts.append(f'globalSlot{idx} = {val};')
        elif op == OP_FINDDEF:
            mn, ctx.p = _ru30(ctx.code, ctx.p)
            ctx.stack.append(abc.mn_name(mn))

    def _method_signature_inline(self, mi: int) -> str:
        """Create a compact inline function signature."""
        if mi >= len(self.abc.methods):
            return f'(/*method#{mi}*/)'
        m = self.abc.methods[mi]
        params = []
        for i in range(m.param_count):
            pname = ''
            if i < len(m.param_names):
                pname = self.abc.strings[m.param_names[i]] if m.param_names[i] < len(self.abc.strings) else ''
            if not pname:
                pname = f'_arg_{i+1}'
            params.append(pname)
        if m.flags & METHOD_NeedRest:
            params.append('...rest')
        return f'({", ".join(params)})'

    def _decompile_inline_function(self, mi: int) -> str:
        """Decompile an anonymous/inline function with full body."""
        abc = self.abc
        if mi >= len(abc.methods):
            return f'function(/*method#{mi}*/)'
        m = abc.methods[mi]

        # Build parameter list with types and defaults
        params = []
        num_required = m.param_count - len(m.optional_values)
        for i in range(m.param_count):
            pname = ''
            if i < len(m.param_names):
                pname = abc.strings[m.param_names[i]] if m.param_names[i] < len(abc.strings) else ''
            if not pname:
                pname = f'_arg_{i+1}'
            ptype = abc.type_name(m.param_types[i]) if i < len(m.param_types) and m.param_types[i] else '*'
            param_str = f'{pname}:{ptype}'
            if i >= num_required:
                opt_idx = i - num_required
                if opt_idx < len(m.optional_values):
                    vkind, vindex = m.optional_values[opt_idx]
                    param_str += f'={abc.default_value_str(vkind, vindex)}'
            params.append(param_str)
        if m.flags & METHOD_NeedRest:
            params.append('...rest')

        # Return type
        rtype = abc.type_name(m.return_type) if m.return_type else '*'
        ret_str = f':{rtype}' if rtype else ''

        sig = f'function ({", ".join(params)}){ret_str}'

        # Try to decompile the body
        body = abc.method_bodies.get(mi)
        if not body:
            return sig

        try:
            stmts = self._run(body.code, body, mi)
            stmts = self._fold_increments(stmts)
            stmts = self._fold_compound_assign(stmts)
            stmts = self._fold_inline_assignment(stmts)
            stmts = self._fold_short_circuit_conditions(stmts)
            if body.exceptions:
                stmts = self._fold_try_catch(stmts, body, body.code)
            stmts = self._fold_switch(stmts)
            stmts = self._structure_flow(stmts)
            stmts = self._fold_goto_dowhile(stmts)
            stmts = self._fold_while_to_for(stmts)
            stmts = self._fold_for_each_in(stmts)
            stmts = self._fold_if_else_return_chains(stmts)
            stmts = self._fold_regexp_literals(stmts)
            stmts = self._fold_redundant_casts(stmts)
            # Remove stray break; outside loop/switch contexts
            stmts = self._remove_stray_breaks(stmts)
        except (IndexError, ValueError, KeyError, AttributeError):
            return sig

        # Format as multi-line inline function
        # Don't add indentation here — _expand_multiline_stmt handles it via brace tracking
        lines = [sig]
        lines.append('{')
        for s in stmts:
            if s:
                for sub in s.split('\n'):
                    lines.append(sub.lstrip(' '))
        lines.append('}')
        return '\n'.join(lines)

    # Regex for matching temp variable assignments (var declarations and bare)
    # Accepts any type annotation (e.g. :*, :int, :uint, :Number, etc.)
    _RE_TEMP_ASSIGN = re.compile(
        r'^(?:var )?(_local_\d+)(?::\S+)? = (.+);$')
    # Regex for matching (EXPR +/- 1) — possibly wrapped in int()/uint()
    _RE_INC_DEC_EXPR = re.compile(
        r'^(?:var )?(_local_\d+)(?::\S+)? = (?:(?:int|uint)\()?\((.+?) ([+-]) 1\)\)?;$')

    @staticmethod
    def _fold_increments(stmts: List[str]) -> List[str]:
        """Fold increment/decrement patterns into x++/x-- forms.

        Pattern 1 — 3-line property increment (any type annotation on temps):
            var VAR1:TYPE = OBJ;
            var VAR2:TYPE = (OBJ.PROP + 1);    # or int/uint wrapped
            VAR1.PROP = VAR2;
        → OBJ.PROP++;

        Pattern 2 — 4-line array element increment (separate index temp):
            VAR_OBJ = ARR;
            VAR_IDX = INDEX;
            VAR_VAL = (ARR[VAR_IDX] + 1);      # or int/uint wrapped
            VAR_OBJ[VAR_IDX] = VAR_VAL;
        → ARR[INDEX]++;

        Pattern 3 — single-stmt local increment:
            X = (X + 1);                        → X++;
            X = uint((X + 1));                  → X++;   (issue #10)
            X = int((X + 1));                   → X++;   (issue #10)
        """
        result = []
        i = 0
        while i < len(stmts):
            # ── Pattern 2: 4-line array element increment ──
            if i + 3 < len(stmts):
                s0 = stmts[i]
                s1 = stmts[i + 1]
                s2 = stmts[i + 2]
                s3 = stmts[i + 3]
                m0 = MethodDecompiler._RE_TEMP_ASSIGN.match(s0)
                m1 = MethodDecompiler._RE_TEMP_ASSIGN.match(s1)
                if m0 and m1:
                    var_obj = m0.group(1)
                    arr_expr = m0.group(2)
                    var_idx = m1.group(1)
                    idx_expr = m1.group(2)
                    m2 = MethodDecompiler._RE_INC_DEC_EXPR.match(s2)
                    if m2:
                        var_val = m2.group(1)
                        inc_expr = m2.group(2)
                        op = m2.group(3)
                        # Check s3: VAR_OBJ[VAR_IDX] = VAR_VAL;
                        m3 = re.match(
                            r'^' + re.escape(var_obj) + r'\[' + re.escape(var_idx) + r'\] = ' +
                            re.escape(var_val) + r';$', s3)
                        if m3:
                            expected = f'{arr_expr}[{var_idx}]'
                            if inc_expr == expected:
                                op_str = '++' if op == '+' else '--'
                                result.append(f'{arr_expr}[{idx_expr}]{op_str};')
                                i += 4
                                continue

            # ── Pattern 1: 3-line property/array increment ──
            if i + 2 < len(stmts):
                s0 = stmts[i]
                s1 = stmts[i + 1]
                s2 = stmts[i + 2]
                m0 = MethodDecompiler._RE_TEMP_ASSIGN.match(s0)
                if m0:
                    var1 = m0.group(1)
                    obj = m0.group(2)
                    m1 = MethodDecompiler._RE_INC_DEC_EXPR.match(s1)
                    if m1:
                        var2 = m1.group(1)
                        expr = m1.group(2)
                        op = m1.group(3)
                        # Match: VAR1.PROP = VAR2;  (property assignment)
                        m2 = re.match(r'^' + re.escape(var1) + r'\.(\w+) = ' + re.escape(var2) + r';$', s2)
                        if m2:
                            prop = m2.group(1)
                            expected_expr = f'{obj}.{prop}'
                            if expr == expected_expr:
                                op_str = '++' if op == '+' else '--'
                                result.append(f'{obj}.{prop}{op_str};')
                                i += 3
                                continue
                        # Match: VAR1[IDX] = VAR2;  (array element assignment)
                        m2b = re.match(r'^' + re.escape(var1) + r'\[(.+?)\] = ' + re.escape(var2) + r';$', s2)
                        if m2b:
                            idx_expr = m2b.group(1)
                            expected_expr = f'{obj}[{idx_expr}]'
                            if expr == expected_expr:
                                op_str = '++' if op == '+' else '--'
                                result.append(f'{obj}[{idx_expr}]{op_str};')
                                i += 3
                                continue

            # ── Pattern 3: single-stmt local increment ──
            s = stmts[i]
            # X = (X + 1); | X = uint((X + 1)); | X = int((X + 1));
            m_inc = _RE_INC_DEC.match(s)
            if m_inc:
                target = m_inc.group(1)
                op_str = '++' if m_inc.group(2) == '+' else '--'
                result.append(f'{target}{op_str};')
                i += 1
                continue
            result.append(stmts[i])
            i += 1
        return result

    @staticmethod
    def _fold_compound_assign(stmts: List[str]) -> List[str]:
        """Fold X = (X OP val) and X = TYPE((X OP val)) into compound assignments.

        Patterns:
            X = (X + val);                → X += val;
            X = int((X + val));           → X += val;
            X = uint((X + val));          → X += val;
            X = (X & val);               → X &= val;
            X = (X | val);               → X |= val;
            etc.

        Applies to all compound-assignable operators: + - * / % & | ^ << >> >>>
        Skips patterns already folded to ++ or --.
        """
        result = []
        for s in stmts:
            folded = False
            for op in _COMPOUND_OPS:
                # Pattern 1: X = (X OP val);
                m = _COMPOUND_PAT1[op].match(s)
                if m:
                    target = m.group(1)
                    val = m.group(2)
                    if op in ('+', '-') and val.strip() == '1':
                        break  # Leave for increment folding
                    result.append(f'{target} {op}= {val};')
                    folded = True
                    break
                # Pattern 2: X = int((X OP val));  or  X = uint((X OP val));
                m = _COMPOUND_PAT2[op].match(s)
                if m:
                    target = m.group(1)
                    val = m.group(2)
                    if op in ('+', '-') and val.strip() == '1':
                        break  # Leave for increment folding
                    result.append(f'{target} {op}= {val};')
                    folded = True
                    break
            if not folded:
                result.append(s)
        return result

    @staticmethod
    def _fold_regexp_literals(stmts: List[str]) -> List[str]:
        r"""Convert new RegExp("pattern", "flags") → /pattern/flags in statements.

        Only converts when the pattern string doesn't contain unescaped forward
        slashes (which would break the regex literal syntax).
        """
        def _replace_new_regexp(m: re.Match) -> str:
            pattern = m.group(1)
            flags = m.group(2) if m.group(2) is not None else ''
            # Unescape the string-form pattern: \\\\ → \\, \\" → "
            # In a string literal, \\ represents a single backslash.
            # In a regex literal, a single backslash is just \
            # So we convert \\d → \d, \\\\ → \\, etc.
            regex_pat = pattern.replace('\\\\', '\x00ESCAPE\x00')
            regex_pat = regex_pat.replace('\\', '')  # Remove single escaping
            regex_pat = regex_pat.replace('\x00ESCAPE\x00', '\\')  # Restore real backslashes
            # If the pattern contains unescaped /, don't convert
            if '/' in regex_pat:
                return m.group(0)
            return f'/{regex_pat}/{flags}'

        _REGEXP_PAT = re.compile(
            r'new RegExp\("((?:[^"\\]|\\.)*)"\s*(?:,\s*"([^"]*)")?\)')
        result = []
        for s in stmts:
            result.append(_REGEXP_PAT.sub(_replace_new_regexp, s))
        return result

    @staticmethod
    def _fold_redundant_casts(stmts: List[str]) -> List[str]:
        """Strip redundant int()/uint() casts on assignments to typed variables.

        The AVM2 compiler emits ``convert_i`` / ``convert_u`` opcodes when
        assigning to ``int`` / ``uint`` typed slots.  These produce explicit
        ``int(expr)`` / ``uint(expr)`` wrappers in the decompiled output, but
        the original AS3 source never has them because the assignment performs
        the coercion implicitly.

        Rules
        -----
        * ``X = int(expr);``  where *X* is ``:int``  → ``X = expr;``
        * ``X = uint(expr);`` where *X* is ``:uint`` → ``X = expr;``
        * ``var X:int = int(expr);``  → ``var X:int = expr;``
        * ``var X:uint = uint(expr);`` → ``var X:uint = expr;``
        * ``int(int(expr))`` / ``uint(uint(expr))`` → ``int(expr)`` / ``uint(expr)``
          (double-cast elimination, unconditional).

        Compound-assignment RHS (``+= int(expr)``) is **not** touched because
        the cast may convert an unknown-typed operand before the operation.
        """
        # -- Phase 1: build type map from var declarations -------------------
        _VAR_DECL = re.compile(
            r'var\s+(\w+)\s*:\s*(int|uint)\b')
        var_types: dict[str, str] = {}
        for s in stmts:
            for m in _VAR_DECL.finditer(s):
                var_types[m.group(1)] = m.group(2)

        # -- Phase 2: strip casts -------------------------------------------
        # Matches `= int(...)` or `= uint(...)` at end of assignment (but NOT +=, -=, etc.)
        _ASSIGN_CAST = re.compile(
            r'^(\s*(?:var\s+)?(\w+)\s*(?::\s*\w+\s*)?=\s*)'   # lhs + "="
            r'(int|uint)\((.+)\);$'                             # cast(expr);
        )
        # Double-cast anywhere: int(int(...)) or uint(uint(...))
        _DOUBLE_CAST = re.compile(r'\b(int|uint)\(\1\(')

        def _strip_double_cast(s: str) -> str:
            """Remove one layer of double-cast: int(int(expr)) → int(expr)."""
            while True:
                m = _DOUBLE_CAST.search(s)
                if not m:
                    break
                # m.start() is position of outer 'int(' / 'uint('
                # The inner cast starts at m.start() + len('int(') = m.end() - len('int(') ... 
                # Actually: m.group(0) is e.g. 'int(int(' and m.group(1) is 'int'
                outer_start = m.start()
                fn = m.group(1)
                # Find the matching ')' for the OUTER cast's '('
                open_pos = outer_start + len(fn)  # position of outer '('
                depth = 0
                close_pos = -1
                for i in range(open_pos, len(s)):
                    if s[i] == '(':
                        depth += 1
                    elif s[i] == ')':
                        depth -= 1
                        if depth == 0:
                            close_pos = i
                            break
                if close_pos == -1:
                    break  # unbalanced — bail
                # Inner content: everything between the outer '(' and outer ')'
                inner = s[open_pos + 1 : close_pos]
                # inner starts with "int(" or "uint(" — that's the inner cast, keep it
                s = s[:outer_start] + inner + s[close_pos + 1:]
            return s

        result: list[str] = []
        for s in stmts:
            # --- double-cast elimination (unconditional) ---
            s = _strip_double_cast(s)

            # --- assignment-level cast stripping ---
            m = _ASSIGN_CAST.match(s)
            if m:
                lhs = m.group(1)          # e.g. "  _local_1 = " or "  var _local_1:int = "
                var_name = m.group(2)      # e.g. "_local_1"
                cast_fn = m.group(3)       # "int" or "uint"
                inner = m.group(4)         # expression inside cast(...)
                # Verify the captured inner doesn't have unbalanced parens
                # (greedy `.+` may over-match when there's trailing content)
                depth = 0
                balanced = True
                for ch in inner:
                    if ch == '(':
                        depth += 1
                    elif ch == ')':
                        depth -= 1
                        if depth < 0:
                            balanced = False
                            break
                if not balanced or depth != 0:
                    result.append(s)
                    continue
                target_type = var_types.get(var_name)
                if target_type == cast_fn:
                    s = f'{lhs}{inner};'
            result.append(s)
        return result

    @staticmethod
    def _remove_stray_breaks(stmts: List[str]) -> List[str]:
        """Remove ``break;`` statements that appear outside any loop or switch.

        These arise when try/catch blocks are mis-reconstructed: the jump at the
        end of the try body (which should skip the catch handler) is emitted as
        ``break;`` when no enclosing loop/switch context exists.  Leaving them
        in causes mxmlc to report *"Target of break statement was not found"*.

        The approach: walk the statement list tracking a breakable scope depth
        (incremented on ``for``, ``for each``, ``while``, ``do``, ``switch``
        block openers, decremented on close).  Any ``break;`` at depth 0 is
        a stray and is removed.
        """
        _BREAK_KW = re.compile(
            r'^\s*(?:for\s*\(|for\s+each\s*\(|while\s*\(|do\s*$|do\s*\{|switch\s*\()')

        # Two-pass approach:
        # Pass 1: find indices of all lines that open a breakable scope
        #         (for/while/do/switch keywords)
        # Pass 2: track brace depth with a stack — each '{' pushed as
        #         breakable=True if it follows a breakable keyword, else False.
        #         On '}' pop.  A break; at breakable_depth==0 is stray.

        pending_breakable = False
        scope_stack: list[bool] = []  # True = breakable scope, False = not
        breakable_depth = 0
        result: list[str] = []

        for s in stmts:
            stripped = s.strip()

            # Check if this line opens a breakable scope
            if _BREAK_KW.match(stripped):
                pending_breakable = True

            # Count braces
            in_string = False
            string_char = ''
            i = 0
            while i < len(stripped):
                ch = stripped[i]
                if in_string:
                    if ch == '\\':
                        i += 1  # skip escaped char
                    elif ch == string_char:
                        in_string = False
                elif ch in ('"', "'"):
                    in_string = True
                    string_char = ch
                elif ch == '{':
                    is_brk = pending_breakable
                    scope_stack.append(is_brk)
                    if is_brk:
                        breakable_depth += 1
                    pending_breakable = False
                elif ch == '}':
                    if scope_stack:
                        was_brk = scope_stack.pop()
                        if was_brk:
                            breakable_depth -= 1
                    pending_breakable = False
                i += 1

            # Check for stray break
            if stripped == 'break;' and breakable_depth <= 0:
                continue  # remove stray break

            # Reset pending if we saw a non-brace line without opening
            if '{' not in stripped and '}' not in stripped:
                # Keep pending_breakable across blank/keyword-only lines
                # but reset if it's a regular statement
                if stripped and not _BREAK_KW.match(stripped) and stripped not in ('{', '}'):
                    # Only reset if this isn't part of the keyword continuation
                    # e.g.  "for (" on one line, "var i = 0; ..." on next
                    if not pending_breakable:
                        pass  # already not pending
                    # If the line is '{' it'll be handled above

            result.append(s)
        return result

    @staticmethod
    def _fold_short_circuit_conditions(stmts: List[str]) -> List[str]:
        """Combine consecutive if-gotos targeting the same label into compound && conditions.

        AVM2 compiles `if (A && B) { body }` as two separate branch instructions
        that both skip the body:
            if (!(A)) goto EXIT;
            if (!(B)) goto EXIT;
            // body
            EXIT:

        This pass combines them into a single compound condition:
            if (!((A) && (B))) goto EXIT;

        which _emit_if then negates to produce `if ((A) && (B)) { body }`.
        """
        result = []
        i = 0
        while i < len(stmts):
            s = stmts[i].strip()
            m = _RE_IF_GOTO.match(s)
            if m:
                target = m.group(2)
                conds = [m.group(1)]
                j = i + 1
                while j < len(stmts):
                    sj = stmts[j].strip()
                    mj = re.match(r'^if \((.+)\) goto ' + re.escape(target) + r';$', sj)
                    if mj:
                        conds.append(mj.group(1))
                        j += 1
                    else:
                        break
                if len(conds) > 1:
                    # Each condition skips the body when true; body runs when ALL are false.
                    # Body condition = NOT(C1) AND NOT(C2) AND ...
                    # Emit as: if (!(body_cond)) goto TARGET;
                    body_parts = []
                    for c in conds:
                        neg = MethodDecompiler._negate_cond(c)
                        # Wrap in parens if it contains spaces/operators to prevent ambiguity
                        if (' ' in neg and not _has_outer_parens(neg)
                                and '&&' not in neg and '||' not in neg):
                            body_parts.append(f'({neg})')
                        else:
                            body_parts.append(neg)
                    body_cond = ' && '.join(body_parts)
                    result.append(f'if (!({body_cond})) goto {target};')
                    i = j
                else:
                    result.append(stmts[i])
                    i += 1
            else:
                result.append(stmts[i])
                i += 1
        return result

    @staticmethod
    def _fold_inline_assignment(stmts: List[str]) -> List[str]:
        """Fold inline assignment patterns back into compact form.

        Pattern:
            var _local_N:TYPE = EXPR;
            TARGET = _local_N;
            return _local_N;
        → return (TARGET = EXPR);

        This handles the AVM2 pattern where `return (this.prop = expr)` is compiled
        as a temp variable + assignment + return.
        """
        result = []
        i = 0
        while i < len(stmts):
            if i + 2 < len(stmts):
                s0 = stmts[i]
                s1 = stmts[i + 1]
                s2 = stmts[i + 2]
                # Match: var _local_N:TYPE = EXPR;
                m0 = _RE_VAR_LOCAL.match(s0)
                if m0:
                    tmp_var = m0.group(1)
                    expr = m0.group(2)
                    # Match: TARGET = _local_N;
                    m1 = re.match(r'^(.+?) = ' + re.escape(tmp_var) + r';$', s1)
                    if m1:
                        target = m1.group(1)
                        # Match: return _local_N;
                        m2 = re.match(r'^return ' + re.escape(tmp_var) + r';$', s2)
                        if m2:
                            result.append(f'return ({target} = {expr});')
                            i += 3
                            continue
            result.append(stmts[i])
            i += 1
        return result

    def _fold_try_catch(self, stmts: List[str], body: 'MethodBody', code: bytes) -> List[str]:
        """Reconstruct try/catch/finally blocks using exception table and labels.
        
        Uses bytecode offsets (now mapped to labels) to find try body boundaries,
        catch handler starts, and merge points.
        """
        abc = self.abc
        if not body.exceptions:
            return stmts

        # Build label → statement index mapping
        label_pos: Dict[int, int] = {}
        for si, s in enumerate(stmts):
            m = _RE_LABEL_NUM_COLON.match(s.strip())
            if m:
                label_pos[int(m.group(1))] = si

        # Build exception info with resolved positions
        exc_info = []
        for ei_idx, ex in enumerate(body.exceptions):
            var_name = abc.mn_name(ex.var_name) if ex.var_name else 'e'
            exc_type = abc.type_name(ex.exc_type) if ex.exc_type else ''
            # Find merge point: JUMP at to_pos goes to the merge point after catches
            merge_offset = -1
            if ex.to_pos < len(code) and code[ex.to_pos] == OP_JUMP:
                off, _ = _rs24(code, ex.to_pos + 1)
                merge_offset = ex.to_pos + 4 + off
            exc_info.append({
                'idx': ei_idx, 'from': ex.from_pos, 'to': ex.to_pos,
                'target': ex.target, 'merge': merge_offset,
                'var': var_name, 'type': exc_type,
                'from_si': label_pos.get(ex.from_pos, -1),
                'to_si': label_pos.get(ex.to_pos, -1),
                'target_si': label_pos.get(ex.target, -1),
                'merge_si': label_pos.get(merge_offset, -1),
            })

        # Group exceptions by (from_pos, to_pos) → same try body
        try_groups: Dict[Tuple[int, int], List[dict]] = {}
        for ei in exc_info:
            key = (ei['from'], ei['to'])
            if key not in try_groups:
                try_groups[key] = []
            try_groups[key].append(ei)

        # Detect "finally" handlers vs catch-all catches.
        #
        # JPEXS-style heuristic: A catch-all (exc_type=0) is a *finally*
        # handler only when it wraps a broader range than sibling typed
        # catches — i.e. its (from, to) covers both the try body AND the
        # typed catch handlers.  A standalone catch-all with the same
        # range as (or no sibling) typed catches is a regular
        # ``catch(e:*)``.
        #
        # Additionally, a single catch-all is always treated as a regular
        # catch, never as finally.  In AVM2, finally is compiled as a
        # *pair* of handlers — one for the try body and one that covers
        # try+catch — so a single handler is never a finally.
        finally_map: Dict[Tuple[int, int], dict] = {}  # (from, to) → finally exception info
        regular_groups: Dict[Tuple[int, int], List[dict]] = {}

        # First, gather typed (non-catch-all) ranges so we can compare.
        typed_ranges: Set[Tuple[int, int]] = set()
        for key, group in try_groups.items():
            for ei in group:
                if ei['type']:
                    typed_ranges.add(key)

        for key, group in try_groups.items():
            typed_in_group = [ei for ei in group if ei['type']]
            catchall_in_group = [ei for ei in group if not ei['type']]

            # Add typed catches to regular_groups
            if typed_in_group:
                regular_groups[key] = typed_in_group

            for ei in catchall_in_group:
                # A catch-all is a finally if:
                #  1) There are typed catches with a DIFFERENT (narrower) range,
                #     AND this catch-all's range encompasses those typed catches'
                #     targets (i.e. it wraps try + catch).
                #  2) OR there are typed catches in the SAME group (same range)
                #     AND there exists another catch-all with a broader range.
                is_finally = False
                if typed_in_group:
                    # Same range as typed catches AND typed catches exist → this is
                    # a finally only if ANOTHER catch-all with a BROADER range also
                    # exists (two-handler finally pattern).
                    for other_key, other_group in try_groups.items():
                        if other_key == key:
                            continue
                        for other_ei in other_group:
                            if not other_ei['type']:
                                # Broader range covering our try body targets?
                                if other_key[0] <= key[0] and other_key[1] >= key[1]:
                                    is_finally = True
                                    break
                elif not typed_in_group:
                    # No typed catches in this group.  Check if a typed catch with
                    # a narrower range exists — if so, this catch-all wraps them
                    # (finally pattern).  Otherwise, it's a standalone catch(e:*).
                    for tkey in typed_ranges:
                        if key[0] <= tkey[0] and key[1] >= tkey[1] and key != tkey:
                            is_finally = True
                            break

                if is_finally:
                    finally_map[key] = ei
                else:
                    # Treat as a regular catch(e:*)
                    if key not in regular_groups:
                        regular_groups[key] = []
                    regular_groups[key].append(ei)

        # Build replacement regions: for each try/catch group, define the range of
        # statements to replace and the replacement content
        replacements = []  # list of (start_si, end_si, replacement_lines)

        for key, catches in regular_groups.items():
            from_pos, to_pos = key
            from_si = label_pos.get(from_pos, -1)
            to_si = label_pos.get(to_pos, -1)
            if from_si < 0 or to_si < 0:
                continue

            # Try body: statements from from_si+1 (after the from label) to to_si-1
            # The to_si label has a goto that skips past catches
            try_body = []
            for k in range(from_si + 1, to_si):
                try_body.append(stmts[k])

            # Collect catch handler info
            catch_blocks = []
            for ei in catches:
                target_si = ei['target_si']
                if target_si < 0:
                    continue
                var_name = ei['var']
                exc_type = ei['type']
                catch_clause = f'catch({var_name}:{exc_type})' if exc_type else f'catch({var_name})'

                # Catch body: from target_si+1 to the next catch target or merge point
                # Find the end of this catch handler
                next_targets = sorted([e['target_si'] for e in catches if e['target_si'] > target_si])
                # Also check for finally handler of the SAME try group
                for fkey, fei in finally_map.items():
                    if fkey[0] == from_pos and fei['target_si'] > target_si:
                        next_targets.append(fei['target_si'])
                next_targets.sort()

                if next_targets:
                    catch_end_si = next_targets[0]
                elif ei['merge_si'] >= 0:
                    catch_end_si = ei['merge_si']
                else:
                    # Fallback: find the merge label
                    catch_end_si = len(stmts)
                    for km in range(target_si + 1, len(stmts)):
                        ms = stmts[km].strip()
                        if _RE_GOTO_LABEL_BARE.match(ms):
                            # This goto + following label is the end of catch
                            catch_end_si = km + 1
                            break

                catch_body = []
                for k in range(target_si + 1, catch_end_si):
                    s = stmts[k].strip()
                    # Skip gotos that jump to the merge point (these are implicit breaks)
                    if _RE_GOTO_LABEL_BARE.match(s):
                        continue
                    # Skip labels
                    if s.endswith(':'):
                        continue
                    catch_body.append(stmts[k])

                catch_blocks.append((catch_clause, catch_body))

            # Find the overall end: the merge point after all catches
            all_catch_targets = [ei['target_si'] for ei in catches if ei['target_si'] >= 0]
            max_catch_target = max(all_catch_targets) if all_catch_targets else to_si
            merge_si = catches[0].get('merge_si', -1) if catches else -1

            # The region to replace: from from_si (the from label) to the merge label
            # Find the first merge label after all catches
            region_end_si = -1
            if merge_si >= 0:
                region_end_si = merge_si
            else:
                # Search for the merge label after the last catch
                for k in range(max_catch_target + 1, len(stmts)):
                    if _RE_LABEL_NUM_COLON.match(stmts[k].strip()):
                        region_end_si = k
                        break
            if region_end_si < 0:
                region_end_si = max_catch_target + 2  # fallback

            # Build replacement
            repl = []
            repl.append('try')
            repl.append('{')
            repl.extend(try_body)
            repl.append('}')
            for catch_clause, catch_body in catch_blocks:
                repl.append(catch_clause)
                repl.append('{')
                repl.extend(catch_body)
                repl.append('}')
            repl.append(';')

            # Check if there's a "finally" that wraps this try+catches
            # Finally handlers have from_pos == our from_pos but larger to_pos
            finally_block = None
            for fkey, fei in finally_map.items():
                if fkey[0] == from_pos and fkey[1] > to_pos:
                    finally_block = fei
                    break

            if finally_block:
                # The finally handler generates dispatch code between the merge point
                # and the continuation of normal execution. We need to cover it all.
                ftarget_si = finally_block['target_si']
                fmerge_si = finally_block.get('merge_si', -1)
                if ftarget_si >= 0:
                    # Find the continuation point: the farthest label referenced by
                    # the finally dispatch code
                    farthest = ftarget_si
                    for k in range(region_end_si, len(stmts)):
                        ms = stmts[k].strip()
                        # Look for labels and gotos in the finally mechanism area
                        gm = _RE_DEFAULT_GOTO.match(ms)
                        if gm:
                            target_off = int(gm.group(2))
                            tsi = label_pos.get(target_off, -1)
                            if tsi > farthest:
                                farthest = tsi
                        lm = _RE_LABEL_NUM_COLON.match(ms)
                        if lm:
                            lsi = k
                            if lsi > farthest:
                                break  # Past the finally mechanism
                        # Also check case gotos in switch blocks
                        cm = _RE_CASE_GOTO.match(ms)
                        if cm:
                            target_off = int(cm.group(1))
                            tsi = label_pos.get(target_off, -1)
                            if tsi > farthest:
                                farthest = tsi
                    region_end_si = farthest

            replacements.append((from_si, region_end_si, repl))

        if not replacements:
            return stmts

        # Sort by range size (smallest first = innermost first)
        replacements.sort(key=lambda r: (r[1] - r[0], r[0]))

        # Handle nesting: for overlapping replacements with the same start,
        # apply the inner replacement to the try body of the outer one.
        # Group by start position
        starts: Dict[int, List[tuple]] = {}
        for r in replacements:
            if r[0] not in starts:
                starts[r[0]] = []
            starts[r[0]].append(r)

        final_replacements = []
        for start_si, group in starts.items():
            if len(group) == 1:
                final_replacements.append(group[0])
            else:
                # Multiple replacements at the same start: nest inner into outer
                # Sort by range size (smallest = innermost first)
                group.sort(key=lambda r: r[1] - r[0])
                # The innermost becomes the try body content of the outermost
                inner = group[0]
                for outer_idx in range(1, len(group)):
                    outer = group[outer_idx]
                    # Rebuild outer with inner's replacement as the try body
                    outer_start, outer_end, outer_repl = outer
                    inner_start, inner_end, inner_repl = inner
                    # Replace the outer's try body with the inner's full replacement
                    # The outer's try body is between its 'try {' and the first '}'
                    new_repl = []
                    in_try_body = False
                    try_body_emitted = False
                    for line in outer_repl:
                        if line == '{' and not in_try_body and not try_body_emitted:
                            new_repl.append(line)
                            # Insert inner replacement as the try body
                            new_repl.extend(inner_repl)
                            in_try_body = True
                            try_body_emitted = True
                            continue
                        if in_try_body:
                            if line == '}':
                                in_try_body = False
                                new_repl.append(line)
                                continue
                            # Skip the outer's try body lines (replaced by inner)
                            continue
                        new_repl.append(line)
                    inner = (outer_start, outer_end, new_repl)
                final_replacements.append(inner)

        # Deduplicate and sort by start position
        final_replacements.sort(key=lambda r: r[0])

        # Apply replacements: build new statement list
        result = []
        skip_until = -1
        repl_by_start = {}
        for r in final_replacements:
            repl_by_start[r[0]] = r

        for idx in range(len(stmts)):
            if idx < skip_until:
                continue
            if idx in repl_by_start:
                start, end, repl = repl_by_start[idx]
                result.extend(repl)
                skip_until = end
            else:
                result.append(stmts[idx])

        return result

    @staticmethod
    def _fold_switch(stmts: List[str]) -> List[str]:
        """Reconstruct switch/case/break from lookupswitch + comparison chain patterns.

        The AVM2 lookupswitch pattern produces statements in this order:
        1. goto COMP_CHAIN;          (jump past case bodies to comparison chain)
        2. [case body labels and code]
        3. [comparison chain: if (VAL !== VAR) goto next; goto dispatch; ...]
        4. switch (N) { case 0: goto __label_X; ... }   (the lookupswitch)
        5. EXIT_LABEL:               (where break gotos point)

        This method detects this pattern and reconstructs proper switch/case/break.
        """
        # Build label position index
        label_pos: Dict[str, int] = {}
        for idx, s in enumerate(stmts):
            m = _RE_LABEL_COLON.match(s.strip())
            if m:
                label_pos[m.group(1)] = idx

        # First pass: find all switch blocks and mark their complete ranges
        switch_ranges = []
        for idx, s in enumerate(stmts):
            if not s.strip().startswith('switch ('):
                continue
            # Parse the lookupswitch block
            j = idx + 1
            case_targets: Dict[int, str] = {}
            default_label = None
            while j < len(stmts):
                cs = stmts[j].strip()
                j += 1
                if cs == '}':
                    break
                cm = _RE_CASE_NUM_GOTO.match(cs)
                if cm:
                    case_targets[int(cm.group(1))] = cm.group(2)
                dm = _RE_DEFAULT_GOTO2.match(cs)
                if dm:
                    default_label = dm.group(1)
            switch_block_end = j

            if not case_targets:
                continue

            all_case_labels = set(case_targets.values())
            if default_label:
                all_case_labels.add(default_label)

            # Check if case bodies are BEFORE the switch (typical lookupswitch pattern)
            body_positions = sorted(
                [label_pos[lbl] for lbl in all_case_labels if lbl in label_pos])
            if not body_positions or body_positions[0] >= idx:
                continue  # Bodies after switch — different pattern

            first_body_pos = body_positions[0]

            # Find the initial goto that jumps past case bodies to the comparison chain
            initial_goto_idx = None
            chain_label = None  # label the initial goto jumps to (comparison chain start)
            for k in range(first_body_pos - 1, -1, -1):
                cs = stmts[k].strip()
                mg = _RE_GOTO_LABEL.match(cs)
                if mg:
                    initial_goto_idx = k
                    chain_label = mg.group(1)
                    break
                if cs and not cs.endswith(':') and not cs.startswith('var '):
                    break

            # The comparison chain starts at the chain_label position
            chain_start_pos = label_pos.get(chain_label, idx) if chain_label else idx

            # Find the break/exit label
            break_label = None
            for k in range(switch_block_end, len(stmts)):
                ml = _RE_LABEL_COLON.match(stmts[k].strip())
                if ml:
                    break_label = ml.group(1)
                    break
                if stmts[k].strip():
                    break

            # Verify by checking most common goto target from case bodies
            goto_counts: Dict[str, int] = {}
            for k in range(first_body_pos, chain_start_pos):
                mg = _RE_GOTO_LABEL.match(stmts[k].strip())
                if mg and mg.group(1) not in all_case_labels:
                    tgt = mg.group(1)
                    # Don't count gotos to comparison chain  
                    if tgt != chain_label:
                        # Only count gotos to labels OUTSIDE the case body range
                        # to avoid nested switch break labels polluting the
                        # outer switch break detection.
                        tgt_pos = label_pos.get(tgt, -1)
                        if tgt_pos >= switch_block_end or tgt_pos < first_body_pos:
                            goto_counts[tgt] = goto_counts.get(tgt, 0) + 1
            if goto_counts:
                likely_break = max(goto_counts, key=goto_counts.get)
                if break_label is None or goto_counts.get(likely_break, 0) > goto_counts.get(break_label, 0):
                    break_label = likely_break

            # Extract the switch variable from the comparison chain
            switch_var = None
            case_values: Dict[int, str] = {}
            cmp_count = 0
            for k in range(chain_start_pos, idx):
                cs = stmts[k].strip()
                m_cmp = _RE_IF_CMP_GOTO.match(cs)
                if m_cmp:
                    val_str = m_cmp.group(1)
                    var_str = m_cmp.group(3)
                    if switch_var is None:
                        switch_var = var_str
                    case_values[cmp_count] = val_str
                    cmp_count += 1

            # Resolve temp var assignment: var _local_3:* = _arg_1;
            if switch_var:
                for k in range(chain_start_pos, idx):
                    cs = stmts[k].strip()
                    m_assign = re.match(
                        r'^var ' + re.escape(switch_var) + r':\* = (.+);$', cs)
                    if m_assign:
                        switch_var = m_assign.group(1)
                        break
            if switch_var is None:
                switch_var = '?'

            # Record this switch range
            range_start = initial_goto_idx if initial_goto_idx is not None else first_body_pos
            break_label_pos = label_pos.get(break_label, switch_block_end) if break_label else switch_block_end
            switch_ranges.append({
                'range_start': range_start,
                'first_body_pos': first_body_pos,
                'chain_start_pos': chain_start_pos,
                'switch_block_end': switch_block_end,
                'break_label': break_label,
                'break_label_pos': break_label_pos,
                'case_targets': case_targets,
                'default_label': default_label,
                'all_case_labels': all_case_labels,
                'switch_var': switch_var,
                'case_values': case_values,
            })

        if not switch_ranges:
            return stmts

        # Second pass: build output, replacing switch ranges
        result: List[str] = []
        skip_until = -1
        for idx in range(len(stmts)):
            if idx < skip_until:
                continue

            # Check if this position starts a switch range
            sw = None
            for sr in switch_ranges:
                if idx == sr['range_start']:
                    sw = sr
                    break
            if sw is None:
                result.append(stmts[idx])
                continue

            # Emit reconstructed switch
            case_targets = sw['case_targets']
            default_label = sw['default_label']
            all_case_labels = sw['all_case_labels']
            switch_var = sw['switch_var']
            case_values = sw['case_values']
            break_label = sw['break_label']
            chain_start = sw['chain_start_pos']

            # Group case indices by target label
            label_to_cases: Dict[str, List[int]] = {}
            for ci2, lbl in case_targets.items():
                label_to_cases.setdefault(lbl, []).append(ci2)

            # Sort unique targets by their position
            sorted_targets = sorted(
                [(label_pos.get(lbl, 9999), lbl) for lbl in all_case_labels])

            result.append(f'switch ({switch_var})')
            result.append('{')

            processed = set()
            for tidx, (bpos, blabel) in enumerate(sorted_targets):
                if blabel in processed:
                    continue
                processed.add(blabel)

                # Emit case labels for this target
                cases = label_to_cases.get(blabel, [])
                for ci2 in sorted(cases):
                    val = case_values.get(ci2, str(ci2))
                    result.append(f'{INDENT_UNIT}case {val}:')
                if blabel == default_label:
                    result.append(f'{INDENT_UNIT}default:')

                # Find case body range: from label+1 to next case label or chain start
                body_start = bpos + 1
                body_end = chain_start  # Default: stop at comparison chain
                for bpos2, _ in sorted_targets:
                    if bpos2 > bpos:
                        body_end = bpos2
                        break

                # Collect body statements
                has_break = False
                for k in range(body_start, body_end):
                    cs = stmts[k].strip()
                    if not cs:
                        continue
                    if cs.endswith(':'):
                        continue  # skip labels
                    if break_label and cs == f'goto {break_label};':
                        has_break = True
                        continue
                    # Skip gotos to case labels (fall-through markers)
                    mg = _RE_GOTO_LABEL.match(cs)
                    if mg and mg.group(1) in all_case_labels:
                        continue
                    result.append(f'{INDENT_UNIT * 2}{cs}')
                if has_break:
                    result.append(f'{INDENT_UNIT * 2}break;')

            result.append('}')

            # Skip everything up to (and including) the break label
            skip_until = sw['break_label_pos'] + 1 if sw['break_label_pos'] < len(stmts) else sw['switch_block_end']

            # Preserve break label if gotos outside the switch range reference it
            if break_label and sw['break_label_pos'] < len(stmts):
                blab = break_label
                range_s = sw['range_start']
                range_e = skip_until
                for ext_idx, ext_s in enumerate(stmts):
                    if ext_idx >= range_s and ext_idx < range_e:
                        continue
                    if f'goto {blab};' in ext_s:
                        result.append(f'{blab}:')
                        break

        return result

    @staticmethod
    def _fold_if_else_return_chains(stmts: List[str]) -> List[str]:
        """Reconstruct if/else-if chains from sequential if-return blocks.

        When an if-block ends with return/throw, a following if at the same
        level is semantically equivalent to else-if.  Converts:
            if (cond1) { return x; };
            if (cond2) { return y; };
            return z;
        Into:
            if (cond1) { return x; }
            else if (cond2) { return y; }
            else { return z; };
        """
        result = list(stmts)

        def _block_ends_with_return(end_idx: int) -> bool:
            """Check if the block ending at end_idx has return/throw as last real stmt."""
            for k in range(end_idx - 1, -1, -1):
                prev = result[k].strip()
                if prev and prev != '{' and not prev.startswith('//'):
                    return (prev.startswith('return ') or prev.startswith('return(') or
                            prev == 'return;' or prev.startswith('throw '))
            return False

        # First pass: convert }; + if → } else if when block ends with return/throw
        i = 0
        while i < len(result):
            s = result[i].strip()
            if s == '};' and i + 1 < len(result):
                next_s = result[i + 1].strip()
                if next_s.startswith('if (') and _block_ends_with_return(i):
                    # Only chain if at the same indentation level to avoid
                    # cross-nesting inner ifs with outer else-if blocks
                    indent1 = len(result[i]) - len(result[i].lstrip())
                    indent2 = len(result[i + 1]) - len(result[i + 1].lstrip())
                    if indent1 == indent2:
                        result[i] = result[i].replace('};', '}')
                        result.insert(i + 1, 'else')
            i += 1

        # Second pass: wrap trailing return/throw in else { } after if-return chain
        i = 0
        while i < len(result):
            s = result[i].strip()
            if s == '};' and i + 1 < len(result):
                next_s = result[i + 1].strip()
                if ((next_s.startswith('return ') or next_s.startswith('return(')
                        or next_s == 'return;' or next_s.startswith('throw '))
                        and _block_ends_with_return(i)):
                    # Only chain at the same indentation level
                    indent1 = len(result[i]) - len(result[i].lstrip())
                    indent2 = len(result[i + 1]) - len(result[i + 1].lstrip())
                    if indent1 != indent2:
                        i += 1
                        continue
                    # Check that this is part of an if/else chain (look back for 'else')
                    in_chain = False
                    for k in range(i - 1, max(i - 30, -1), -1):
                        pk = result[k].strip()
                        if pk == 'else':
                            in_chain = True
                            break
                        if pk == '{' or pk == '};' or pk.startswith('if ('):
                            continue
                        if pk.startswith('return ') or pk.startswith('return(') or pk.startswith('throw '):
                            continue
                        break
                    if in_chain:
                        result[i] = result[i].replace('};', '}')
                        result.insert(i + 1, 'else')
                        result.insert(i + 2, '{')
                        # Find the return/throw statement (now at i+3)
                        # Add closing }; after it
                        ret_idx = i + 3
                        result.insert(ret_idx + 1, '};')
            i += 1

        return result

    @staticmethod
    def _fold_goto_dowhile(stmts: List[str]) -> List[str]:
        """Convert 'goto __label_N; do { ... } while (cond);' → 'while (cond) { ... };'"""
        result: List[str] = []
        i = 0
        while i < len(stmts):
            s = stmts[i].strip()
            # Look for: goto __label_N; followed by do { ... } while(...);
            if _RE_GOTO_LABEL_BARE.match(s) and i + 1 < len(stmts) and stmts[i + 1].strip() == 'do':
                do_line = stmts[i + 1]
                indent = do_line[:len(do_line) - len(do_line.lstrip())]
                # Find matching } while (cond);
                j = i + 2
                if j < len(stmts) and stmts[j].strip() == '{':
                    depth = 1
                    j += 1
                    while j < len(stmts) and depth > 0:
                        line = stmts[j].strip()
                        if line == '{':
                            depth += 1
                        elif line.startswith('} while (') or line == '}':
                            depth -= 1
                        j += 1
                    # j now points past the closing } while (cond);
                    close_line = stmts[j - 1].strip()
                    m_close = _RE_WHILE_CLOSE.match(close_line)
                    if m_close:
                        cond = m_close.group(1)
                        result.append(f'{indent}while ({cond})')
                        result.append(f'{indent}{{')
                        # Body = stmts[i+3 : j-1] (between { and } while)
                        for k in range(i + 3, j - 1):
                            result.append(stmts[k])
                        result.append(f'{indent}}};')
                        i = j
                        continue
            result.append(stmts[i])
            i += 1
        return result

    @staticmethod
    def _fold_while_to_for(stmts: List[str]) -> List[str]:
        """Convert 'var X = init; while (cond) { ...; X++; }' → 'for (var X = init; cond; X++) { ... }'.

        Detects the init-test-increment pattern that the compiler generates for
        ``for`` loops and rewrites them back.  Handles nested for loops,
        ``X++``, ``X--``, ``X += N``, and ``X = X + N`` step forms.
        Skips ``while (true)`` and ``do … while`` loops.

        **Extended**: When the init statement is not immediately before the
        ``while``, scans backwards through preceding ``var`` declarations to
        find the loop variable's initializer (e.g., ``var i:int;`` followed by
        ``var sum:int;`` followed by ``while (i < 10)``).
        """
        result: List[str] = []
        i = 0
        while i < len(stmts):
            # ── Try to match: … init_stmt ; [other vars] ; while (cond) { body… step; }; ──
            matched = False
            s_stripped = stmts[i].strip()

            # Strip optional loop label  (_loop_N: while (...))
            loop_label_prefix = ''
            mw_label = _RE_LOOP_LABEL.match(s_stripped)
            if mw_label:
                loop_label_prefix = mw_label.group(1)
                while_core = s_stripped[mw_label.end():]
            else:
                while_core = s_stripped

            m_while = _RE_WHILE_COND.match(while_core)
            if m_while and while_core != 'while (true)':
                cond = m_while.group(1)

                # Verify next stmt is '{'
                if i + 1 < len(stmts) and stmts[i + 1].strip() == '{':
                    # Find matching '};'
                    depth = 1
                    j = i + 2
                    while j < len(stmts) and depth > 0:
                        depth += MethodDecompiler._count_net_braces(stmts[j])
                        j += 1
                    close_idx = j - 1  # index of };

                    if depth == 0 and close_idx > i + 2:
                        # Last body statement (before };)
                        last_body_idx = close_idx - 1
                        last_s = stmts[last_body_idx].strip()

                        # Try to match step in last body statement for each
                        # candidate init variable found by scanning backwards.
                        init_info = MethodDecompiler._find_for_init(
                            result, cond, last_s)

                        if init_info:
                            var_name, init_expr, remove_idx = init_info
                            step_expr = MethodDecompiler._match_step(
                                var_name, last_s)

                            if step_expr:
                                # Remove the init statement from result
                                if remove_idx is not None:
                                    del result[remove_idx]
                                # Build the for statement
                                for_line = (f'{loop_label_prefix}'
                                            f'for ({init_expr}; {cond}; {step_expr})')
                                result.append(for_line)
                                result.append('{')
                                # Body = everything between { and last_body_stmt (exclusive)
                                for k in range(i + 2, last_body_idx):
                                    result.append(stmts[k])
                                result.append(stmts[close_idx])  # }; or }
                                i = close_idx + 1
                                matched = True

            if not matched:
                result.append(stmts[i])
                i += 1

        # Recurse into nested blocks: re-process body of for/while/if/etc.
        return MethodDecompiler._fold_while_to_for_recursive(result)

    @staticmethod
    def _find_for_init(
        result: List[str], cond: str, last_body: str
    ) -> Optional[tuple]:
        """Scan backwards through already-emitted ``result`` to find a for-loop
        init statement whose variable appears in *cond* and in *last_body*.

        Returns ``(var_name, init_expr, remove_index)`` or ``None``.
        ``remove_index`` is the index in *result* to delete, or ``None`` if the
        init is logically empty (shouldn't happen in practice).
        """
        # We scan backwards through result, skipping only bare var declarations
        # that are NOT the init we're looking for.
        _VAR_DECL = re.compile(r'^var (\w+)(:\w[\w.<>]*)?;$')
        _VAR_INIT = re.compile(r'^var (\w+)(:\w[\w.<>]*)?\s*=\s*(.+);$')
        _ASSIGN   = re.compile(r'^(\w+)\s*=\s*(.+);$')

        # How far back to scan (limit to a small window)
        max_scan = min(len(result), 6)
        for back in range(1, max_scan + 1):
            idx = len(result) - back
            if idx < 0:
                break
            candidate = result[idx].strip()

            # Try match patterns
            var_name = None
            init_expr = None

            m = _VAR_INIT.match(candidate)
            if m:
                var_name = m.group(1)
                var_type = m.group(2) or ''
                init_expr = f'var {var_name}{var_type} = {m.group(3)}'
            else:
                m = _VAR_DECL.match(candidate)
                if m:
                    var_name = m.group(1)
                    var_type = m.group(2) or ''
                    init_expr = f'var {var_name}{var_type} = 0'
                else:
                    m = _ASSIGN.match(candidate)
                    if m:
                        var_name = m.group(1)
                        init_expr = f'{var_name} = {m.group(2)}'
                    else:
                        # Hit a non-declaration / non-assignment — stop scanning
                        break

            if var_name and re.search(r'\b' + re.escape(var_name) + r'\b', cond):
                # Verify the step also references this variable
                if re.search(r'\b' + re.escape(var_name) + r'\b', last_body):
                    return (var_name, init_expr, idx)

            # If we haven't matched yet but the candidate IS a var declaration,
            # keep scanning backwards (skip over unrelated var decls).
            if not _VAR_DECL.match(candidate) and not _VAR_INIT.match(candidate):
                # Not a var declaration — stop scanning
                break

        return None

    @staticmethod
    def _match_step(var_name: str, last_s: str) -> Optional[str]:
        """Match step expression patterns for a given variable in the last
        body statement.  Returns the step expression string or ``None``."""
        vn_esc = re.escape(var_name)
        if re.match(rf'^{vn_esc}\+\+;$', last_s):
            return f'{var_name}++'
        if re.match(rf'^{vn_esc}--;$', last_s):
            return f'{var_name}--'
        # X += N
        if (m := re.match(rf'^{vn_esc} \+= (.+);$', last_s)):
            return f'{var_name} += {m.group(1)}'
        # X -= N
        if (m := re.match(rf'^{vn_esc} -= (.+);$', last_s)):
            return f'{var_name} -= {m.group(1)}'
        # X = X + N  (bare)
        if (m := re.match(rf'^{vn_esc} = {vn_esc} \+ (.+);$', last_s)):
            return f'{var_name} += {m.group(1)}'
        # X = X - N  (bare)
        if (m := re.match(rf'^{vn_esc} = {vn_esc} - (.+);$', last_s)):
            return f'{var_name} -= {m.group(1)}'
        # X = (X + N)
        if (m := re.match(rf'^{vn_esc} = \({vn_esc} \+ (.+)\);$', last_s)):
            return f'{var_name} += {m.group(1)}'
        # X = (X - N)
        if (m := re.match(rf'^{vn_esc} = \({vn_esc} - (.+)\);$', last_s)):
            return f'{var_name} -= {m.group(1)}'
        # X = int((X + N))
        if (m := re.match(rf'^{vn_esc} = int\(\({vn_esc} \+ (.+)\)\);$', last_s)):
            return f'{var_name} += {m.group(1)}'
        # X = int((X - N))
        if (m := re.match(rf'^{vn_esc} = int\(\({vn_esc} - (.+)\)\);$', last_s)):
            return f'{var_name} -= {m.group(1)}'
        # X = uint((X + N))
        if (m := re.match(rf'^{vn_esc} = uint\(\({vn_esc} \+ (.+)\)\);$', last_s)):
            return f'{var_name} += {m.group(1)}'
        # X = uint((X - N))
        if (m := re.match(rf'^{vn_esc} = uint\(\({vn_esc} - (.+)\)\);$', last_s)):
            return f'{var_name} -= {m.group(1)}'
        return None

    @staticmethod
    def _count_net_braces(line: str) -> int:
        """Count net opening braces minus closing braces in a line,
        ignoring braces inside string literals and comments."""
        s = line.strip()
        if s.startswith('//'):
            return 0
        count = 0
        in_str: Optional[str] = None
        idx = 0
        while idx < len(s):
            c = s[idx]
            if in_str:
                if c == '\\':
                    idx += 2
                    continue
                if c == in_str:
                    in_str = None
            elif c in ('"', "'"):
                in_str = c
            elif c == '{':
                count += 1
            elif c == '}':
                count -= 1
            idx += 1
        return count

    @staticmethod
    def _fold_while_to_for_recursive(stmts: List[str]) -> List[str]:
        """Apply _fold_while_to_for inside nested blocks (for, while, if, etc.).

        Handles both separate-line braces (header + ``{``) and inline braces
        (e.g. ``switch (N) {``).  Uses brace counting that correctly tracks
        depth through try/catch, switch/case, and other block types (issue #32).
        """
        result: List[str] = []
        i = 0
        while i < len(stmts):
            s = stmts[i].strip()

            # Case 1: Block with { on separate next line (standard format from
            # _struct_block, _fold_try_catch, _fold_switch, etc.)
            if i + 1 < len(stmts) and stmts[i + 1].strip() == '{':
                # Find the matching close brace using robust brace counting
                depth = 1
                j = i + 2
                while j < len(stmts) and depth > 0:
                    depth += MethodDecompiler._count_net_braces(stmts[j])
                    j += 1
                close_idx = j - 1
                if depth == 0:
                    result.append(stmts[i])  # header line
                    result.append(stmts[i + 1])  # {
                    # Recursively fold the inner body
                    inner = stmts[i + 2:close_idx]
                    inner = MethodDecompiler._fold_while_to_for(inner)
                    result.extend(inner)
                    result.append(stmts[close_idx])  # };
                    i = close_idx + 1
                    continue

            # Case 2: Header line with inline { (e.g. "switch (N) {")
            # The line itself opens a block — no separate { line.
            net = MethodDecompiler._count_net_braces(s)
            if net > 0 and s != '{' and not s.startswith('//'):
                depth = net
                j = i + 1
                while j < len(stmts) and depth > 0:
                    depth += MethodDecompiler._count_net_braces(stmts[j])
                    j += 1
                close_idx = j - 1
                if depth == 0:
                    result.append(stmts[i])  # header with {
                    inner = stmts[i + 1:close_idx]
                    inner = MethodDecompiler._fold_while_to_for(inner)
                    result.extend(inner)
                    result.append(stmts[close_idx])  # };
                    i = close_idx + 1
                    continue

            result.append(stmts[i])
            i += 1
        return result

    @staticmethod
    def _fold_for_each_in(stmts: List[str]) -> List[str]:
        """Reconstruct for-each / for-in loops from hasnext2+nextvalue/nextname patterns.

        Detects:
            [idx_var = 0;]
            [obj_var = collection;]
            while (hasnext2(obj_var, idx_var))
            {
                loop_var = [cast](nextvalue(obj_var, idx_var));   // for-each
                loop_var = nextname(obj_var, idx_var);            // for-in
                ... body ...
            };
        Transforms to:
            for each (var loop_var[:type] in collection)  // nextvalue
            for (var loop_var[:type] in collection)       // nextname
        """
        result: List[str] = []
        i = 0
        while i < len(stmts):
            s = stmts[i].strip()

            # Match: while (hasnext2(OBJ, IDX))
            m_while = _RE_WHILE_HASNEXT.match(s)
            if m_while:
                obj_var = m_while.group(1)
                idx_var = m_while.group(2)

                # Expect { on next line
                if i + 1 < len(stmts) and stmts[i + 1].strip() == '{':
                    # Find matching };
                    brace_start = i + 1
                    depth = 1
                    j = brace_start + 1
                    while j < len(stmts) and depth > 0:
                        line = stmts[j].strip()
                        if line == '{':
                            depth += 1
                        elif line == '};' or line == '}':
                            depth -= 1
                        j += 1
                    brace_end = j - 1  # index of the '};'

                    # Check first body statement for nextvalue or nextname
                    if brace_start + 1 < brace_end:
                        first_body = stmts[brace_start + 1].strip()

                        # Match: VAR = [cast](nextvalue(obj, idx)); or VAR = nextvalue(obj, idx);
                        m_nv = re.match(
                            r'^(\w+)\s*=\s*(?:\w+\()?\s*nextvalue\(' +
                            re.escape(obj_var) + r',\s*' + re.escape(idx_var) +
                            r'\)\)?;$', first_body)
                        # Match: VAR = nextname(obj, idx);
                        m_nn = re.match(
                            r'^(\w+)\s*=\s*nextname\(' +
                            re.escape(obj_var) + r',\s*' + re.escape(idx_var) +
                            r'\);$', first_body)

                        # Also match var declarations: var VAR:TYPE = ...
                        m_nv_var = re.match(
                            r'^var\s+(\w+)(:\w+\*?)?\s*=\s*(?:\w+\()?\s*nextvalue\(' +
                            re.escape(obj_var) + r',\s*' + re.escape(idx_var) +
                            r'\)\)?;$', first_body)
                        m_nn_var = re.match(
                            r'^var\s+(\w+)(:\w+\*?)?\s*=\s*nextname\(' +
                            re.escape(obj_var) + r',\s*' + re.escape(idx_var) +
                            r'\);$', first_body)

                        is_for_each = m_nv is not None or m_nv_var is not None
                        is_for_in = m_nn is not None or m_nn_var is not None

                        if is_for_each or is_for_in:
                            loop_var = (m_nv or m_nv_var or m_nn or m_nn_var).group(1)

                            # Look backwards for obj_var = COLLECTION; to find original collection
                            collection = obj_var
                            remove_indices = set()
                            for k in range(len(result) - 1, -1, -1):
                                rline = result[k].strip()
                                # obj_var = EXPR;
                                m_obj = re.match(
                                    r'^(?:var\s+)?' + re.escape(obj_var) +
                                    r'(?::\S+)?\s*=\s*(.+);$', rline)
                                if m_obj:
                                    collection = m_obj.group(1)
                                    remove_indices.add(k)
                                    break

                            # Also try to remove idx_var = 0; or var idx_var:int;
                            for k in range(len(result) - 1, -1, -1):
                                rline = result[k].strip()
                                if re.match(r'^(?:var\s+)?' + re.escape(idx_var) +
                                            r'(?::\w+)?\s*=\s*0;$', rline):
                                    remove_indices.add(k)
                                    break
                                elif re.match(r'^var\s+' + re.escape(idx_var) +
                                              r':int;$', rline):
                                    remove_indices.add(k)
                                    break

                            # Determine loop variable type annotation
                            loop_var_type = ''
                            if m_nv_var and m_nv_var.group(2):
                                loop_var_type = m_nv_var.group(2)
                            elif m_nn_var and m_nn_var.group(2):
                                loop_var_type = m_nn_var.group(2)
                            else:
                                # Look backwards for var declaration of loop_var
                                for k in range(len(result) - 1, -1, -1):
                                    rline = result[k].strip()
                                    m_decl = re.match(
                                        r'^var\s+' + re.escape(loop_var) +
                                        r'(:\S+?)?\s*(?:=.*)?;$', rline)
                                    if m_decl:
                                        if m_decl.group(1):
                                            loop_var_type = m_decl.group(1)
                                        remove_indices.add(k)
                                        break

                            # Also remove var obj_var declaration if separate from assignment
                            for k in range(len(result) - 1, -1, -1):
                                rline = result[k].strip()
                                if re.match(r'^var\s+' + re.escape(obj_var) +
                                            r':\S+\s*=\s*.+;$', rline):
                                    # Already handled above
                                    break
                                elif re.match(r'^var\s+' + re.escape(obj_var) +
                                              r':\S+;$', rline):
                                    remove_indices.add(k)
                                    break

                            # Remove the identified setup lines from result
                            if remove_indices:
                                result = [r for ri, r in enumerate(result)
                                          if ri not in remove_indices]

                            # Emit for-each or for-in
                            keyword = 'for each' if is_for_each else 'for'
                            var_decl = f'var {loop_var}{loop_var_type}'
                            result.append(f'{keyword} ({var_decl} in {collection})')
                            result.append('{')
                            # Body: everything after the first assignment
                            for k in range(brace_start + 2, brace_end):
                                result.append(stmts[k])
                            result.append('};')
                            i = brace_end + 1
                            continue

            result.append(stmts[i])
            i += 1
        return result

    def _structure_flow(self, stmts: List[str]) -> List[str]:
        """Convert goto-based statements into structured if/else/while blocks."""
        # Build label → position mapping
        label_pos: Dict[str, int] = {}
        for i, s in enumerate(stmts):
            m = _RE_LABEL_NUM_COLON.match(s.strip())
            if m:
                label_pos[f'__label_{m.group(1)}'] = i

        if not label_pos:
            # No labels — just remove trailing return;
            if stmts and stmts[-1].strip() == 'return;':
                stmts = stmts[:-1]
            return stmts

        # Save/restore shared state for re-entrancy (issue #21):
        # _decompile_inline_function() may call _structure_flow() recursively
        # while an outer _structure_flow() is still in progress.
        prev_counter = getattr(self, '_loop_label_counter', 0)
        prev_labels = getattr(self, '_needs_loop_label', set())
        self._loop_label_counter = 0
        self._needs_loop_label = set()

        result = self._struct_block(stmts, 0, len(stmts), label_pos, depth=0)

        # Remove trailing return;
        while result and result[-1].strip() == 'return;':
            result.pop()

        # Fix unresolved gotos: Remove ALL goto statements that weren't properly
        # restructured into control flow (issue #25 workaround).
        # Final cleanup pass: remove any remaining gotos and orphaned labels
        # Repeat until no more changes (edge case where removal creates new patterns)
        for _pass in range(25):
            changed = False
            temp_result = []
            
            for line in result:
                stripped = line.strip()
                
                # Remove any line with goto __label_ (decompiler artifacts)
                if 'goto __label_' in stripped:
                    changed = True
                    continue
                
                # Remove orphaned labels
                if _RE_LABEL_WS.match(stripped):
                    changed = True
                    continue
                
                temp_result.append(line)
            
            result = temp_result
            if not changed:
                break
        
        # Remove empty lines at the end
        while result and not result[-1].strip():
            result.pop()

        # Restore previous state for the outer call
        self._loop_label_counter = prev_counter
        self._needs_loop_label = prev_labels

        return result

    def _struct_block(self, stmts: List[str], start: int, end: int,
                      label_pos: Dict[str, int],
                      loop_ctx: Optional[Dict] = None,
                      depth: int = 0) -> List[str]:
        """Recursively convert a range of statements into structured code.

        loop_ctx: optional dict with:
            'continue_labels': set of label names where goto → continue
            'break_label_map': dict mapping label_name → None (own loop) or
                               (loop_label_str, needs_label_set) for outer loops
        depth: current recursion depth for overflow protection
        """
        if depth > _MAX_STRUCT_DEPTH:
            # Recursion too deep — emit remaining statements flat
            return [stmts[j] for j in range(start, end) if stmts[j].strip()]

        result: List[str] = []
        i = start

        while i < end:
            s = stmts[i].strip()
            if not s:
                i += 1
                continue

            # ── Label ─────────────────────────────────────────
            if s.startswith('__label_') and s.endswith(':'):
                label_name = s[:-1]

                # Check if this label is a backward-goto target (loop header)
                back_pos = self._find_back_goto(stmts, i, end, label_name)
                if back_pos is not None:
                    i = self._emit_loop(stmts, i, back_pos, end, label_name,
                                        label_pos, result, loop_ctx, depth)
                    continue

                # Non-loop label — skip it (consumed by forward jumps)
                i += 1
                continue

            # ── If-goto (forward) ─────────────────────────────
            m = _RE_IF_GOTO.match(s)
            if m:
                cond = m.group(1)
                target = m.group(2)
                target_pos = label_pos.get(target, -1)

                if target_pos > i:
                    i = self._emit_if(stmts, i, cond, target, target_pos,
                                      end, label_pos, result, loop_ctx, depth)
                    continue
                # Backward if-goto — leave as-is (rare w/o loop detection)
                result.append(s)
                i += 1
                continue

            # ── switch ────────────────────────────────────────
            if s.startswith('switch ('):
                result.append(s)
                i += 1
                # Capture the switch body up to the closing '}'
                while i < end:
                    si = stmts[i].strip()
                    result.append(si)
                    i += 1
                    if si == '}':
                        break
                continue

            # ── Unconditional goto ────────────────────────────
            m_goto = _RE_GOTO_LABEL.match(s)
            if m_goto:
                target = m_goto.group(1)
                target_pos = label_pos.get(target, -1)

                # Check loop context: continue/break labels
                if loop_ctx:
                    if target in loop_ctx.get('continue_labels', set()):
                        result.append('continue;')
                        i += 1
                        continue
                    brk_map = loop_ctx.get('break_label_map', {})
                    if target in brk_map:
                        info = brk_map[target]
                        if info is None:
                            # Own loop break
                            result.append('break;')
                        else:
                            # Outer loop break — emit labeled break
                            loop_label, needs_label = info
                            needs_label.add(loop_label)
                            result.append(f'break {loop_label};')
                        i += 1
                        continue

                # Check for while-loop pattern:
                #   goto COND_LABEL; BODY_LABEL: ...body... COND_LABEL: if(cond) goto BODY_LABEL;
                if target_pos > i:
                    next_i = i + 1
                    if next_i < end:
                        next_s = stmts[next_i].strip()
                        m_body_lbl = _RE_LABEL_COLON.match(next_s)
                        if m_body_lbl:
                            body_label = m_body_lbl.group(1)
                            # Find the condition at or after target_pos
                            # Skip ALL consecutive labels (there may be multiple
                            # due to short-circuit && combine points)
                            cpos = target_pos
                            while cpos < end and _RE_LABEL_NUM_COLON.match(stmts[cpos].strip()):
                                cpos += 1
                            if cpos < end:
                                m_cond = re.match(
                                    rf'^if \((.+)\) goto {re.escape(body_label)};$',
                                    stmts[cpos].strip())
                                if m_cond:
                                    cond = m_cond.group(1)
                                    # Determine loop exit label (first label after the loop condition)
                                    loop_exit_pos = cpos + 1
                                    exit_labels = set()
                                    if loop_exit_pos < len(stmts):
                                        m_exit = _RE_LABEL_COLON.match(stmts[loop_exit_pos].strip())
                                        if m_exit:
                                            exit_labels.add(m_exit.group(1))
                                    # Determine continue labels: scan backward from target_pos
                                    # to find labels that only have non-branching code between
                                    # them and the condition (pure increment section)
                                    cont_labels = set()
                                    # The condition label itself is a continue target
                                    m_cl_cond = _RE_LABEL_COLON.match(stmts[target_pos].strip())
                                    if m_cl_cond:
                                        cont_labels.add(m_cl_cond.group(1))
                                    # Scan backward from target_pos to find increment section labels
                                    for cl_idx in range(target_pos - 1, next_i, -1):
                                        cl_s = stmts[cl_idx].strip()
                                        m_cl = _RE_LABEL_COLON.match(cl_s)
                                        if m_cl:
                                            cont_labels.add(m_cl.group(1))
                                        elif cl_s and ('goto' in cl_s or cl_s.startswith('if ')):
                                            break  # Hit a branch — stop scanning
                                    w_loop_label = self._next_loop_label()
                                    inner_loop_ctx = {
                                        'continue_labels': cont_labels,
                                        'break_label_map': self._build_break_label_map(
                                            exit_labels, w_loop_label, loop_ctx),
                                        'loop_label': w_loop_label,
                                    }
                                    inner = self._struct_block(stmts, next_i + 1,
                                                               target_pos, label_pos,
                                                               inner_loop_ctx, depth + 1)
                                    while_line = f'while ({cond})'
                                    if w_loop_label in self._needs_loop_label:
                                        while_line = f'{w_loop_label}: {while_line}'
                                    result.append(while_line)
                                    result.append('{')
                                    for line in inner:
                                        result.append(f'{INDENT_UNIT}{line}')
                                    result.append('};')
                                    i = cpos + 1
                                    continue

                            # Check for while(true) pattern:
                            #   goto COND_LABEL; BODY_LABEL: ...body... COND_LABEL: goto BODY_LABEL;
                            if cpos < end:
                                m_uncond = re.match(
                                    rf'^goto {re.escape(body_label)};$',
                                    stmts[cpos].strip())
                                if m_uncond:
                                    # while(true) loop
                                    # Find exit labels after the loop (first label after cpos)
                                    exit_labels = set()
                                    for el_idx in range(cpos + 1, min(cpos + 3, len(stmts))):
                                        m_el = _RE_LABEL_COLON.match(stmts[el_idx].strip())
                                        if m_el:
                                            exit_labels.add(m_el.group(1))
                                            break
                                    wt_loop_label = self._next_loop_label()
                                    inner_loop_ctx = {
                                        'continue_labels': set(),
                                        'break_label_map': self._build_break_label_map(
                                            exit_labels, wt_loop_label, loop_ctx),
                                        'loop_label': wt_loop_label,
                                    }
                                    inner = self._struct_block(stmts, next_i + 1,
                                                               target_pos, label_pos,
                                                               inner_loop_ctx, depth + 1)
                                    while_line = 'while (true)'
                                    if wt_loop_label in self._needs_loop_label:
                                        while_line = f'{wt_loop_label}: {while_line}'
                                    result.append(while_line)
                                    result.append('{')
                                    for line in inner:
                                        result.append(f'{INDENT_UNIT}{line}')
                                    result.append('};')
                                    i = cpos + 1
                                    continue

                # Check for do-while with redundant goto into body
                # Pattern: goto L2; L1: [L2:] body; if(cond) goto L1;
                # The goto target is inside the loop body → this is do-while, skip the goto
                if target_pos > i:
                    next_i = i + 1
                    if next_i < end:
                        next_s = stmts[next_i].strip()
                        m_body_lbl = _RE_LABEL_COLON.match(next_s)
                        if m_body_lbl:
                            body_label = m_body_lbl.group(1)
                            back_pos = self._find_back_goto(stmts, next_i, end, body_label)
                            if back_pos is not None and target_pos <= back_pos:
                                # The goto target is within the loop body
                                # Skip the goto; the loop header at next_i will be
                                # processed and emitted as do-while
                                i += 1
                                continue

                if 0 <= target_pos < i:
                    result.append('continue;')
                elif target_pos >= end:
                    result.append('break;')
                else:
                    result.append(s)
                i += 1
                continue

            # ── Regular statement ─────────────────────────────
            result.append(s)
            i += 1

        return result

    # ── Loop emission ─────────────────────────────────────────────
    def _find_back_goto(self, stmts: List[str], label_idx: int,
                        end: int, label_name: str) -> Optional[int]:
        """Find a backward goto/if-goto targeting label_name after label_idx."""
        # Use fast string matching instead of regex (performance hotspot)
        goto_exact = f'goto {label_name};'
        if_goto_suffix = f') goto {label_name};'
        for j in range(label_idx + 1, end):
            s = stmts[j].strip()
            if s == goto_exact:
                return j
            if s.startswith('if (') and s.endswith(if_goto_suffix):
                return j
        return None

    def _next_loop_label(self) -> str:
        """Generate a unique loop label for labeled break support."""
        self._loop_label_counter += 1
        return f'_loop_{self._loop_label_counter}'

    def _build_break_label_map(self, own_break_labels: set,
                               loop_label: str,
                               outer_loop_ctx: Optional[Dict]) -> Dict:
        """Build a break_label_map for a new loop context.

        own_break_labels: labels that mean 'break' for THIS loop → mapped to None
        loop_label: this loop's label (for outer loops to reference)
        outer_loop_ctx: the enclosing loop's context (if any)

        Returns a dict mapping label_name → None (own break) or
            (outer_loop_label, needs_label_set) for outer loop breaks.
        """
        brk_map = {}
        for lbl in own_break_labels:
            brk_map[lbl] = None
        if outer_loop_ctx:
            outer_map = outer_loop_ctx.get('break_label_map', {})
            for lbl, info in outer_map.items():
                if lbl not in brk_map:
                    if info is None:
                        # Outer loop's own break → now references the outer loop's label
                        outer_label = outer_loop_ctx.get('loop_label', '')
                        brk_map[lbl] = (outer_label, self._needs_loop_label)
                    else:
                        # Propagate deeper outer breaks as-is
                        brk_map[lbl] = info
        return brk_map

    def _emit_loop(self, stmts: List[str], label_idx: int, back_pos: int,
                   end: int, label_name: str,
                   label_pos: Dict[str, int],
                   result: List[str],
                   outer_loop_ctx: Optional[Dict] = None,
                   depth: int = 0) -> int:
        """Emit a while / do-while loop, return the next index to process."""
        back_stmt = stmts[back_pos].strip()
        loop_label = self._next_loop_label()

        if back_stmt == f'goto {label_name};':
            # Unconditional back-edge → check for while (cond) pattern
            body_start = label_idx + 1
            if body_start < back_pos:
                first = stmts[body_start].strip()
                m = _RE_IF_GOTO.match(first)
                if m:
                    exit_label = m.group(2)
                    exit_pos = label_pos.get(exit_label, -1)
                    if exit_pos >= back_pos:
                        # while (negated_cond) { body }
                        cond = self._negate_cond(m.group(1))
                        exit_labels = {exit_label}
                        cont_labels = {label_name}
                        # Scan backward for increment-section continue labels
                        for cl_idx in range(back_pos - 1, body_start, -1):
                            cl_s = stmts[cl_idx].strip()
                            m_cl = _RE_LABEL_COLON.match(cl_s)
                            if m_cl:
                                cont_labels.add(m_cl.group(1))
                            elif cl_s and ('goto' in cl_s or cl_s.startswith('if ')):
                                break
                        inner_loop_ctx = {
                            'continue_labels': cont_labels,
                            'break_label_map': self._build_break_label_map(
                                exit_labels, loop_label, outer_loop_ctx),
                            'loop_label': loop_label,
                        }
                        inner = self._struct_block(stmts, body_start + 1,
                                                   back_pos, label_pos,
                                                   inner_loop_ctx, depth + 1)
                        while_line = f'while ({cond})'
                        if loop_label in self._needs_loop_label:
                            while_line = f'{loop_label}: {while_line}'
                        result.append(while_line)
                        result.append('{')
                        for line in inner:
                            result.append(f'{INDENT_UNIT}{line}')
                        result.append('};')
                        # Advance past the exit label
                        nxt = exit_pos
                        if nxt < end and stmts[nxt].strip().startswith('__label_') \
                                and stmts[nxt].strip().endswith(':'):
                            nxt += 1
                        return nxt

            # Fallback: while (true) { body }
            exit_labels = set()
            for el_idx in range(back_pos + 1, min(back_pos + 3, len(stmts))):
                m_el = _RE_LABEL_COLON.match(stmts[el_idx].strip())
                if m_el:
                    exit_labels.add(m_el.group(1))
                    break
            inner_loop_ctx = {
                'continue_labels': {label_name},
                'break_label_map': self._build_break_label_map(
                    exit_labels, loop_label, outer_loop_ctx),
                'loop_label': loop_label,
            }
            inner = self._struct_block(stmts, label_idx + 1,
                                       back_pos, label_pos,
                                       inner_loop_ctx, depth + 1)
            while_line = 'while (true)'
            if loop_label in self._needs_loop_label:
                while_line = f'{loop_label}: {while_line}'
            result.append(while_line)
            result.append('{')
            for line in inner:
                result.append(f'{INDENT_UNIT}{line}')
            result.append('};')
            return back_pos + 1

        # Conditional back-edge → do-while
        m = re.match(rf'^if \((.+)\) goto {re.escape(label_name)};$',
                     back_stmt)
        if m:
            cond = m.group(1)
            exit_labels = set()
            for el_idx in range(back_pos + 1, min(back_pos + 3, len(stmts))):
                m_el = _RE_LABEL_COLON.match(stmts[el_idx].strip())
                if m_el:
                    exit_labels.add(m_el.group(1))
                    break
            inner_loop_ctx = {
                'continue_labels': {label_name},
                'break_label_map': self._build_break_label_map(
                    exit_labels, loop_label, outer_loop_ctx),
                'loop_label': loop_label,
            }
            inner = self._struct_block(stmts, label_idx + 1,
                                       back_pos, label_pos,
                                       inner_loop_ctx, depth + 1)
            do_line = 'do'
            if loop_label in self._needs_loop_label:
                do_line = f'{loop_label}: do'
            result.append(do_line)
            result.append('{')
            for line in inner:
                result.append(f'{INDENT_UNIT}{line}')
            result.append(f'}} while ({cond});')
            return back_pos + 1

        # Unrecognised — leave as-is
        return label_idx + 1

    # ── If / if-else emission ─────────────────────────────────────
    def _emit_if(self, stmts: List[str], if_idx: int, cond: str,
                 target: str, target_pos: int, end: int,
                 label_pos: Dict[str, int],
                 result: List[str],
                 loop_ctx: Optional[Dict] = None,
                 depth: int = 0) -> int:
        """Emit an if or if-else block, return the next index to process."""
        # Check for if-else: goto __label_END just before target label
        # When then-block would be empty (goto is at if_idx+1), check if the
        # goto is actually a continue/break rather than an else-end marker.
        pre_target = target_pos - 1
        if pre_target > if_idx:
            pre_stmt = stmts[pre_target].strip()
            m2 = _RE_GOTO_LABEL.match(pre_stmt)
            if m2:
                end_label = m2.group(1)
                end_pos = label_pos.get(end_label, -1)
                # Skip if-else detection when then-block would be empty AND the
                # goto targets a loop continue/break label (it's the body, not a marker)
                skip_ifelse = False
                if pre_target == if_idx + 1 and loop_ctx:
                    if (end_label in loop_ctx.get('continue_labels', set()) or
                            end_label in loop_ctx.get('break_label_map', {})):
                        skip_ifelse = True
                if not skip_ifelse and end_pos > target_pos and end_pos <= end:
                    # If-else pattern (end_pos within current block)
                    neg_cond = self._negate_cond(cond)
                    then_block = self._struct_block(stmts, if_idx + 1,
                                                   pre_target, label_pos,
                                                   loop_ctx, depth + 1)
                    else_block = self._struct_block(stmts, target_pos + 1,
                                                   end_pos, label_pos,
                                                   loop_ctx, depth + 1)
                    result.append(f'if ({neg_cond})')
                    result.append('{')
                    for t in then_block:
                        result.append(f'{INDENT_UNIT}{t}')
                    result.append('}')
                    result.append('else')
                    result.append('{')
                    for e in else_block:
                        result.append(f'{INDENT_UNIT}{e}')
                    result.append('};')
                    nxt = end_pos
                    if nxt < end and stmts[nxt].strip().startswith('__label_') \
                            and stmts[nxt].strip().endswith(':'):
                        nxt += 1
                    return nxt
                elif end_pos > target_pos and end_pos > end:
                    # The "else end" is beyond our block — this is not a true
                    # if-else; the goto before target is a break/continue.
                    # Check if it's a loop break/continue
                    if loop_ctx and end_label in loop_ctx.get('break_label_map', {}):
                        # then_body = stmts[if_idx+1..pre_target) + break
                        neg_cond = self._negate_cond(cond)
                        then_block = self._struct_block(stmts, if_idx + 1,
                                                       pre_target, label_pos,
                                                       loop_ctx, depth + 1)
                        brk_info = loop_ctx['break_label_map'][end_label]
                        if brk_info is None:
                            then_block.append('break;')
                        else:
                            lbl, needs = brk_info
                            needs.add(lbl)
                            then_block.append(f'break {lbl};')
                        else_block = self._struct_block(stmts, target_pos + 1,
                                                       end, label_pos,
                                                       loop_ctx, depth + 1)
                        if else_block:
                            result.append(f'if ({neg_cond})')
                            result.append('{')
                            for t in then_block:
                                result.append(f'{INDENT_UNIT}{t}')
                            result.append('}')
                            result.append('else')
                            result.append('{')
                            for e in else_block:
                                result.append(f'{INDENT_UNIT}{e}')
                            result.append('};')
                        else:
                            result.append(f'if ({neg_cond})')
                            result.append('{')
                            for t in then_block:
                                result.append(f'{INDENT_UNIT}{t}')
                            result.append('};')
                        nxt = end
                        return nxt
                    elif loop_ctx and end_label in loop_ctx.get('continue_labels', set()):
                        neg_cond = self._negate_cond(cond)
                        then_block = self._struct_block(stmts, if_idx + 1,
                                                       pre_target, label_pos,
                                                       loop_ctx, depth + 1)
                        then_block.append('continue;')
                        else_block = self._struct_block(stmts, target_pos + 1,
                                                       end, label_pos,
                                                       loop_ctx, depth + 1)
                        if else_block:
                            result.append(f'if ({neg_cond})')
                            result.append('{')
                            for t in then_block:
                                result.append(f'{INDENT_UNIT}{t}')
                            result.append('}')
                            result.append('else')
                            result.append('{')
                            for e in else_block:
                                result.append(f'{INDENT_UNIT}{e}')
                            result.append('};')
                        else:
                            result.append(f'if ({neg_cond})')
                            result.append('{')
                            for t in then_block:
                                result.append(f'{INDENT_UNIT}{t}')
                            result.append('};')
                        nxt = end
                        return nxt
                    else:
                        # Fall through to simple if-then (the goto will be
                        # handled when processing the then-body)
                        pass

        # Simple if-then
        neg_cond = self._negate_cond(cond)
        then_block = self._struct_block(stmts, if_idx + 1,
                                        target_pos, label_pos,
                                        loop_ctx, depth + 1)
        result.append(f'if ({neg_cond})')
        result.append('{')
        for t in then_block:
            result.append(f'{INDENT_UNIT}{t}')
        result.append('};')
        nxt = target_pos
        if nxt < end and stmts[nxt].strip().startswith('__label_') \
                and stmts[nxt].strip().endswith(':'):
            nxt += 1
        return nxt

    # ── Condition negation ────────────────────────────────────────
    @staticmethod
    def _negate_cond(cond: str) -> str:
        """Negate a condition expression for structured flow.
        
        Handles:
        - !(x) → x
        - !var → var
        - a OP b → a NEG_OP b (for simple comparisons)
        - Compound expressions (a && b, a || b) → wrap in !(...)
        
        For compound expressions containing && or || at depth 0,
        we avoid negating the inner operators to prevent incorrect results.
        """
        cond = cond.strip()

        # !(x) → x
        if cond.startswith('!(') and cond.endswith(')'):
            inner = cond[2:-1]
            depth = 0
            balanced = True
            for c in inner:
                if c == '(':
                    depth += 1
                elif c == ')':
                    depth -= 1
                if depth < 0:
                    balanced = False
                    break
            if balanced and depth == 0:
                return inner

        # Simple !var → var
        if cond.startswith('!') and '(' not in cond and ' ' not in cond:
            return cond[1:]

        # Check for compound logical operators at depth 0
        # If found, don't try to negate individual comparisons
        has_logical_op = False
        depth = 0
        i = 0
        while i < len(cond) - 1:
            if cond[i] == '(':
                depth += 1
            elif cond[i] == ')':
                depth -= 1
            elif cond[i] == '"':
                # Skip string literals
                i += 1
                while i < len(cond) and cond[i] != '"':
                    if cond[i] == '\\':
                        i += 1
                    i += 1
            elif depth == 0 and cond[i:i+2] in ('&&', '||'):
                has_logical_op = True
                break
            i += 1

        # If we found a logical operator at depth 0, wrap in !(...)
        if has_logical_op:
            if _has_outer_parens(cond):
                return f'!{cond}'
            return f'!({cond})'

        # (a OP b) → (a NEG_OP b) for simple comparisons without logical ops
        op_neg = {'==': '!=', '!=': '==', '===': '!==', '!==': '===',
                  '<': '>=', '>=': '<', '>': '<=', '<=': '>',
                  '!<': '<', '!<=': '<=', '!>': '>', '!>=': '>='}
        # Try each operator, longer first
        for pos_op in sorted(op_neg, key=len, reverse=True):
            idx = _find_op_outside_parens(cond, pos_op)
            if idx >= 0:
                left = cond[:idx].strip()
                right = cond[idx + len(pos_op):].strip()
                return f'{left} {op_neg[pos_op]} {right}'

        # Default: wrap in !()
        if _has_outer_parens(cond):
            return f'!{cond}'
        # Simple expressions: function calls, property chains, identifiers — don't need wrapping
        if cond.endswith(')') or ').' in cond or cond.replace('.', '').replace('_', '').isalnum():
            return f'!{cond}'
        return f'!({cond})'

    # ─── Ternary expression detection ────────────────────────────────────
    def _try_ternary(self, code: bytes, true_start: int, false_label: int,
                     stack_copy: List[str], local_names: Dict[int, str],
                     abc: 'ABCFile', slot_map: Dict[int, str],
                     local0_name: str, is_static: bool, class_idx: int
                     ) -> Optional[Tuple[str, str, int]]:
        """Detect ternary pattern after an iffalse instruction.

        Returns (true_val, false_val, end_pos) or None if not a ternary.
        true_start: position right after the iffalse operand (start of true branch)
        false_label: target of the iffalse (start of false branch)
        """
        if false_label <= true_start or false_label > len(code):
            return None

        # Find OP_JUMP at the end of the true branch (just before false_label)
        # Scan forward through the true branch looking for the last JUMP before false_label
        jump_pos = -1
        end_label = -1
        p = true_start
        while p < false_label:
            op = code[p]
            op_start = p
            p += 1
            if op == OP_JUMP:
                off, p = _rs24(code, p)
                jump_target = p + off
                if p == false_label:
                    jump_pos = op_start
                    end_label = jump_target
                    break
                # Not at the end → reset, keep scanning
            elif op in (OP_IFFALSE, OP_IFTRUE, OP_IFEQ, OP_IFNE, OP_IFLT, OP_IFLE,
                        OP_IFGT, OP_IFGE, OP_IFSTRICTEQ, OP_IFSTRICTNE,
                        OP_IFNLT, OP_IFNLE, OP_IFNGT, OP_IFNGE):
                _, p = _rs24(code, p)
            elif op == OP_LOOKUPSWITCH:
                _, p = _rs24(code, p)
                cc, p = _ru30(code, p)
                for _ in range(cc + 1):
                    _, p = _rs24(code, p)
            else:
                p = _skip_operands(op, code, p)

        if jump_pos < 0 or end_label < 0:
            return None

        # Evaluate both branches — use _eval_branch for each
        true_val = self._eval_branch(code, true_start, jump_pos, list(stack_copy),
                                     local_names, abc, slot_map, local0_name, is_static, class_idx)
        if true_val is None:
            return None

        false_val = self._eval_branch(code, false_label, end_label, list(stack_copy),
                                      local_names, abc, slot_map, local0_name, is_static, class_idx)
        if false_val is None:
            return None

        return (true_val, false_val, end_label)

    def _eval_branch(self, code: bytes, start: int, end: int,
                     stack: List[str], local_names: Dict[int, str],
                     abc: 'ABCFile', slot_map: Dict[int, str],
                     local0_name: str, is_static: bool, class_idx: int
                     ) -> Optional[str]:
        """Evaluate a branch's bytecodes and return the top-of-stack expression.
        Returns None if any side-effect statements are produced (not a pure expression)."""
        ectx = _EvalContext()
        ectx.code = code
        ectx.abc = abc
        ectx.stack = stack
        ectx.local_names = local_names
        ectx.slot_map = slot_map
        ectx.local0_name = local0_name
        ectx.is_static = is_static
        ectx.class_idx = class_idx
        ectx.p = start
        ectx.bail = False

        initial_depth = len(stack)
        while ectx.p < end:
            op = code[ectx.p]; ectx.p += 1

            handler = self._eval_dispatch.get(op)
            if handler is None:
                return None  # unknown/side-effect opcode — bail
            handler(op, ectx)
            if ectx.bail:
                return None

        # Should have produced exactly one new value on the stack
        if len(stack) > initial_depth:
            return stack[-1]
        return None

    # ═══════════════════════════════════════════════════════════════════════
    #  _eval_branch() opcode dispatch handlers
    # ═══════════════════════════════════════════════════════════════════════

    # ═══════════════════════════════════════════════════════════════════════
    #  _eval_branch() opcode dispatch handlers
    # ═══════════════════════════════════════════════════════════════════════

    def _eh_push_ops(self, op, ectx):
        """Handle push opcodes in eval mode."""
        abc = ectx.abc
        stack = ectx.stack
        if op == OP_PUSHBYTE:
            val = ectx.code[ectx.p]
            if val > 127: val -= 256
            ectx.p += 1
            stack.append(str(val))
        elif op == OP_PUSHSHORT:
            val, ectx.p = _ru30(ectx.code, ectx.p)
            if val >= 0x20000000: val -= 0x40000000
            stack.append(str(val))
        elif op == OP_PUSHSTRING:
            idx, ectx.p = _ru30(ectx.code, ectx.p)
            s = abc.strings[idx] if idx < len(abc.strings) else '?'
            stack.append(f'"{_escape_str(s)}"')
        elif op == OP_PUSHINT:
            idx, ectx.p = _ru30(ectx.code, ectx.p)
            stack.append(str(abc.integers[idx] if idx < len(abc.integers) else 0))
        elif op == OP_PUSHUINT:
            idx, ectx.p = _ru30(ectx.code, ectx.p)
            stack.append(_fmt_uint(abc.uintegers[idx] if idx < len(abc.uintegers) else 0))
        elif op == OP_PUSHDOUBLE:
            idx, ectx.p = _ru30(ectx.code, ectx.p)
            v = abc.doubles[idx] if idx < len(abc.doubles) else 0.0
            if v == int(v) and abs(v) < 1e15:
                iv = int(v)
                if iv >= 256 and iv == (iv & 0xFFFFFFFF):
                    stack.append(_fmt_hex(iv))
                else:
                    stack.append(str(iv))
            else:
                stack.append(f'{v:.15g}')
        elif op == OP_PUSHTRUE:
            stack.append('true')
        elif op == OP_PUSHFALSE:
            stack.append('false')
        elif op == OP_PUSHNULL:
            stack.append('null')
        elif op == OP_PUSHUNDEFINED:
            stack.append('undefined')
        elif op == OP_PUSHNAN:
            stack.append('NaN')

    def _eh_local_ops(self, op, ectx):
        """Handle getlocal ops in eval mode (no setlocal — those are side effects)."""
        if op == OP_GETLOCAL_0:
            ectx.stack.append(ectx.local_names.get(0, 'this'))
        elif op == OP_GETLOCAL_1:
            ectx.stack.append(ectx.local_names.get(1, '_local_1'))
        elif op == OP_GETLOCAL_2:
            ectx.stack.append(ectx.local_names.get(2, '_local_2'))
        elif op == OP_GETLOCAL_3:
            ectx.stack.append(ectx.local_names.get(3, '_local_3'))
        elif op == OP_GETLOCAL:
            idx, ectx.p = _ru30(ectx.code, ectx.p)
            ectx.stack.append(ectx.local_names.get(idx, f'_local_{idx}'))

    def _eh_property_ops(self, op, ectx):
        """Handle read-only property access in eval mode."""
        abc = ectx.abc
        stack = ectx.stack
        if op == OP_GETPROPERTY:
            mn, ectx.p = _ru30(ectx.code, ectx.p)
            rt_name = stack.pop() if (stack and abc.mn_needs_rt_name(mn)) else None
            rt_ns = stack.pop() if (stack and abc.mn_needs_rt_ns(mn)) else None
            obj = stack.pop() if stack else '?'
            if rt_name is not None:
                stack.append(f'{obj}[{rt_name}]')
            else:
                name = abc.mn_name(mn)
                if obj in ('', 'global') or obj == name:
                    stack.append(name)
                elif obj == 'this':
                    stack.append(f'this.{name}')
                elif obj == ectx.local0_name and ectx.is_static:
                    stack.append(name)
                else:
                    stack.append(f'{obj}.{name}')
        elif op == OP_GETLEX:
            mn, ectx.p = _ru30(ectx.code, ectx.p)
            stack.append(abc.mn_name(mn))
        elif op == OP_GETSLOT:
            idx, ectx.p = _ru30(ectx.code, ectx.p)
            obj = stack.pop() if stack else '?'
            slot_name = ectx.slot_map.get(idx, f'slot{idx}')
            if obj in ('', 'this', 'global', ectx.local0_name):
                stack.append(slot_name)
            else:
                stack.append(f'{obj}.{slot_name}')

    def _eh_find_ops(self, op, ectx):
        """Handle findproperty/findpropstrict in eval mode.
        
        Push the resolved name (not empty string) so that constructprop can
        detect obj==name and avoid spurious dot prefix in 'new .Array()' etc.
        """
        abc = ectx.abc
        if op == OP_FINDPROPSTRICT:
            mn, ectx.p = _ru30(ectx.code, ectx.p)
            if abc.mn_needs_rt_name(mn) and ectx.stack: ectx.stack.pop()
            if abc.mn_needs_rt_ns(mn) and ectx.stack: ectx.stack.pop()
            name = abc.mn_name(mn)
            ectx.stack.append(name)  # push resolved name (not empty)
        elif op == OP_FINDPROPERTY:
            mn, ectx.p = _ru30(ectx.code, ectx.p)
            if abc.mn_needs_rt_name(mn) and ectx.stack: ectx.stack.pop()
            if abc.mn_needs_rt_ns(mn) and ectx.stack: ectx.stack.pop()
            ectx.stack.append(abc.mn_name(mn))

    def _eh_coerce_noop(self, op, ectx):
        """Handle type coercion no-ops in eval mode."""
        if op == OP_COERCE:
            _, ectx.p = _ru30(ectx.code, ectx.p)
        elif op == OP_ASTYPE:
            idx, ectx.p = _ru30(ectx.code, ectx.p)
            tn = ectx.abc.mn_name(idx) if idx < len(ectx.abc.multinames) else '?'
            obj = ectx.stack.pop() if ectx.stack else '?'
            ectx.stack.append(f'({obj} as {tn})')
        # Other coerce ops are truly no-op (value stays on stack)

    def _eh_arithmetic_ops(self, op, ectx):
        """Handle arithmetic/bitwise/unary ops in eval mode."""
        stack = ectx.stack
        if op == OP_ADD:
            b = stack.pop() if stack else '?'
            a = stack.pop() if stack else '?'
            stack.append(f'{a} + {b}')
        elif op == OP_SUBTRACT:
            b = stack.pop() if stack else '?'
            a = stack.pop() if stack else '?'
            stack.append(f'{a} - {b}')
        elif op == OP_MULTIPLY:
            b = stack.pop() if stack else '?'
            a = stack.pop() if stack else '?'
            stack.append(f'{a} * {b}')
        elif op == OP_DIVIDE:
            b = stack.pop() if stack else '?'
            a = stack.pop() if stack else '?'
            stack.append(f'{a} / {b}')
        elif op == OP_MODULO:
            b = stack.pop() if stack else '?'
            a = stack.pop() if stack else '?'
            stack.append(f'{a} % {b}')
        elif op in (OP_NEGATE, OP_NEGATE_I):
            a = stack.pop() if stack else '?'
            stack.append(f'-({a})')
        elif op == OP_NOT:
            a = stack.pop() if stack else '?'
            _eq_match = _RE_EQ_MATCH.match(a)
            if _eq_match:
                _left, _eqop, _right = _eq_match.groups()
                _negop = '!==' if _eqop == '===' else '!='
                stack.append(f'({_left} {_negop} {_right})')
            else:
                stack.append(f'!{a}')
        elif op == OP_TYPEOF:
            a = stack.pop() if stack else '?'
            stack.append(f'typeof {a}')
        elif op == OP_BITOR:
            b = stack.pop() if stack else '?'
            a = stack.pop() if stack else '?'
            stack.append(f'{_to_hex_if_int(a)} | {_to_hex_if_int(b)}')
        elif op == OP_BITAND:
            b = stack.pop() if stack else '?'
            a = stack.pop() if stack else '?'
            stack.append(f'{_to_hex_if_int(a)} & {_to_hex_if_int(b)}')
        elif op == OP_BITXOR:
            b = stack.pop() if stack else '?'
            a = stack.pop() if stack else '?'
            stack.append(f'{_to_hex_if_int(a)} ^ {_to_hex_if_int(b)}')
        elif op == OP_BITNOT:
            a = stack.pop() if stack else '?'
            stack.append(f'(~({_to_hex_if_int(a)}))')
        elif op == OP_LSHIFT:
            b = stack.pop() if stack else '?'
            a = stack.pop() if stack else '?'
            stack.append(f'{a} << {b}')
        elif op == OP_RSHIFT:
            b = stack.pop() if stack else '?'
            a = stack.pop() if stack else '?'
            stack.append(f'{a} >> {b}')
        elif op == OP_URSHIFT:
            b = stack.pop() if stack else '?'
            a = stack.pop() if stack else '?'
            stack.append(f'{a} >>> {b}')
        elif op in (OP_INCREMENT, OP_INCREMENT_I):
            if stack: stack[-1] = f'({stack[-1]} + 1)'
        elif op in (OP_DECREMENT, OP_DECREMENT_I):
            if stack: stack[-1] = f'({stack[-1]} - 1)'

    def _eh_comparison_ops(self, op, ectx):
        """Handle comparison ops in eval mode."""
        stack = ectx.stack
        if op == OP_EQUALS:
            b = stack.pop() if stack else '?'
            a = stack.pop() if stack else '?'
            stack.append(f'{a} == {b}')
        elif op == OP_STRICTEQUALS:
            b = stack.pop() if stack else '?'
            a = stack.pop() if stack else '?'
            stack.append(f'{a} === {b}')
        elif op == OP_LESSTHAN:
            b = stack.pop() if stack else '?'
            a = stack.pop() if stack else '?'
            stack.append(f'{a} < {b}')
        elif op == OP_LESSEQUALS:
            b = stack.pop() if stack else '?'
            a = stack.pop() if stack else '?'
            stack.append(f'{a} <= {b}')
        elif op == OP_GREATERTHAN:
            b = stack.pop() if stack else '?'
            a = stack.pop() if stack else '?'
            stack.append(f'{a} > {b}')
        elif op == OP_GREATEREQUALS:
            b = stack.pop() if stack else '?'
            a = stack.pop() if stack else '?'
            stack.append(f'{a} >= {b}')
        elif op == OP_IN:
            name = stack.pop() if stack else '?'
            obj = stack.pop() if stack else '?'
            stack.append(f'({obj} in {name})')
        elif op == OP_INSTANCEOF:
            ty = stack.pop() if stack else '?'
            obj = stack.pop() if stack else '?'
            stack.append(f'({obj} instanceof {ty})')
        elif op == OP_ISTYPELATE:
            ty = stack.pop() if stack else '?'
            obj = stack.pop() if stack else '?'
            stack.append(f'({obj} is {ty})')
        elif op == OP_ASTYPELATE:
            ty = stack.pop() if stack else '?'
            obj = stack.pop() if stack else '?'
            stack.append(f'({obj} as {ty})')

    def _eh_object_ops(self, op, ectx):
        """Handle object/array construction in eval mode."""
        stack = ectx.stack
        if op == OP_NEWOBJECT:
            count, ectx.p = _ru30(ectx.code, ectx.p)
            pairs = []
            for _ in range(count):
                v = stack.pop() if stack else '?'
                k = stack.pop() if stack else '?'
                pairs.append(f'{k}:{v}')
            pairs.reverse()
            stack.append('{' + ', '.join(pairs) + '}')
        elif op == OP_NEWARRAY:
            count, ectx.p = _ru30(ectx.code, ectx.p)
            items = [stack.pop() for _ in range(count)] if stack else []
            items.reverse()
            stack.append(f'[{", ".join(items)}]')

    def _eh_call_ops(self, op, ectx):
        """Handle value-producing call ops in eval mode."""
        abc = ectx.abc
        stack = ectx.stack
        if op in (OP_CALLPROPERTY, OP_CALLPROPLEX):
            mn, ectx.p = _ru30(ectx.code, ectx.p)
            argc, ectx.p = _ru30(ectx.code, ectx.p)
            args = [stack.pop() for _ in range(argc)] if stack else []
            args.reverse()
            obj = stack.pop() if stack else '?'
            name = abc.mn_name(mn)
            if obj in ('', 'global'):
                stack.append(f'{name}({", ".join(args)})')
            else:
                stack.append(f'{obj}.{name}({", ".join(args)})')
        elif op == OP_CALLMETHOD:
            method_idx, ectx.p = _ru30(ectx.code, ectx.p)
            argc, ectx.p = _ru30(ectx.code, ectx.p)
            args = [stack.pop() for _ in range(argc)] if stack else []
            args.reverse()
            obj = stack.pop() if stack else '?'
            # OP_CALLMETHOD calls a specific method index on the object
            stack.append(f'callMethod({obj}, {method_idx}, {", ".join(args)})')
        elif op == OP_CALLSTATIC:
            method_idx, ectx.p = _ru30(ectx.code, ectx.p)
            argc, ectx.p = _ru30(ectx.code, ectx.p)
            args = [stack.pop() for _ in range(argc)] if stack else []
            args.reverse()
            # OP_CALLSTATIC calls a static method
            stack.append(f'callStatic({method_idx}, {", ".join(args)})')
        elif op == OP_CALLSUPER:
            mn, ectx.p = _ru30(ectx.code, ectx.p)
            argc, ectx.p = _ru30(ectx.code, ectx.p)
            args = [stack.pop() for _ in range(argc)] if stack else []
            args.reverse()
            # OP_CALLSUPER calls a method on this via super
            name = abc.mn_name(mn)
            stack.append(f'super.{name}({", ".join(args)})')

    def _eh_stack_ops(self, op, ectx):
        """Handle stack manipulation in eval mode."""
        stack = ectx.stack
        if op == OP_DUP:
            if stack:
                stack.append(stack[-1])
        elif op == OP_SWAP:
            if len(stack) >= 2:
                stack[-1], stack[-2] = stack[-2], stack[-1]
        elif op == OP_POP:
            if stack:
                stack.pop()

    def _eh_branch_ops(self, op, ectx):
        """Handle branch ops in eval mode — attempt ternary, else bail."""
        if op == OP_IFFALSE:
            off, p2 = _rs24(ectx.code, ectx.p)
            false_target = p2 + off
            ectx.p = p2
            cond = ectx.stack.pop() if ectx.stack else '?'
            inner = self._try_ternary(ectx.code, ectx.p, false_target, list(ectx.stack),
                                       ectx.local_names, ectx.abc, ectx.slot_map,
                                       ectx.local0_name, ectx.is_static, ectx.class_idx)
            if inner is not None:
                true_val, false_val, end_pos = inner
                c = cond if _has_outer_parens(cond) else f'({cond})'
                tv = f'({true_val})' if _needs_ternary_wrap(true_val) else true_val
                fv = f'({false_val})' if _needs_ternary_wrap(false_val) else false_val
                ectx.stack.append(f'({c} ? {tv} : {fv})')
                ectx.p = end_pos
            else:
                ectx.bail = True
        elif op in (OP_IFEQ, OP_IFNE, OP_IFLT, OP_IFLE, OP_IFGT, OP_IFGE,
                    OP_IFSTRICTEQ, OP_IFSTRICTNE,
                    OP_IFNLT, OP_IFNLE, OP_IFNGT, OP_IFNGE):
            off, p2 = _rs24(ectx.code, ectx.p)
            target = p2 + off
            ectx.p = p2
            b = ectx.stack.pop() if ectx.stack else '?'
            a = ectx.stack.pop() if ectx.stack else '?'
            op_map = {
                OP_IFEQ: '==', OP_IFNE: '!=', OP_IFLT: '<', OP_IFLE: '<=',
                OP_IFGT: '>', OP_IFGE: '>=', OP_IFSTRICTEQ: '===',
                OP_IFSTRICTNE: '!==',
            }
            not_cond_map = {
                OP_IFNGT: '>', OP_IFNLT: '<', OP_IFNLE: '<=', OP_IFNGE: '>=',
            }
            if op in not_cond_map and target > ectx.p:
                cond_str = f'{a} {not_cond_map[op]} {b}'
                inner = self._try_ternary(ectx.code, ectx.p, target, list(ectx.stack),
                                           ectx.local_names, ectx.abc, ectx.slot_map,
                                           ectx.local0_name, ectx.is_static, ectx.class_idx)
                if inner is not None:
                    true_val, false_val, end_pos = inner
                    c = f'({cond_str})'
                    tv = f'({true_val})' if _needs_ternary_wrap(true_val) else true_val
                    fv = f'({false_val})' if _needs_ternary_wrap(false_val) else false_val
                    ectx.stack.append(f'({c} ? {tv} : {fv})')
                    ectx.p = end_pos
                else:
                    ectx.bail = True
            elif op in op_map and target > ectx.p:
                ectx.bail = True
            else:
                ectx.bail = True
        elif op in (OP_IFTRUE, OP_JUMP):
            ectx.bail = True

    def _eh_construct_ops(self, op, ectx):
        """Handle construction ops in eval mode."""
        abc = ectx.abc
        stack = ectx.stack
        if op == OP_CONSTRUCT:
            argc, ectx.p = _ru30(ectx.code, ectx.p)
            args = [stack.pop() for _ in range(argc)] if stack else []
            args.reverse()
            obj = stack.pop() if stack else '?'
            stack.append(f'new {obj}({", ".join(args)})')
        elif op == OP_CONSTRUCTPROP:
            mn, ectx.p = _ru30(ectx.code, ectx.p)
            argc, ectx.p = _ru30(ectx.code, ectx.p)
            args = [stack.pop() for _ in range(argc)] if stack else []
            args.reverse()
            rt_name = stack.pop() if (stack and abc.mn_needs_rt_name(mn)) else None
            rt_ns = stack.pop() if (stack and abc.mn_needs_rt_ns(mn)) else None
            obj = stack.pop() if stack else '?'
            if rt_name is not None:
                stack.append(f'new {obj}[{rt_name}]({", ".join(args)})')
            else:
                name = abc.mn_name(mn)
                # Suppress dot when obj matches the class name or is empty/this
                # (prevents 'new .Array()' when findpropstrict pushes the name)
                if not obj or obj == 'this' or obj == name:
                    stack.append(f'new {name}({", ".join(args)})')
                else:
                    stack.append(f'new {obj}.{name}({", ".join(args)})')
        elif op == OP_APPLYTYPE:
            argc, ectx.p = _ru30(ectx.code, ectx.p)
            args = [stack.pop() for _ in range(argc)] if stack else []
            args.reverse()
            # In type parameter context, null represents * (the any type)
            args = ['*' if a == 'null' else a for a in args]
            obj = stack.pop() if stack else '?'
            # OP_APPLYTYPE applies type parameters to a generic type
            stack.append(f'{obj}.<{", ".join(args)}>')

    def _eh_bail(self, op, ectx):
        """Handler that forces bail for side-effect opcodes."""
        ectx.bail = True

    def _prescan_branches(self, code: bytes, targets: Set[int]) -> None:
        p = 0
        while p < len(code):
            op = code[p]; p += 1
            if op in _BRANCH_OPS:
                off, p = _rs24(code, p)
                targets.add(p + off)
            elif op == OP_LOOKUPSWITCH:
                base = p - 1
                default_off, p = _rs24(code, p)
                targets.add(base + default_off)
                case_count, p = _ru30(code, p)
                for _ in range(case_count + 1):
                    o, p = _rs24(code, p)
                    targets.add(base + o)
            else:
                p = _skip_operands(op, code, p)

    @staticmethod
    def _prescan_local_types(code: bytes, body: 'MethodBody', abc: 'ABCFile') -> Dict[int, str]:
        """Pre-scan bytecode to find local variable types from coerce→setlocal
        and push→setlocal patterns.

        Branch instructions reset the type-tracking state so that types inferred
        in one branch are not carried into another (issue #29).
        """
        local_types: Dict[int, str] = {}

        # First pass: collect branch targets so we can reset at join points too
        branch_targets: set = set()
        bp = 0
        while bp < len(code):
            bop = code[bp]; bp += 1
            if bop in _BRANCH_OPS:
                off, bp = _rs24(code, bp)
                branch_targets.add(bp + off)
            elif bop == OP_LOOKUPSWITCH:
                base = bp - 1
                default_off, bp = _rs24(code, bp)
                branch_targets.add(base + default_off)
                case_count, bp = _ru30(code, bp)
                for _ in range(case_count + 1):
                    o, bp = _rs24(code, bp)
                    branch_targets.add(base + o)
            else:
                bp = _skip_operands(bop, code, bp)

        p = 0
        last_coerce_type: Optional[str] = None
        last_push_type: Optional[str] = None  # fallback type from push instructions
        last_was_default: bool = False  # True when pushed value is null/0/false (default)
        last_was_pushnull: bool = False  # True specifically for pushnull (null+coerce keeps type)
        while p < len(code):
            # Reset tracking at branch targets (join points from other paths)
            if p in branch_targets:
                last_coerce_type = None
                last_push_type = None
                last_was_default = False
                last_was_pushnull = False

            op = code[p]; p += 1
            if op == OP_COERCE:
                mn, p = _ru30(code, p)
                last_coerce_type = abc.type_name(mn) if mn else None
                # pushnull + coerce X: keep the type (null is the default for class types)
                # pushdouble 0.0 + coerce Number: suppress the type
                if last_was_pushnull and last_coerce_type:
                    # Keep the coerce type, reset default flags
                    last_was_default = False
                    last_was_pushnull = False
                elif last_was_default:
                    last_coerce_type = None
                last_push_type = None
            elif op == OP_COERCE_I or op == OP_CONVERT_I:
                last_coerce_type = 'int' if not last_was_default else None
                last_push_type = None
            elif op == OP_COERCE_D or op == OP_CONVERT_D:
                last_coerce_type = 'Number' if not last_was_default else None
                last_push_type = None
            elif op == OP_COERCE_U or op == OP_CONVERT_U:
                last_coerce_type = 'uint' if not last_was_default else None
                last_push_type = None
            elif op == OP_COERCE_S or op == OP_CONVERT_S:
                last_coerce_type = 'String' if not last_was_default else None
                last_push_type = None
            elif op == OP_COERCE_B or op == OP_CONVERT_B:
                last_coerce_type = 'Boolean' if not last_was_default else None
                last_push_type = None
            elif op == OP_COERCE_O or op == OP_CONVERT_O:
                last_coerce_type = 'Object' if not last_was_default else None
                last_push_type = None
            elif op in (OP_PUSHBYTE, OP_PUSHSHORT, OP_PUSHINT):
                last_push_type = 'int'
                last_coerce_type = None
                last_was_default = False  # int 0 is a valid typed default
                last_was_pushnull = False
                p = _skip_operands(op, code, p)
            elif op == OP_PUSHUINT:
                last_push_type = 'uint'
                last_coerce_type = None
                last_was_default = False  # uint 0 is a valid typed default
                last_was_pushnull = False
                p = _skip_operands(op, code, p)
            elif op == OP_PUSHDOUBLE:
                last_coerce_type = None
                idx, _ = _ru30(code, p)
                v = abc.doubles[idx] if idx < len(abc.doubles) else 0.0
                # pushdouble 0.0 as default → suppress type inference (use *)
                last_was_default = (v == 0.0)
                last_push_type = None if last_was_default else 'Number'
                last_was_pushnull = False
                p = _skip_operands(op, code, p)
            elif op in (OP_PUSHTRUE, OP_PUSHFALSE):
                last_push_type = 'Boolean'
                last_coerce_type = None
                last_was_default = False  # Boolean is a valid typed default
                last_was_pushnull = False
            elif op == OP_PUSHNULL:
                last_was_default = True
                last_was_pushnull = True
                last_coerce_type = None
                last_push_type = None
            elif op == OP_PUSHNAMESPACE:
                # Namespace constants are built-in default values (issue #30)
                last_was_default = True
                last_was_pushnull = False
                last_coerce_type = None
                last_push_type = None
                p = _skip_operands(op, code, p)
            elif op in (OP_SETLOCAL_0, OP_SETLOCAL_1, OP_SETLOCAL_2, OP_SETLOCAL_3):
                reg = op - OP_SETLOCAL_0
                if reg not in local_types:
                    detected = last_coerce_type or last_push_type
                    if detected:
                        local_types[reg] = detected
                    elif last_was_default:
                        local_types[reg] = '*'  # mark as untyped
                last_coerce_type = None
                last_push_type = None
                last_was_default = False
                last_was_pushnull = False
            elif op == OP_SETLOCAL:
                idx, p2 = _ru30(code, p)
                p = p2
                if idx not in local_types:
                    detected = last_coerce_type or last_push_type
                    if detected:
                        local_types[idx] = detected
                    elif last_was_default:
                        local_types[idx] = '*'  # mark as untyped
                last_coerce_type = None
                last_push_type = None
                last_was_default = False
                last_was_pushnull = False
            else:
                # Branch ops have s24 operands, reset tracking and skip correctly
                if op in _BRANCH_OPS:
                    _, p = _rs24(code, p)
                elif op == OP_LOOKUPSWITCH:
                    _, p = _rs24(code, p)  # default offset
                    case_count, p = _ru30(code, p)
                    for _ in range(case_count + 1):
                        _, p = _rs24(code, p)
                else:
                    p = _skip_operands(op, code, p)
                # Any non-transparent op resets the coerce tracking
                if op not in (OP_DUP, OP_KILL, OP_POP):
                    last_coerce_type = None
                    last_push_type = None
                    last_was_default = False
                    last_was_pushnull = False
        return local_types

