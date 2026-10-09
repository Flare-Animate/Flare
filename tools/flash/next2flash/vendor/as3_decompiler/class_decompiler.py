"""Full AS3 class/interface decompiler."""

from __future__ import annotations

import logging
import os
import re
from typing import Dict, List, Set, Tuple

from .abc_parser import *
from .opcodes import *
from .helpers import *
from .method_decompiler import MethodDecompiler

logger = logging.getLogger(__name__)

# Without __all__, `from .class_decompiler import *` (done by __init__.py)
# would re-export every name this module itself imported via `import *`
# (abc_parser/opcodes/helpers), polluting the package namespace. Only the
# public class is meant to be re-exported from here.
__all__ = ['AS3Decompiler']

# Derived indent levels (from INDENT_UNIT imported from helpers)
_I1 = INDENT_UNIT          # 1 level (package body / file-scope class body)
_I2 = INDENT_UNIT * 2      # 2 levels (class members)
_I3 = INDENT_UNIT * 3      # 3 levels (method body)


# ═══════════════════════════════════════════════════════════════════════════
#  Class Decompiler — full AS3 source from class structures
# ═══════════════════════════════════════════════════════════════════════════

class AS3Decompiler:
    """Decompile an ABCFile back into AS3 source files."""

    def __init__(self, abc: ABCFile):
        logger.debug("AS3Decompiler.__init__: %d classes", len(abc.instances))
        self.abc = abc
        self.md = MethodDecompiler(abc)

    @staticmethod
    def _scan_wildcard_imports(abc: ABCFile, code: bytes, result: list):
        """Scan bytecodes for multinames with namespace sets → wildcard import packages.

        Only considers opcodes that reference *types* (coerce, astype, istype,
        findpropstrict, getlex, constructprop) and only when the multiname name
        starts with an uppercase letter (class-like references).  Property
        accesses (getproperty/setproperty/callproperty with lowercase names)
        are ignored to avoid polluting the wildcard list.
        """
        # Opcodes that reference types / class names
        TYPE_OPS = {OP_COERCE, OP_ASTYPE, OP_ISTYPE, OP_FINDPROPSTRICT,
                    OP_FINDPROPERTY, OP_FINDDEF, OP_GETLEX}
        TYPE_OPS2 = {OP_CONSTRUCTPROP}  # first u30 = multiname, second = argc
        # All opcodes with a single u30 multiname operand (for skipping)
        MN1 = {OP_GETSUPER, OP_SETSUPER, OP_GETPROPERTY, OP_SETPROPERTY,
                OP_INITPROPERTY, OP_DELETEPROPERTY, OP_GETDESCENDANTS,
                OP_FINDPROPSTRICT, OP_FINDPROPERTY, OP_FINDDEF, OP_GETLEX,
                OP_COERCE, OP_ASTYPE, OP_ISTYPE}
        MN2 = {OP_CALLSUPER, OP_CALLPROPERTY, OP_CONSTRUCTPROP,
                OP_CALLPROPLEX, OP_CALLSUPERVOID, OP_CALLPROPVOID}
        p = 0
        while p < len(code):
            op = code[p]; p += 1
            if op in MN1:
                mn_idx, p = _ru30(code, p)
                if op in TYPE_OPS:
                    _check_mn_ns_set_typed(abc, mn_idx, result)
            elif op in MN2:
                mn_idx, p = _ru30(code, p)
                _, p = _ru30(code, p)  # argc
                if op in TYPE_OPS2:
                    _check_mn_ns_set_typed(abc, mn_idx, result)
            elif op in (OP_IFNLT, OP_IFNLE, OP_IFNGT, OP_IFNGE,
                        OP_JUMP, OP_IFTRUE, OP_IFFALSE,
                        OP_IFEQ, OP_IFNE, OP_IFLT, OP_IFLE,
                        OP_IFGT, OP_IFGE, OP_IFSTRICTEQ, OP_IFSTRICTNE):
                p += 3  # branch s24 offset
            elif op == OP_LOOKUPSWITCH:
                p += 3  # default offset
                cnt, p = _ru30(code, p)
                p += (cnt + 1) * 3
            else:
                p = _skip_operands(op, code, p)

    @staticmethod
    def _scan_body_imports(abc: ABCFile, code: bytes, add_import_fn):
        """Scan bytecodes for type references in method bodies."""
        TYPE_OPS = {OP_COERCE, OP_ASTYPE, OP_ISTYPE}
        # FINDPROPSTRICT/GETLEX reference classes for new/getlex — only import
        # package-qualified names where the final component looks like a class
        CLASS_REF_OPS = {OP_FINDPROPSTRICT, OP_GETLEX}
        p = 0
        while p < len(code):
            op = code[p]; p += 1
            if op in TYPE_OPS:
                mn_idx, p = _ru30(code, p)
                if mn_idx < len(abc.multinames):
                    fqn = abc.mn_full(mn_idx)
                    if '.' in fqn and fqn != '*':
                        add_import_fn(fqn)
            elif op in CLASS_REF_OPS:
                mn_idx, p = _ru30(code, p)
                if mn_idx < len(abc.multinames):
                    fqn = abc.mn_full(mn_idx)
                    if '.' in fqn and fqn != '*':
                        final = fqn.rsplit('.', 1)[-1]
                        if final and final[0].isupper():
                            add_import_fn(fqn)
                    # For Multiname/MultinameA (namespace-set-based), do NOT
                    # generate specific imports — we cannot know which package
                    # the name resolves to without runtime info.  Instead,
                    # _scan_wildcard_imports adds the packages and they are
                    # emitted as wildcard (pkg.*) imports.
            elif op in (OP_IFNLT, OP_IFNLE, OP_IFNGT, OP_IFNGE,
                        OP_JUMP, OP_IFTRUE, OP_IFFALSE,
                        OP_IFEQ, OP_IFNE, OP_IFLT, OP_IFLE,
                        OP_IFGT, OP_IFGE, OP_IFSTRICTEQ, OP_IFSTRICTNE):
                p += 3
            elif op == OP_LOOKUPSWITCH:
                p += 3
                cnt, p = _ru30(code, p)
                p += (cnt + 1) * 3
            else:
                p = _skip_operands(op, code, p)

    def list_classes(self) -> List[dict]:
        """Return list of class info dicts."""
        logger.debug("list_classes: %d instances", len(self.abc.instances))
        result = []
        for ci, inst in enumerate(self.abc.instances):
            name = self.abc.mn_name(inst.name_idx)
            pkg = self.abc.mn_ns(inst.name_idx)
            super_name = self.abc.mn_full(inst.super_idx) if inst.super_idx else ''
            is_interface = bool(inst.flags & INSTANCE_Interface)
            result.append({
                'index': ci,
                'name': name,
                'package': pkg,
                'full_name': f'{pkg}.{name}' if pkg else name,
                'super': super_name,
                'is_interface': is_interface,
                'trait_count': len(inst.traits) + len(self.abc.classes[ci].traits),
            })
        return result

    def decompile_class(self, class_idx: int) -> str:
        """Decompile a single class into a full .as source file."""
        logger.debug("decompile_class: class_idx=%d", class_idx)
        abc = self.abc
        inst = abc.instances[class_idx]
        cls = abc.classes[class_idx]

        class_name = abc.mn_name(inst.name_idx)
        pkg = abc.mn_ns(inst.name_idx)
        super_full = abc.mn_full(inst.super_idx) if inst.super_idx else ''
        super_name = abc.mn_name(inst.super_idx) if inst.super_idx else ''
        is_interface = bool(inst.flags & INSTANCE_Interface)
        is_final = bool(inst.flags & INSTANCE_Final)
        is_sealed = bool(inst.flags & INSTANCE_Sealed)

        # Collect imports needed (preserve first-occurrence order)
        imports: List[str] = []
        _imports_seen: Set[str] = set()
        def _add_import(fqn: str):
            if '.' in fqn and fqn != '*' and fqn not in _imports_seen:
                _imports_seen.add(fqn)
                imports.append(fqn)

        wildcard_imports: list = []
        if super_full:
            _add_import(super_full)
        for intf_idx in inst.interfaces:
            intf_full = abc.mn_full(intf_idx)
            if '.' in intf_full:
                _add_import(intf_full)
            else:
                # Interface multiname is namespace-set-based; resolve by
                # searching for a matching class definition in the ABC pool.
                _intf_name = abc.mn_name(intf_idx)
                for _ci2, _inst2 in enumerate(abc.instances):
                    _cn = abc.mn_name(_inst2.name_idx)
                    if _cn == _intf_name:
                        _cp = abc.mn_ns(_inst2.name_idx)
                        if _cp:
                            _add_import(f'{_cp}.{_cn}')
                            break
                else:
                    # Not in our ABC — try namespace set packages
                    kind_i, data_i = abc.multinames[intf_idx]
                    if kind_i in (MN_Multiname, MN_MultinameA) and data_i:
                        ns_set = abc.ns_sets[data_i[1]]
                        for ns_idx in ns_set:
                            ns_nm = abc.ns_name(ns_idx)
                            if ns_nm and ns_nm[0].islower() and ':' not in ns_nm and ns_nm != 'http://adobe.com/AS3/2006/builtin':
                                _add_import(f'{ns_nm}.*')

        # Scan constructor (iinit) and static initializer (cinit) param/return types
        for _init_mi in (inst.iinit, cls.cinit):
            if _init_mi < len(abc.methods):
                _init_m = abc.methods[_init_mi]
                for pt in _init_m.param_types:
                    if pt:
                        _add_import(abc.mn_full(pt))
                if _init_m.return_type:
                    _add_import(abc.mn_full(_init_m.return_type))

        # Scan all traits (slots, methods) in single pass to preserve trait order
        for trait in inst.traits + cls.traits:
            if trait.kind in (TRAIT_Slot, TRAIT_Const) and trait.type_name:
                _add_import(abc.mn_full(trait.type_name))
            elif trait.kind in (TRAIT_Method, TRAIT_Getter, TRAIT_Setter, TRAIT_Function):
                mi = trait.method_idx
                if mi < len(abc.methods):
                    m = abc.methods[mi]
                    for pt in m.param_types:
                        if pt:
                            _add_import(abc.mn_full(pt))
                    if m.return_type:
                        _add_import(abc.mn_full(m.return_type))

        # Scan method bodies for MultinameL/Multiname with namespace sets → wildcard imports
        # Also scan for specific type references (coerce, astype, istype)
        method_indices = [inst.iinit, cls.cinit]
        for trait in inst.traits + cls.traits:
            if trait.kind in (TRAIT_Method, TRAIT_Getter, TRAIT_Setter, TRAIT_Function):
                method_indices.append(trait.method_idx)
        # Discover closure (NEWFUNCTION) method indices recursively, preserving order
        scanned = set(method_indices)
        closure_indices = []
        queue = list(method_indices)
        while queue:
            mi = queue.pop(0)  # FIFO for stable BFS order
            body = abc.method_bodies.get(mi)
            if body:
                code = body.code
                p2 = 0
                while p2 < len(code):
                    op2 = code[p2]; p2 += 1
                    if op2 == OP_NEWFUNCTION:
                        child_mi, p2 = _ru30(code, p2)
                        if child_mi not in scanned:
                            scanned.add(child_mi)
                            closure_indices.append(child_mi)
                            queue.append(child_mi)
                    else:
                        p2 = _skip_operands(op2, code, p2)
        # Scan closure method signatures for imports (param types, return types)
        for mi in closure_indices:
            if mi < len(abc.methods):
                m = abc.methods[mi]
                for pt in m.param_types:
                    if pt:
                        _add_import(abc.mn_full(pt))
                if m.return_type:
                    _add_import(abc.mn_full(m.return_type))
        # Scan ALL method bodies (original + closures) for type references
        all_method_indices = method_indices + closure_indices
        for mi in all_method_indices:
            body = abc.method_bodies.get(mi)
            if body:
                self._scan_wildcard_imports(abc, body.code, wildcard_imports)
                self._scan_body_imports(abc, body.code, _add_import)

        # Remove self-package imports
        imports = [imp for imp in imports
                   if not imp.startswith(pkg + '.') or imp.count('.') > pkg.count('.') + 1]
        # Remove multiname-style imports containing ':' (e.g. fl.motion:ColorMatrix.LUMINANCEB)
        imports = [imp for imp in imports if ':' not in imp]
        # Remove internal __AS3__.vec.Vector imports (Vector doesn't need explicit imports)
        imports = [imp for imp in imports if not imp.startswith('__AS3__')]

        # Build source
        lines: List[str] = []
        # Header comment with full class name (like AS3 Sorcerer)
        full_name = f'{pkg}.{class_name}' if pkg else class_name
        lines.append(f'package {pkg}' if pkg else 'package')
        lines.append('{')

        # Import statements (keep discovery order, not sorted)
        all_imports = imports
        # Add wildcard imports with AS3 Sorcerer ordering:
        # Priority packages first (display, geom, events, media, filters, utils),
        # then remaining alphabetically
        _WILD_PRIORITY = ['flash.display', 'flash.geom', 'flash.events',
                          'flash.media', 'flash.filters', 'flash.utils']
        wild_pkgs = [w for w in wildcard_imports if w and w != pkg and not w.startswith('__AS3__')]
        # Deduplicate wildcard packages while preserving order
        seen_wild = set()
        deduped_wild = []
        for w in wild_pkgs:
            if w not in seen_wild:
                seen_wild.add(w)
                deduped_wild.append(w)
        wild_pkgs = deduped_wild

        # Remove wildcard packages that would shadow an explicitly imported class.
        # E.g. if we have `import flash.utils.Dictionary;` and wildcard
        # `com.some.pkg.*` also contains a `Dictionary` class, the wildcard
        # would create an ambiguity error in mxmlc.
        explicit_simple_names: Dict[str, str] = {}  # simple_name → package
        for imp in all_imports:
            parts = imp.rsplit('.', 1)
            if len(parts) == 2:
                explicit_simple_names[parts[1]] = parts[0]
        # Build class-name-to-package map from the ABCFile.
        # Include both user-defined classes (instances) AND classes referenced
        # via QName multinames (which covers built-in flash.* types).
        pkg_classes: Dict[str, Set[str]] = {}  # package → set of simple class names
        for ci2 in range(len(abc.instances)):
            mn = abc.instances[ci2].name_idx
            fqn = abc.mn_full(mn)
            if '.' in fqn:
                cpkg, cname = fqn.rsplit('.', 1)
                pkg_classes.setdefault(cpkg, set()).add(cname)
        # Also scan QName multinames for built-in class references
        for _mn_kind, _mn_data in abc.multinames:
            if _mn_kind in (MN_QName, MN_QNameA) and _mn_data and len(_mn_data) >= 2:
                _mn_fqn = None
                _ns_idx, _name_idx = _mn_data
                if _ns_idx < len(abc.namespaces) and _name_idx < len(abc.strings):
                    _ns_k = abc.ns_kind(_ns_idx)
                    if _ns_k == NS_PackageNamespace:
                        _ns_nm = abc.ns_name(_ns_idx)
                        _cn_nm = abc.strings[_name_idx]
                        if _ns_nm and _cn_nm and _cn_nm[0].isupper():
                            pkg_classes.setdefault(_ns_nm, set()).add(_cn_nm)
        safe_wild = []
        for w in wild_pkgs:
            classes_in_pkg = pkg_classes.get(w, set())
            # Check if any class in this wildcard package shadows an explicit import
            shadowed = False
            for cn in classes_in_pkg:
                if cn in explicit_simple_names and explicit_simple_names[cn] != w:
                    shadowed = True
                    break
            if not shadowed:
                safe_wild.append(w)
        wild_pkgs = safe_wild

        priority = [w for w in _WILD_PRIORITY if w in wild_pkgs]
        rest = [w for w in wild_pkgs if w not in _WILD_PRIORITY]
        wild_list = [f'{w}.*' for w in priority + rest]
        for imp in all_imports:
            lines.append(f'    import {imp};')
        for imp in wild_list:
            lines.append(f'    import {imp};')
        if all_imports or wild_list:
            lines.append('')

        # Class declaration
        decl_parts = ['    public']
        if is_final:
            decl_parts.append('final')
        if not is_sealed and not is_interface:
            decl_parts.append('dynamic')
        if is_interface:
            decl_parts.append('interface')
        else:
            decl_parts.append('class')
        decl_parts.append(class_name)
        if super_name and super_name not in ('Object', '*'):
            decl_parts.append(f'extends {super_name}')
        if inst.interfaces:
            intf_names = [abc.mn_name(ii) for ii in inst.interfaces]
            kw = 'extends' if is_interface else 'implements'
            decl_parts.append(f'{kw} {", ".join(intf_names)}')
        lines.append(' '.join(decl_parts))
        lines.append('    {')
        lines.append('')  # blank line after class opening brace

        # ── Static (class) traits ─────────────────────────────────
        static_vars = [t for t in cls.traits if t.kind in (TRAIT_Slot, TRAIT_Const)]
        static_methods = [t for t in cls.traits if t.kind in (TRAIT_Method, TRAIT_Getter, TRAIT_Setter, TRAIT_Function)]
        # Sort: consts before vars (matching AS3 Sorcerer output)
        static_vars.sort(key=lambda t: (0 if t.kind == TRAIT_Const else 1))

        last_kind = None
        for t in static_vars:
            if last_kind == TRAIT_Const and t.kind != TRAIT_Const:
                lines.append('')  # blank between consts and vars
            lines.append(self._decompile_var_trait(t, static=True))
            last_kind = t.kind

        # ── Static initializer (cinit) ────────────────────────────
        cinit_block_stmts = []
        if not is_interface and cls.cinit is not None:
            cinit_src = self.md.decompile(cls.cinit, indent='',
                                          class_idx=class_idx, is_static=True,
                                          class_name=class_name)
            cinit_stmts_raw = [l.strip() for l in cinit_src.split('\n') if l.strip()]
            # Reassemble multi-line statements (e.g., VAR = { ... };)
            cinit_stmts = []
            accum = ''
            brace_depth = 0
            for raw_line in cinit_stmts_raw:
                if accum:
                    accum += '\n' + raw_line
                else:
                    accum = raw_line
                brace_depth += raw_line.count('{') - raw_line.count('}')
                if brace_depth <= 0:
                    cinit_stmts.append(accum)
                    accum = ''
                    brace_depth = 0
            if accum:
                cinit_stmts.append(accum)
            # Separate var initializations from other statements
            var_names = {abc.mn_name(t.name_idx) for t in static_vars}
            for stmt in cinit_stmts:
                matched_var = False
                for vn in var_names:
                    if stmt.startswith(f'{vn} = ') and stmt.endswith(';'):
                        init_val = stmt[len(vn) + 3:-1]
                        # Find and update the corresponding var declaration line
                        for idx in range(len(lines)):
                            if f' {vn}:' in lines[idx] and lines[idx].strip().endswith(';'):
                                if ' = ' not in lines[idx]:
                                    # No existing value — fold the cinit value in
                                    if '\n' in init_val:
                                        # Multi-line value: expand with proper indentation
                                        base_indent = len(lines[idx]) - len(lines[idx].lstrip(' '))
                                        decl_prefix = lines[idx][:-1] + ' = '
                                        val_lines = init_val.split('\n')
                                        expanded = [decl_prefix + val_lines[0]]
                                        for vl in val_lines[1:]:
                                            expanded.append(' ' * (base_indent + len(INDENT_UNIT)) + vl)
                                        expanded[-1] = ' ' * base_indent + expanded[-1].strip()
                                        expanded[-1] += ';'
                                        lines[idx:idx+1] = expanded
                                    else:
                                        lines[idx] = lines[idx][:-1] + f' = {init_val};'
                                # If already has value, cinit is duplicate — suppress
                                matched_var = True
                                break
                        break
                if not matched_var:
                    cinit_block_stmts.append(stmt)

        # ── Instance traits (properties) ──────────────────────────
        inst_vars = [t for t in inst.traits if t.kind in (TRAIT_Slot, TRAIT_Const)]
        inst_methods = [t for t in inst.traits if t.kind in (TRAIT_Method, TRAIT_Getter, TRAIT_Setter, TRAIT_Function)]

        if static_vars and inst_vars:
            lines.append('')  # blank line between static and instance vars

        for t in inst_vars:
            lines.append(self._decompile_var_trait(t, static=False))

        # Determine if constructor will be emitted (needed for spacing decisions)
        has_ctor = False
        ctor_src = None
        if not is_interface:
            ctor_src = self._decompile_constructor(inst, class_name, class_idx=class_idx)
            has_ctor = ctor_src is not None

        if static_vars or inst_vars:
            lines.append('')
        if static_vars and inst_vars and not has_ctor:
            lines.append('')  # extra blank when both groups present and no constructor

        # ── Static initializer block (cinit statements) ───────────
        if cinit_block_stmts:
            lines.append(f'{_I2}{{')
            for stmt in cinit_block_stmts:
                lines.append(f'{_I3}{stmt}')
            lines.append(f'{_I2}}}')
            lines.append('')
            lines.append('')  # extra blank after cinit block (like AS3 Sorcerer)

        # ── Constructor ───────────────────────────────────────────
        if has_ctor:
            lines.append(ctor_src)
            lines.append('')

        # Extra blank between vars and methods when no constructor or cinit block emitted
        # (only needed when single var group without cinit — other cases already have enough blanks)
        if (not has_ctor and not cinit_block_stmts
            and (static_vars or inst_vars) and not (static_vars and inst_vars)
            and (inst_methods or static_methods)):
            lines.append('')

        # ── Static methods ────────────────────────────────────────
        for t in static_methods:
            lines.append(self._decompile_method_trait(t, static=True, is_interface=is_interface, class_idx=class_idx, class_name=class_name))
            lines.append('')

        # Extra blank between static and instance methods
        if static_methods and inst_methods:
            lines.append('')

        # ── Instance methods ──────────────────────────────────────
        for t in inst_methods:
            lines.append(self._decompile_method_trait(t, static=False, is_interface=is_interface, class_idx=class_idx, class_name=class_name))
            lines.append('')

        # Close — add extra blank before closing if class has methods or no constructor
        has_methods = bool(inst_methods or static_methods)
        if has_methods or not has_ctor:
            lines.append('')
        lines.append(f'{_I1}}}')
        lines.append('}')

        # Emit file-scope (non-package) classes from the same script
        file_scope_src = self._emit_file_scope_classes(class_idx)
        if file_scope_src:
            lines.append('')
            lines.append(file_scope_src)
            lines.append('')  # extra trailing blank for file-scope classes

        return '\n'.join(lines) + '\n\n'

    def _emit_file_scope_classes(self, main_class_idx: int) -> str:
        """Find and emit non-package classes from the same script as main_class_idx."""
        abc = self.abc
        # Find which script contains this class
        script = None
        for si in abc.scripts:
            for t in si.traits:
                if t.kind == TRAIT_Class and t.class_idx == main_class_idx:
                    script = si
                    break
            if script:
                break
        if not script:
            return ''

        # Collect other class traits in this script (file-scope classes)
        sibling_classes = []
        for t in script.traits:
            if t.kind == TRAIT_Class and t.class_idx != main_class_idx:
                sibling_classes.append(t.class_idx)

        if not sibling_classes:
            return ''

        parts = []
        for sci in sibling_classes:
            sinst = abc.instances[sci]
            scls = abc.classes[sci]
            sname = abc.mn_name(sinst.name_idx)

            lines = [f'class {sname}']
            lines.append('{')
            lines.append('')

            # Instance vars
            for tr in sinst.traits:
                if tr.kind in (TRAIT_Slot, TRAIT_Const):
                    # _decompile_var_trait uses 2-level indent; strip to 1-level for file-scope
                    var_line = self._decompile_var_trait(tr, static=False)
                    lines.append(INDENT_UNIT + var_line.lstrip())
            lines.append('')
            lines.append('')
            lines.append('}')
            parts.append('\n'.join(lines))
        return '\n'.join(parts)

    def _decompile_var_trait(self, trait: TraitInfo, static: bool) -> str:
        abc = self.abc
        name = abc.mn_name(trait.name_idx)
        type_str = abc.type_name(trait.type_name) if trait.type_name else '*'
        ns_kind = abc.mn_ns_kind(trait.name_idx)

        access = _access_modifier(ns_kind)
        kw = 'const' if trait.kind == TRAIT_Const else 'var'
        prefix = 'static ' if static else ''

        default = ''
        if trait.vindex:
            val_str = abc.default_value_str(trait.vkind, trait.vindex)
            # Format integer constants >= 256 as hex
            if trait.vkind == CONSTANT_Int and type_str == 'int':
                ival = abc.integers[trait.vindex] if trait.vindex < len(abc.integers) else 0
                if ival >= 256:
                    val_str = _fmt_hex_const(ival)
            elif trait.vkind == CONSTANT_UInt and type_str == 'uint':
                uval = abc.uintegers[trait.vindex] if trait.vindex < len(abc.uintegers) else 0
                if uval >= 256:
                    val_str = _fmt_hex_const(uval)
            # Append .0 for Number-typed traits with integer-valued defaults
            if type_str == 'Number' and re.match(r'^-?\d+$', val_str):
                val_str += '.0'
            default = f' = {val_str}'

        return f'{_I2}{access} {prefix}{kw} {name}:{type_str}{default};'

    def _decompile_constructor(self, inst: InstanceInfo, class_name: str, class_idx: int = -1):
        """Returns constructor source, or None if it should be omitted (empty no-arg constructor)."""
        abc = self.abc
        mi = inst.iinit
        m = abc.methods[mi] if mi < len(abc.methods) else None
        params_str = self._format_params(m) if m else ''

        ret_type = abc.type_name(m.return_type) if m else 'void'
        ret_suffix = f':{ret_type}' if ret_type and ret_type != '*' else ''

        body_src = self.md.decompile(mi, indent=_I3, class_idx=class_idx,
                                     is_static=False, class_name=class_name)
        # Remove implicit no-arg super() calls only when superclass is Object
        # (compiler always inserts constructsuper; for Object subclasses it's implicit)
        super_name = abc.mn_name(inst.super_idx) if inst.super_idx else ''
        body_lines = body_src.rstrip().split('\n') if body_src.strip() else []
        # Find first non-empty line
        first_real = -1
        for i, l in enumerate(body_lines):
            if l.strip():
                first_real = i
                break
        if first_real >= 0 and body_lines[first_real].strip() == 'super();':
            if not super_name or super_name in ('Object', '*'):
                body_lines.pop(first_real)
        body_src = '\n'.join(body_lines)

        # Omit constructor if it has no params and no body (implicit default ctor)
        if not params_str and not body_src.strip():
            return None

        lines = [f'{_I2}public function {class_name}({params_str}){ret_suffix}']
        lines.append(f'{_I2}{{')
        if body_src.strip():
            lines.append(body_src)
        lines.append(f'{_I2}}}')
        return '\n'.join(lines)

    def _decompile_method_trait(self, trait: TraitInfo, static: bool,
                                 is_interface: bool, class_idx: int = -1,
                                 class_name: str = '') -> str:
        abc = self.abc
        name = abc.mn_name(trait.name_idx)
        mi = trait.method_idx
        m = abc.methods[mi] if mi < len(abc.methods) else None
        ns_kind = abc.mn_ns_kind(trait.name_idx)

        access = _access_modifier(ns_kind)
        prefix_parts = []
        if trait.attr & 0x01:  # ATTR_Final
            # Suppress 'final' for static methods — static methods can't be overridden
            if not static:
                prefix_parts.append('final')
        if trait.attr & 0x02:  # ATTR_Override
            prefix_parts.append('override')
        # AS3 interface members are implicitly public — access modifier is illegal
        if not is_interface:
            prefix_parts.append(access)
        if static:
            prefix_parts.append('static')

        if trait.kind == TRAIT_Getter:
            prefix_parts.append('function get')
        elif trait.kind == TRAIT_Setter:
            prefix_parts.append('function set')
        else:
            prefix_parts.append('function')

        params_str = self._format_params(m) if m else ''
        ret_type = abc.type_name(m.return_type) if m else '*'
        if trait.kind == TRAIT_Setter:
            ret_type = 'void'

        prefix = ' '.join(prefix_parts)
        sig = f'{_I2}{prefix} {name}({params_str}):{ret_type}'

        if is_interface:
            return f'{sig};'

        lines = [sig]
        lines.append(f'{_I2}{{')
        body_src = self.md.decompile(mi, indent=_I3, class_idx=class_idx,
                                     is_static=static, class_name=class_name)
        if body_src.strip():
            lines.append(body_src.rstrip())
        lines.append(f'{_I2}}}')
        return '\n'.join(lines)

    def _format_params(self, m: MethodInfo) -> str:
        abc = self.abc
        params = []
        num_required = m.param_count - len(m.optional_values)

        for i in range(m.param_count):
            pname = ''
            if i < len(m.param_names):
                pname = abc.strings[m.param_names[i]] if m.param_names[i] < len(abc.strings) else ''
            if not pname:
                pname = f'_arg_{i + 1}'

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

        return ', '.join(params)

    def decompile_all(self, outdir: str) -> int:
        """Decompile all classes to .as files under outdir. Return count."""
        logger.info("decompile_all: %d classes -> %s", len(self.abc.instances), outdir)
        count = 0
        total = len(self.abc.instances)
        for ci in range(total):
            try:
                src = self.decompile_class(ci)
                info = self.list_classes()[ci]
                pkg = info['package']
                name = info['name']
                full_name = f'{pkg}.{name}' if pkg else name
                logger.info(f'  [{ci + 1}/{total}] {full_name}')

                # Create package directory
                if pkg:
                    pkg_dir = os.path.join(outdir, pkg.replace('.', os.sep))
                else:
                    pkg_dir = outdir
                os.makedirs(pkg_dir, exist_ok=True)

                filepath = os.path.join(pkg_dir, f'{name}.as')
                with open(filepath, 'w', encoding='utf-8') as f:
                    f.write(src)
                count += 1
            except (IndexError, ValueError, KeyError, AttributeError, IOError, OSError) as e:
                logger.warning(f'Error decompiling class #{ci}: {e}')
        return count

    # ═══════════════════════════════════════════════════════════════════
    #  Script-level (non-class) code  — issue #27
    # ═══════════════════════════════════════════════════════════════════

    def list_scripts(self) -> List[dict]:
        """Return info about each script's non-class traits (top-level functions,
        variables, constants).

        Each entry has:
            index       — script index
            sinit       — method index of the script initializer
            functions   — list of (name, method_idx)
            variables   — list of (name, type_str, kind_str='var'|'const')
            class_count — how many TRAIT_Class traits (already covered by decompile_class)
        """
        abc = self.abc
        result: List[dict] = []
        for si_idx, si in enumerate(abc.scripts):
            funcs: List[Tuple[str, int]] = []
            varlist: List[Tuple[str, str, str]] = []
            class_count = 0
            for t in si.traits:
                name = abc.mn_name(t.name_idx)
                if t.kind == TRAIT_Class:
                    class_count += 1
                elif t.kind == TRAIT_Function:
                    funcs.append((name, t.method_idx))
                elif t.kind in (TRAIT_Method, TRAIT_Getter, TRAIT_Setter):
                    funcs.append((name, t.method_idx))
                elif t.kind == TRAIT_Slot:
                    type_str = abc.type_name(t.type_name) if t.type_name else '*'
                    varlist.append((name, type_str, 'var'))
                elif t.kind == TRAIT_Const:
                    type_str = abc.type_name(t.type_name) if t.type_name else '*'
                    varlist.append((name, type_str, 'const'))
            result.append({
                'index': si_idx,
                'sinit': si.sinit,
                'functions': funcs,
                'variables': varlist,
                'class_count': class_count,
            })
        return result

    def decompile_script(self, script_idx: int) -> str:
        """Decompile script-level code: top-level variables, functions, and
        the script initializer body.

        Returns AS3 source string.  Script-level TRAIT_Class entries are
        skipped (use ``decompile_class()`` for those).
        """
        abc = self.abc
        if script_idx < 0 or script_idx >= len(abc.scripts):
            raise IndexError(f'Script index {script_idx} out of range (0..{len(abc.scripts) - 1})')
        si = abc.scripts[script_idx]
        lines: List[str] = []

        # ── Top-level variables / constants ──
        for t in si.traits:
            if t.kind in (TRAIT_Slot, TRAIT_Const):
                name = abc.mn_name(t.name_idx)
                type_str = abc.type_name(t.type_name) if t.type_name else '*'
                kw = 'const' if t.kind == TRAIT_Const else 'var'
                if t.vindex:
                    val = abc.default_value_str(t.vkind, t.vindex)
                    lines.append(f'{kw} {name}:{type_str} = {val};')
                else:
                    lines.append(f'{kw} {name}:{type_str};')

        # ── Top-level functions ──
        for t in si.traits:
            if t.kind == TRAIT_Function:
                name = abc.mn_name(t.name_idx)
                mi = t.method_idx
                m = abc.methods[mi] if mi < len(abc.methods) else None
                params_str = self._format_params(m) if m else ''
                ret_type = abc.type_name(m.return_type) if m else '*'
                lines.append('')
                lines.append(f'function {name}({params_str}):{ret_type}')
                lines.append('{')
                body_src = self.md.decompile(mi, indent=INDENT_UNIT)
                if body_src.strip():
                    lines.append(body_src.rstrip())
                lines.append('}')
            elif t.kind in (TRAIT_Method, TRAIT_Getter, TRAIT_Setter):
                name = abc.mn_name(t.name_idx)
                mi = t.method_idx
                m = abc.methods[mi] if mi < len(abc.methods) else None
                params_str = self._format_params(m) if m else ''
                ret_type = abc.type_name(m.return_type) if m else '*'
                prefix = 'function'
                if t.kind == TRAIT_Getter:
                    prefix = 'function get'
                elif t.kind == TRAIT_Setter:
                    prefix = 'function set'
                    ret_type = 'void'
                lines.append('')
                lines.append(f'{prefix} {name}({params_str}):{ret_type}')
                lines.append('{')
                body_src = self.md.decompile(mi, indent=INDENT_UNIT)
                if body_src.strip():
                    lines.append(body_src.rstrip())
                lines.append('}')

        # ── Script initializer ──
        sinit_body = self.md.decompile(si.sinit, indent=INDENT_UNIT)
        # Only emit if there's meaningful code (skip bare returnvoid)
        stripped = sinit_body.strip()
        if stripped and stripped != 'return;':
            lines.append('')
            lines.append('// script initializer')
            lines.append('{')
            lines.append(sinit_body.rstrip())
            lines.append('}')

        return '\n'.join(lines) + '\n' if lines else ''

