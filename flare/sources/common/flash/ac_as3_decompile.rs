//! AVM2 method and class decompiler: stack simulation over the bytecode into
//! statement text, then a series of structuring passes (increments, compound
//! assignment, `&&`/`||`, try/catch, switch, loops, `for`/`for each`, if/else
//! chains) and the class / interface / script source emitter.
//!
//! Derived from Next2Flash's `app/lib/as3_decompiler/{helpers,
//! method_decompiler,class_decompiler}.py`
//! (https://github.com/SSF2-Mods-Official/Next2Flash, MIT, Copyright (c) 2026
//! masterwebx; see `flare/sources/THIRD-PARTY-NOTICES.md`). The Python is the
//! oracle for the golden tests in `ac_as3_tests.rs`: the passes work on the
//! same statement strings in the same order, so the output text matches.
//!
//! Known deviations: the text of the `// decompile error:` comment differs
//! (Rust has no `IndexError` message); a method that makes Python raise
//! `OverflowError` (an infinite `pushdouble`) is reported as a decompile error
//! instead of aborting the run.

use super::ac_as3_abc::*;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

pub const INDENT_UNIT: &str = "    ";
const MAX_STRUCT_DEPTH: usize = 50;

fn fail(msg: &str) -> ! {
    std::panic::resume_unwind(Box::new(msg.to_string()))
}

macro_rules! re {
    ($name:ident, $pat:expr) => {
        static $name: LazyLock<Regex> = LazyLock::new(|| Regex::new($pat).unwrap());
    };
}
re!(RE_LABEL_COLON, r"^(__label_\d+):$");
re!(RE_LABEL_NUM_COLON, r"^__label_(\d+):$");
re!(RE_LABEL_WS, r"^\s*__label_\d+\s*:\s*$");
re!(RE_GOTO_LABEL, r"^goto (__label_\d+);$");
re!(RE_GOTO_LABEL_BARE, r"^goto __label_\d+;$");
re!(RE_IF_GOTO, r"^if \((.+)\) goto (__label_\d+);$");
re!(RE_IF_CMP_GOTO, r"^if \((.+?) (!==|===) (.+?)\) goto __label_\d+;$");
re!(RE_DEFAULT_GOTO, r"^(?:default: )?goto (__label_(\d+));$");
re!(RE_CASE_GOTO, r"^case \d+: goto __label_(\d+);$");
re!(RE_CASE_NUM_GOTO, r"^case (\d+): goto (__label_\d+);");
re!(RE_DEFAULT_GOTO2, r"^default: goto (__label_\d+);");
re!(RE_EQ_MATCH, r"^\((.+) (===?) (.+)\)$");
re!(RE_VAR_LOCAL, r"^var (_local_\d+):\S+ = (.+);$");
re!(RE_SIMPLE_IDENT, r"^[a-zA-Z_][\w.]*$");
re!(RE_NEG_INT, r"^-?\d+$");
re!(RE_WHILE_CLOSE, r"^\} while \((.+)\);$");
re!(RE_LOOP_LABEL, r"^(_loop_\d+:\s*)");
re!(RE_WHILE_COND, r"^while \((.+)\)$");
re!(RE_WHILE_HASNEXT, r"^while \(hasnext2\((\w+), (\w+)\)\)$");
re!(RE_TEMP_ASSIGN, r"^(?:var )?(_local_\d+)(?::\S+)? = (.+);$");
re!(RE_INC_DEC_EXPR, r"^(?:var )?(_local_\d+)(?::\S+)? = (?:(?:int|uint)\()?\((.+?) ([+-]) 1\)\)?;$");
re!(RE_BREAK_KW, r"^\s*(?:for\s*\(|for\s+each\s*\(|while\s*\(|do\s*$|do\s*\{|switch\s*\()");
re!(RE_VAR_DECL_INT, r"var\s+(\w+)\s*:\s*(int|uint)\b");
re!(RE_ASSIGN_CAST, r"^(\s*(?:var\s+)?(\w+)\s*(?::\s*\w+\s*)?=\s*)(int|uint)\((.+)\);$");
re!(RE_REGEXP_NEW, r#"new RegExp\("((?:[^"\\]|\\.)*)"\s*(?:,\s*"([^"]*)")?\)"#);
re!(RE_FOR_VAR_DECL, r"^var (\w+)(:\w[\w.<>]*)?;$");
re!(RE_FOR_VAR_INIT, r"^var (\w+)(:\w[\w.<>]*)?\s*=\s*(.+);$");
re!(RE_FOR_ASSIGN, r"^(\w+)\s*=\s*(.+);$");

fn rx(p: &str) -> Regex {
    Regex::new(p).unwrap()
}
fn esc(s: &str) -> String {
    regex::escape(s)
}

/// Global / top-level AS3 functions that do not get a `this.` prefix.
const GLOBAL_FUNCTIONS: &[&str] = &[
    "trace", "parseInt", "parseFloat", "isNaN", "isFinite", "isXMLName", "escape", "unescape", "encodeURI",
    "encodeURIComponent", "decodeURI", "decodeURIComponent", "String", "Number", "int", "uint", "Boolean",
    "Array", "Object", "XML", "XMLList", "RegExp", "Date", "Vector", "Error", "TypeError", "RangeError",
    "ReferenceError", "ArgumentError", "EvalError", "URIError", "SecurityError", "VerifyError",
    "DefinitionError", "SyntaxError", "UninitializedError", "getDefinitionByName", "getQualifiedClassName",
    "getQualifiedSuperclassName", "getTimer", "describeType", "setTimeout", "setInterval", "clearTimeout",
    "clearInterval",
];
fn is_global_fn(n: &str) -> bool {
    GLOBAL_FUNCTIONS.contains(&n)
}

// ---------------------------------------------------------------- helpers.py

fn pop_n(stack: &mut Vec<String>, n: usize, error_log: &mut Vec<String>, pos: &str) -> Vec<String> {
    let mut args = Vec::new();
    for _ in 0..n {
        if let Some(v) = stack.pop() {
            args.push(v);
        } else {
            let mut msg = format!("Stack underflow (expected {n} items)");
            if !pos.is_empty() {
                msg = format!("{msg} at {pos}");
            }
            args.push("?".to_string());
            error_log.push(msg);
        }
    }
    args.reverse();
    args
}

fn is_type_default(ltype: &str, value: &str) -> bool {
    (ltype == "int" && value == "0")
        || (ltype == "uint" && value == "0")
        || (ltype == "Boolean" && value == "false")
        || (!matches!(ltype, "*" | "int" | "uint" | "Number" | "Boolean" | "String") && value == "null")
}

fn strip_redundant_cast(ltype: &str, value: &str) -> String {
    let prefix = match ltype {
        "int" => "int(",
        "uint" => "uint(",
        _ => return value.to_string(),
    };
    if value.starts_with(prefix) && value.ends_with(')') {
        let inner = &value[prefix.len()..value.len() - 1];
        let mut depth = 0i32;
        for ch in inner.chars() {
            if ch == '(' {
                depth += 1;
            } else if ch == ')' {
                depth -= 1;
            }
            if depth < 0 {
                return value.to_string();
            }
        }
        if depth == 0 {
            return inner.to_string();
        }
    }
    value.to_string()
}

fn py_isdigit(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

fn add_type_cast_if_needed(
    ltype: &str,
    value: &str,
    local_types: &HashMap<u32, String>,
    local_names: &HashMap<u32, String>,
) -> String {
    let v = value.trim();
    // dict iteration order of local_names is insertion order in Python; the
    // lookup finds the first register whose name equals `v`. Names are unique
    // per register in practice, so any order gives the same answer.
    let find = |v: &str| -> Option<u32> {
        let mut regs: Vec<&u32> = local_names.keys().collect();
        regs.sort();
        regs.into_iter().find(|r| local_names[r] == v).copied()
    };
    if ltype == "String" && !v.starts_with("String(") && !v.starts_with('"') {
        if let Some(reg) = find(v) {
            if let Some(vt) = local_types.get(&reg) {
                if matches!(vt.as_str(), "Number" | "int" | "uint") {
                    return format!("String({value})");
                }
            }
        }
    } else if ltype == "Number" && !v.starts_with("Number(") {
        if v.starts_with('"') || v.starts_with('\'') {
            return format!("Number({value})");
        }
        if let Some(reg) = find(v) {
            if local_types.get(&reg).map_or(false, |t| t == "String") {
                return format!("Number({value})");
            }
        }
    } else if ltype == "Boolean" && !v.starts_with("Boolean(") && py_isdigit(v.trim_start_matches('-')) {
        return format!("Boolean({value})");
    }
    value.to_string()
}

fn fmt_call(obj: &str, name: &str, args: &[String]) -> String {
    let a = args.join(", ");
    if obj.is_empty() || obj == "global" || obj == name {
        format!("{name}({a})")
    } else {
        format!("{obj}.{name}({a})")
    }
}

fn binop(stack: &mut Vec<String>, op: &str) {
    let b = stack.pop().unwrap_or_else(|| "?".into());
    let a = stack.pop().unwrap_or_else(|| "?".into());
    stack.push(format!("({a} {op} {b})"));
}

fn bitwise_binop(stack: &mut Vec<String>, op: &str) {
    let b = stack.pop().unwrap_or_else(|| "?".into());
    let a = stack.pop().unwrap_or_else(|| "?".into());
    stack.push(format!("({} {op} {})", to_hex_if_int(&a), to_hex_if_int(&b)));
}

fn fmt_hex(v: u128) -> String {
    let mut h = format!("{v:X}");
    if h.len() % 2 == 1 {
        h.insert(0, '0');
    }
    format!("0x{h}")
}

fn fmt_hex_const(v: u128) -> String {
    let mut h = format!("{v:X}");
    while h.len() < 4 {
        h.insert(0, '0');
    }
    format!("0x{h}")
}

/// Python `int(s)` for the cases the decompiler can meet: optional whitespace,
/// sign, ASCII digits with single underscores between digits.
fn py_int(s: &str) -> Option<i128> {
    let t = s.trim();
    let (neg, rest) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    if rest.is_empty() || rest.starts_with('_') || rest.ends_with('_') || rest.contains("__") {
        return None;
    }
    if !rest.chars().all(|c| c.is_ascii_digit() || c == '_') {
        return None;
    }
    let digits: String = rest.chars().filter(|&c| c != '_').collect();
    if digits.len() > 36 {
        return None;
    }
    let v: i128 = digits.parse().ok()?;
    Some(if neg { -v } else { v })
}

fn to_hex_if_int(s: &str) -> String {
    match py_int(s) {
        Some(v) if v >= 0 => fmt_hex(v as u128),
        _ => s.to_string(),
    }
}

pub fn escape_str(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        let cp = ch as u32;
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            '\x0c' => out.push_str("\\f"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            _ if cp < 0x20 || cp == 0x7F => out.push_str(&format!("\\x{cp:02X}")),
            _ => out.push(ch),
        }
    }
    out
}

fn expand_multiline_stmt(stmt: &str, base_indent: &str) -> Vec<String> {
    if !stmt.contains('\n') {
        return vec![format!("{base_indent}{stmt}")];
    }
    let mut result = Vec::new();
    let base = base_indent.chars().count();
    let leading = stmt.chars().take_while(|&c| c == ' ').count();
    let actual = base + leading;
    let mut indent_stack: Vec<usize> = vec![actual];
    let mut cur = base_indent.to_string();
    let width = INDENT_UNIT.len();
    let chars: Vec<char> = stmt.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\n' {
            result.push(cur.clone());
            let mut j = i + 1;
            while j < chars.len() && chars[j] == ' ' {
                j += 1;
            }
            if j < chars.len() && chars[j] == '}' {
                if indent_stack.len() > 1 {
                    cur = " ".repeat(indent_stack[indent_stack.len() - 2]);
                } else {
                    cur = " ".repeat(*indent_stack.last().unwrap());
                }
            } else {
                cur = " ".repeat(*indent_stack.last().unwrap());
            }
        } else if ch == '{' {
            cur.push(ch);
            let last = *indent_stack.last().unwrap();
            indent_stack.push(last + width);
        } else if ch == '}' {
            if indent_stack.len() > 1 {
                indent_stack.pop();
            }
            cur.push(ch);
        } else {
            cur.push(ch);
        }
        i += 1;
    }
    if !cur.trim().is_empty() {
        result.push(cur);
    }
    result
}

fn has_outer_parens(expr: &str) -> bool {
    if !expr.starts_with('(') || !expr.ends_with(')') {
        return false;
    }
    let mut depth = 0i32;
    let n = expr.len();
    for (i, c) in expr.bytes().enumerate() {
        if c == b'(' {
            depth += 1;
        } else if c == b')' {
            depth -= 1;
        }
        if depth == 0 && i + 1 < n {
            return false;
        }
    }
    true
}

fn needs_ternary_wrap(expr: &str) -> bool {
    if has_outer_parens(expr) {
        return false;
    }
    let b = expr.as_bytes();
    let mut depth = 0i32;
    let mut in_str = false;
    for i in 0..b.len() {
        let c = b[i];
        if c == b'"' && !in_str {
            in_str = true;
        } else if c == b'"' && in_str {
            in_str = false;
        }
        if in_str {
            continue;
        }
        if c == b'(' {
            depth += 1;
        } else if c == b')' {
            depth -= 1;
        }
        if depth == 0 && c == b' ' {
            let rest = &expr[i + 1..];
            for op in [
                "+", "-", "*", "/", "%", "&&", "||", "==", "!=", "===", "!==", "<", ">", "<=", ">=", "&", "|", "^",
                "<<", ">>", ">>>",
            ] {
                if rest.starts_with(&format!("{op} ")) || rest.starts_with(&format!("{op}(")) {
                    return true;
                }
            }
        }
    }
    false
}

fn wrap_for_logical(expr: &str, join_op: &str) -> String {
    if has_outer_parens(expr) {
        return expr.to_string();
    }
    let other = if join_op == "&&" { "||" } else { "&&" };
    let b = expr.as_bytes();
    let mut depth = 0i32;
    let mut i = 0;
    while i + 1 < b.len() {
        let c = b[i];
        if c == b'(' {
            depth += 1;
        } else if c == b')' {
            depth -= 1;
        } else if c == b'"' {
            i += 1;
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
        } else if depth == 0 && i + 2 <= b.len() && &expr.as_bytes()[i..i + 2] == other.as_bytes() {
            return format!("({expr})");
        }
        i += 1;
    }
    expr.to_string()
}

/// Byte offset of `op` at paren depth 0 outside string literals, or -1.
fn find_op_outside_parens(expr: &str, op: &str) -> i64 {
    let b = expr.as_bytes();
    let o = op.as_bytes();
    let mut depth = 0i32;
    let mut i = 0usize;
    while i + o.len() <= b.len() {
        let c = b[i];
        if c == b'(' {
            depth += 1;
        } else if c == b')' {
            depth -= 1;
        } else if c == b'"' || c == b'\'' {
            let q = c;
            i += 1;
            while i < b.len() && b[i] != q {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            continue;
        } else if depth == 0 && &b[i..i + o.len()] == o {
            let next = b.get(i + o.len()).copied();
            if (op == "==" || op == "!=") && next == Some(b'=') {
                i += 1;
                continue;
            }
            if (op == "<" || op == ">") && matches!(next, Some(b'=') | Some(b'<') | Some(b'>')) {
                i += 1;
                continue;
            }
            if op == "=" && i > 0 && matches!(b[i - 1], b'!' | b'<' | b'>' | b'=') {
                i += 1;
                continue;
            }
            return i as i64;
        }
        i += 1;
    }
    -1
}

/// Skip an instruction's operands; on malformed code returns `code.len()`.
fn skip_operands(op: u8, code: &[u8], p: usize) -> usize {
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| skip_operands_inner(op, code, p)));
    r.unwrap_or(code.len())
}
fn skip_operands_inner(op: u8, code: &[u8], p: usize) -> usize {
    match op {
        OP_PUSHBYTE => p + 1,
        OP_PUSHSHORT | OP_PUSHSTRING | OP_PUSHINT | OP_PUSHUINT | OP_PUSHDOUBLE | OP_PUSHNAMESPACE | OP_GETSUPER
        | OP_SETSUPER | OP_DXNS | OP_KILL | OP_NEWFUNCTION | OP_NEWCLASS | OP_NEWCATCH | OP_FINDPROPSTRICT
        | OP_FINDPROPERTY | OP_FINDDEF | OP_GETLEX | OP_SETPROPERTY | OP_GETLOCAL | OP_SETLOCAL
        | OP_GETSCOPEOBJECT | OP_GETPROPERTY | OP_INITPROPERTY | OP_DELETEPROPERTY | OP_GETSLOT | OP_SETSLOT
        | OP_GETGLOBALSLOT | OP_SETGLOBALSLOT | OP_COERCE | OP_ASTYPE | OP_ISTYPE | OP_INCLOCAL | OP_DECLOCAL
        | OP_INCLOCAL_I | OP_DECLOCAL_I | OP_GETDESCENDANTS | OP_DEBUGLINE | OP_DEBUGFILE | OP_CALL
        | OP_CONSTRUCT | OP_APPLYTYPE | OP_NEWOBJECT | OP_NEWARRAY | OP_CONSTRUCTSUPER => ru30(code, p).1,
        OP_CALLMETHOD | OP_CALLSTATIC | OP_CALLSUPER | OP_CALLPROPERTY | OP_CONSTRUCTPROP | OP_CALLPROPLEX
        | OP_CALLSUPERVOID | OP_CALLPROPVOID | OP_HASNEXT2 => {
            let (_, q) = ru30(code, p);
            ru30(code, q).1
        }
        OP_DEBUG => {
            let q = p + 1;
            let (_, q) = ru30(code, q);
            let q = q + 1;
            ru30(code, q).1
        }
        _ => p,
    }
}

fn check_mn_ns_set(abc: &AbcFile, mn_idx: u32, result: &mut Vec<String>) {
    let _ = (abc, mn_idx, result); // kept for parity with helpers.py; not used by the decompiler
}

fn push_unique(result: &mut Vec<String>, ns: String) {
    if !ns.is_empty() && !result.contains(&ns) {
        result.push(ns);
    }
}

fn add_package_nss(abc: &AbcFile, ns_set_idx: u32, result: &mut Vec<String>) {
    if ns_set_idx != 0 && (ns_set_idx as usize) < abc.ns_sets.len() {
        for &ns_idx in &abc.ns_sets[ns_set_idx as usize] {
            if abc.ns_kind(ns_idx) == NS_PACKAGE {
                push_unique(result, abc.ns_name(ns_idx));
            }
        }
    }
}

fn check_typename_param(abc: &AbcFile, mn_idx: u32, result: &mut Vec<String>, depth: u32) {
    if mn_idx as usize >= abc.multinames.len() || depth > 32 {
        return;
    }
    let m = &abc.multinames[mn_idx as usize];
    if m.kind == MN_TYPENAME && !m.data.is_empty() {
        for &px in &m.data[1..] {
            check_typename_param(abc, px, result, depth + 1);
        }
        return;
    }
    if matches!(m.kind, MN_QNAME | MN_QNAME_A) && m.data.len() >= 2 {
        let name = abc.strings.get(m.data[1] as usize).cloned().unwrap_or_default();
        if name.chars().next().map_or(false, |c| c.is_uppercase()) {
            let ns_idx = m.data[0];
            if (ns_idx as usize) < abc.namespaces.len() && abc.ns_kind(ns_idx) == NS_PACKAGE {
                push_unique(result, abc.ns_name(ns_idx));
            }
        }
        return;
    }
    check_mn_ns_set_typed_d(abc, mn_idx, result, depth + 1);
}

fn check_mn_ns_set_typed(abc: &AbcFile, mn_idx: u32, result: &mut Vec<String>) {
    check_mn_ns_set_typed_d(abc, mn_idx, result, 0)
}
fn check_mn_ns_set_typed_d(abc: &AbcFile, mn_idx: u32, result: &mut Vec<String>, depth: u32) {
    if mn_idx as usize >= abc.multinames.len() || depth > 32 {
        return;
    }
    let m = &abc.multinames[mn_idx as usize];
    let ns_set_idx;
    if m.kind == MN_TYPENAME && !m.data.is_empty() {
        for &px in &m.data[1..] {
            check_typename_param(abc, px, result, depth + 1);
        }
        return;
    } else if matches!(m.kind, MN_MULTINAME | MN_MULTINAME_A) && m.data.len() >= 2 {
        let name = abc.strings.get(m.data[0] as usize).cloned().unwrap_or_default();
        if !name.chars().next().map_or(false, |c| c.is_uppercase()) {
            return;
        }
        ns_set_idx = m.data[1];
    } else if matches!(m.kind, MN_MULTINAME_L | MN_MULTINAME_LA) && !m.data.is_empty() {
        ns_set_idx = m.data[0];
    } else {
        return;
    }
    add_package_nss(abc, ns_set_idx, result);
}

fn access_modifier(ns_kind: u8) -> &'static str {
    match ns_kind {
        NS_PRIVATE => "private",
        NS_PROTECTED | NS_STATIC_PROTECTED => "protected",
        NS_PACKAGE_INTERNAL => "internal",
        _ => "public",
    }
}

// ------------------------------------------------- local inc/dec pattern

const INC_OPS: [u8; 2] = [OP_INCREMENT, OP_INCREMENT_I];
const INCDEC_OPS: [u8; 4] = [OP_INCREMENT, OP_INCREMENT_I, OP_DECREMENT, OP_DECREMENT_I];

/// `_match_local_incdec`: `(is_pre, is_increment, new_p)`.
fn match_local_incdec(code: &[u8], p: usize, reg_idx: u32) -> Option<(bool, bool, usize)> {
    if p + 2 > code.len() {
        return None;
    }
    let b0 = code[p];
    let b1 = code.get(p + 1).copied().unwrap_or(0xFF);
    let check_setlocal = |pos: usize| -> Option<usize> {
        if pos >= code.len() {
            return None;
        }
        let op = code[pos];
        if reg_idx <= 3 && op == OP_SETLOCAL_0 + reg_idx as u8 {
            return Some(pos + 1);
        }
        if op == OP_SETLOCAL {
            if pos + 1 >= code.len() {
                return None;
            }
            let (idx, np) = ru30(code, pos + 1);
            if idx == reg_idx {
                return Some(np);
            }
        }
        None
    };
    if b0 == OP_DUP && INCDEC_OPS.contains(&b1) {
        let is_inc = INC_OPS.contains(&b1);
        if let Some(np) = check_setlocal(p + 2) {
            return Some((false, is_inc, np));
        }
    }
    if INCDEC_OPS.contains(&b0) && b1 == OP_DUP {
        let is_inc = INC_OPS.contains(&b0);
        if let Some(np) = check_setlocal(p + 2) {
            return Some((true, is_inc, np));
        }
    }
    None
}

fn is_branch_op(op: u8) -> bool {
    matches!(
        op,
        OP_IFNLT | OP_IFNLE | OP_IFNGT | OP_IFNGE | OP_JUMP | OP_IFTRUE | OP_IFFALSE | OP_IFEQ | OP_IFNE | OP_IFLT
            | OP_IFLE | OP_IFGT | OP_IFGE | OP_IFSTRICTEQ | OP_IFSTRICTNE
    )
}

fn is_cond_if(op: u8) -> bool {
    matches!(
        op,
        OP_IFEQ | OP_IFNE | OP_IFLT | OP_IFLE | OP_IFGT | OP_IFGE | OP_IFSTRICTEQ | OP_IFSTRICTNE | OP_IFNLT
            | OP_IFNLE | OP_IFNGT | OP_IFNGE
    )
}

// ---------------------------------------------------------------- contexts

#[derive(Clone, Debug, PartialEq)]
enum ScopeKind {
    Scope,
    Catch,
    With,
}

struct Run<'a> {
    abc: &'a AbcFile,
    code: &'a [u8],
    body: &'a MethodBody,
    method_idx: i64,
    class_idx: i64,
    is_static: bool,
    class_name: String,
    stmts: Vec<String>,
    stack: Vec<String>,
    scope: Vec<(ScopeKind, String)>,
    local0_name: String,
    local_names: HashMap<u32, String>,
    declared_locals: HashSet<u32>,
    param_count: u32,
    slot_map: HashMap<u32, String>,
    static_trait_names: HashSet<String>,
    activation_slots: HashMap<u32, String>,
    activation_slot_types: HashMap<u32, String>,
    declared_activation_slots: HashSet<u32>,
    p: usize,
    catch_scope_vars: HashMap<String, String>,
    local_types: HashMap<u32, String>,
    logical_combines: HashMap<usize, Vec<(String, String)>>,
    last_was_dup: bool,
    was_dup: bool,
    error_log: Vec<String>,
    construct_tmp_counter: u32,
}

struct Eval<'a> {
    abc: &'a AbcFile,
    code: &'a [u8],
    stack: Vec<String>,
    local_names: &'a HashMap<u32, String>,
    slot_map: &'a HashMap<u32, String>,
    local0_name: &'a str,
    is_static: bool,
    class_idx: i64,
    p: usize,
    bail: bool,
}

#[derive(Clone, Default)]
struct LoopCtx {
    continue_labels: HashSet<String>,
    break_label_map: HashMap<String, Option<String>>,
    loop_label: String,
}

pub struct MethodDecompiler<'a> {
    pub abc: &'a AbcFile,
    loop_label_counter: u32,
    needs_loop_label: HashSet<String>,
}

fn pop_or(stack: &mut Vec<String>) -> String {
    stack.pop().unwrap_or_else(|| "?".into())
}

type Ternary = (String, String, usize);

impl<'a> MethodDecompiler<'a> {
    pub fn new(abc: &'a AbcFile) -> Self {
        MethodDecompiler { abc, loop_label_counter: 0, needs_loop_label: HashSet::new() }
    }

    /// Decompile one method body into indented statement text.
    pub fn decompile(&mut self, method_idx: u32, indent: &str, class_idx: i64, is_static: bool, class_name: &str) -> String {
        let Some(body) = self.abc.method_bodies.get(&method_idx) else {
            return format!("{indent}// (no method body)\n");
        };
        let res = catch(|| {
            let mut stmts = self.run(body.code.as_slice(), body, method_idx as i64, class_idx, is_static, class_name);
            stmts = Self::fold_increments(stmts);
            stmts = Self::fold_compound_assign(stmts);
            stmts = Self::fold_inline_assignment(stmts);
            stmts = Self::fold_short_circuit_conditions(stmts);
            if !body.exceptions.is_empty() {
                stmts = self.fold_try_catch(stmts, body, &body.code);
            }
            stmts = Self::fold_switch(stmts);
            stmts = self.structure_flow(stmts);
            stmts = Self::fold_goto_dowhile(stmts);
            stmts = Self::fold_while_to_for(stmts);
            stmts = Self::fold_for_each_in(stmts);
            stmts = Self::fold_if_else_return_chains(stmts);
            stmts = Self::fold_regexp_literals(stmts);
            stmts = Self::fold_redundant_casts(stmts);
            let mut fin = Vec::new();
            for line in stmts {
                let st = line.trim();
                if st.contains("goto __label_") {
                    continue;
                }
                if RE_LABEL_WS.is_match(st) {
                    continue;
                }
                fin.push(line);
            }
            Self::remove_stray_breaks(fin)
        });
        let stmts = match res {
            Ok(s) => s,
            Err(e) => vec![format!("// decompile error: {e}")],
        };
        let mut lines = Vec::new();
        for s in &stmts {
            if !s.is_empty() {
                lines.extend(expand_multiline_stmt(s, indent));
            }
        }
        if lines.is_empty() {
            String::new()
        } else {
            lines.join("\n") + "\n"
        }
    }

    // ------------------------------------------------------------ _run

    fn run(&mut self, code: &[u8], body: &MethodBody, method_idx: i64, class_idx: i64, is_static: bool, class_name: &str) -> Vec<String> {
        let abc = self.abc;
        let local0_name = if is_static && !class_name.is_empty() { class_name.to_string() } else { "this".to_string() };
        let mut local_names: HashMap<u32, String> = HashMap::new();
        local_names.insert(0, local0_name.clone());
        let mut param_count = 0;
        if method_idx >= 0 && (method_idx as usize) < abc.methods.len() {
            let m = &abc.methods[method_idx as usize];
            param_count = m.param_count;
            for i in 0..m.param_count {
                let mut pname = String::new();
                if (i as usize) < m.param_names.len() {
                    let pn = m.param_names[i as usize] as usize;
                    if pn < abc.strings.len() {
                        pname = abc.strings[pn].clone();
                    }
                }
                if pname.is_empty() {
                    pname = format!("_arg_{}", i + 1);
                }
                local_names.insert(i + 1, pname);
            }
            if m.flags & METHOD_NEED_REST != 0 {
                local_names.insert(m.param_count + 1, "rest".into());
            }
        }
        let mut slot_map = HashMap::new();
        let mut static_trait_names = HashSet::new();
        if class_idx >= 0 && (class_idx as usize) < abc.instances.len() {
            for t in &abc.instances[class_idx as usize].traits {
                if matches!(t.kind, TRAIT_SLOT | TRAIT_CONST) && t.slot_id != 0 {
                    slot_map.insert(t.slot_id, abc.mn_name(t.name_idx));
                }
            }
            if let Some(ci) = abc.classes.get(class_idx as usize) {
                for t in &ci.traits {
                    if matches!(t.kind, TRAIT_SLOT | TRAIT_CONST) && t.slot_id != 0 {
                        slot_map.insert(t.slot_id, abc.mn_name(t.name_idx));
                    }
                    if matches!(t.kind, TRAIT_SLOT | TRAIT_CONST) {
                        static_trait_names.insert(abc.mn_name(t.name_idx));
                    }
                }
            } else {
                fail("list index out of range");
            }
        }
        let mut activation_slots = HashMap::new();
        let mut activation_slot_types = HashMap::new();
        for bt in &body.traits {
            if bt.slot_id != 0 {
                activation_slots.insert(bt.slot_id, abc.mn_name(bt.name_idx));
                activation_slot_types.insert(
                    bt.slot_id,
                    if bt.type_name != 0 { abc.type_name(bt.type_name) } else { "*".to_string() },
                );
            }
        }
        let mut targets: HashSet<usize> = HashSet::new();
        Self::prescan_branches(code, &mut targets);
        for ex in &body.exceptions {
            targets.insert(ex.from_pos as usize);
            targets.insert(ex.to_pos as usize);
            targets.insert(ex.target as usize);
        }
        let mut catch_entry_info: HashMap<usize, (usize, String)> = HashMap::new();
        for (ei_idx, ex) in body.exceptions.iter().enumerate() {
            let vn = if ex.var_name != 0 { abc.mn_name(ex.var_name) } else { "e".to_string() };
            catch_entry_info.insert(ex.target as usize, (ei_idx, vn));
        }
        let local_types = Self::prescan_local_types(code, abc);

        let mut ctx = Run {
            abc,
            code,
            body,
            method_idx,
            class_idx,
            is_static,
            class_name: class_name.to_string(),
            stmts: Vec::new(),
            stack: Vec::new(),
            scope: Vec::new(),
            local0_name,
            local_names,
            declared_locals: HashSet::new(),
            param_count,
            slot_map,
            static_trait_names,
            activation_slots,
            activation_slot_types,
            declared_activation_slots: HashSet::new(),
            p: 0,
            catch_scope_vars: HashMap::new(),
            local_types,
            logical_combines: HashMap::new(),
            last_was_dup: false,
            was_dup: false,
            error_log: Vec::new(),
            construct_tmp_counter: 0,
        };

        while ctx.p < code.len() {
            if ctx.logical_combines.contains_key(&ctx.p) && !ctx.stack.is_empty() {
                let entries = ctx.logical_combines.remove(&ctx.p).unwrap();
                let mut right = ctx.stack.last().unwrap().clone();
                for (op_str, left) in entries.iter().rev() {
                    let wl = wrap_for_logical(left, op_str);
                    let wr = wrap_for_logical(&right, op_str);
                    right = format!("{wl} {op_str} {wr}");
                }
                *ctx.stack.last_mut().unwrap() = right;
            }
            if targets.contains(&ctx.p) && ctx.p > 0 {
                ctx.stmts.push(format!("__label_{}:", ctx.p));
            }
            if let Some((_, ev)) = catch_entry_info.get(&ctx.p) {
                ctx.stack.clear();
                ctx.stack.push(ev.clone());
                ctx.scope.clear();
            }
            let op = code[ctx.p];
            ctx.p += 1;
            ctx.was_dup = ctx.last_was_dup;
            ctx.last_was_dup = false;
            self.dispatch(op, &mut ctx);
        }
        if !ctx.error_log.is_empty() {
            ctx.stmts.push(String::new());
            let errs = ctx.error_log.clone();
            for e in errs {
                ctx.stmts.push(format!("// ERROR: {e}"));
            }
        }
        ctx.stmts
    }

    fn dispatch(&mut self, op: u8, ctx: &mut Run) {
        match op {
            OP_GETLOCAL_0 | OP_GETLOCAL_1 | OP_GETLOCAL_2 | OP_GETLOCAL_3 | OP_GETLOCAL | OP_SETLOCAL_0
            | OP_SETLOCAL_1 | OP_SETLOCAL_2 | OP_SETLOCAL_3 | OP_SETLOCAL | OP_INCLOCAL | OP_INCLOCAL_I
            | OP_DECLOCAL | OP_DECLOCAL_I => self.h_local_ops(op, ctx),
            OP_PUSHBYTE | OP_PUSHSHORT | OP_PUSHSTRING | OP_PUSHINT | OP_PUSHUINT | OP_PUSHDOUBLE | OP_PUSHTRUE
            | OP_PUSHFALSE | OP_PUSHNULL | OP_PUSHUNDEFINED | OP_PUSHNAN | OP_PUSHNAMESPACE => self.h_push_ops(op, ctx),
            OP_PUSHSCOPE | OP_POPSCOPE | OP_PUSHWITH | OP_GETSCOPEOBJECT | OP_GETGLOBALSCOPE => self.h_scope_ops(op, ctx),
            OP_GETPROPERTY | OP_SETPROPERTY | OP_INITPROPERTY | OP_DELETEPROPERTY | OP_GETSLOT | OP_SETSLOT
            | OP_GETSUPER | OP_SETSUPER => self.h_property_ops(op, ctx),
            OP_FINDPROPSTRICT | OP_FINDPROPERTY | OP_GETLEX => self.h_find_ops(op, ctx),
            OP_CALLPROPERTY | OP_CALLPROPVOID | OP_CALLSUPER | OP_CALLSUPERVOID | OP_CALLPROPLEX | OP_CALL
            | OP_CALLMETHOD | OP_CALLSTATIC => self.h_call_ops(op, ctx),
            OP_CONSTRUCT | OP_CONSTRUCTSUPER | OP_CONSTRUCTPROP => self.h_construct_ops(op, ctx),
            OP_NEWOBJECT | OP_NEWARRAY | OP_NEWACTIVATION | OP_NEWFUNCTION | OP_NEWCLASS | OP_NEWCATCH
            | OP_APPLYTYPE | OP_GETDESCENDANTS => self.h_object_ops(op, ctx),
            OP_POP | OP_DUP | OP_SWAP => self.h_stack_ops(op, ctx),
            OP_CONVERT_S | OP_CONVERT_I | OP_CONVERT_U | OP_CONVERT_D | OP_CONVERT_B | OP_CONVERT_O | OP_COERCE_A
            | OP_COERCE_S | OP_COERCE_B | OP_COERCE_D | OP_COERCE_I | OP_COERCE_U | OP_COERCE_O | OP_COERCE
            | OP_ASTYPE | OP_ASTYPELATE | OP_ISTYPE | OP_ISTYPELATE | OP_INSTANCEOF | OP_TYPEOF | OP_CHECKFILTER
            | OP_ESC_XELEM | OP_ESC_XATTR => self.h_coerce_ops(op, ctx),
            OP_ADD | OP_ADD_I | OP_SUBTRACT | OP_SUBTRACT_I | OP_MULTIPLY | OP_MULTIPLY_I | OP_DIVIDE | OP_MODULO
            | OP_LSHIFT | OP_RSHIFT | OP_URSHIFT | OP_BITAND | OP_BITOR | OP_BITXOR | OP_NEGATE | OP_NEGATE_I
            | OP_NOT | OP_BITNOT | OP_INCREMENT | OP_INCREMENT_I | OP_DECREMENT | OP_DECREMENT_I => {
                self.h_arithmetic_ops(op, ctx)
            }
            OP_EQUALS | OP_STRICTEQUALS | OP_LESSTHAN | OP_LESSEQUALS | OP_GREATERTHAN | OP_GREATEREQUALS | OP_IN => {
                self.h_comparison_ops(op, ctx)
            }
            OP_RETURNVOID | OP_RETURNVALUE | OP_JUMP | OP_IFTRUE | OP_IFFALSE | OP_IFEQ | OP_IFNE | OP_IFLT
            | OP_IFLE | OP_IFGT | OP_IFGE | OP_IFSTRICTEQ | OP_IFSTRICTNE | OP_IFNLT | OP_IFNLE | OP_IFNGT
            | OP_IFNGE | OP_LOOKUPSWITCH => self.h_branch_ops(op, ctx),
            OP_NEXTNAME | OP_NEXTVALUE | OP_HASNEXT | OP_HASNEXT2 => self.h_iteration_ops(op, ctx),
            OP_THROW | OP_KILL | OP_DXNS | OP_DXNSLATE => self.h_misc_ops(op, ctx),
            OP_LI8 | OP_LI16 | OP_LI32 | OP_LF32 | OP_LF64 | OP_SI8 | OP_SI16 | OP_SI32 | OP_SF32 | OP_SF64
            | OP_SXI1 | OP_SXI8 | OP_SXI16 => self.h_memory_ops(op, ctx),
            OP_DEBUG | OP_DEBUGLINE | OP_DEBUGFILE => self.h_debug_ops(op, ctx),
            OP_BKPT | OP_NOP | OP_LABEL => {}
            OP_GETGLOBALSLOT | OP_SETGLOBALSLOT | OP_FINDDEF => self.h_global_slot_ops(op, ctx),
            _ => ctx.stmts.push(format!("// unknown opcode 0x{op:02X}")),
        }
    }

    // ---------------------------------------------------- run handlers

    fn local_name(ctx: &Run, reg: u32) -> String {
        ctx.local_names.get(&reg).cloned().unwrap_or_else(|| format!("_local_{reg}"))
    }

    fn h_local_ops(&mut self, op: u8, ctx: &mut Run) {
        match op {
            OP_GETLOCAL_0..=OP_GETLOCAL_3 => {
                let reg = (op - OP_GETLOCAL_0) as u32;
                let dflt = if reg == 0 { "this".to_string() } else { format!("_local_{reg}") };
                if let Some((pre, inc, np)) = match_local_incdec(ctx.code, ctx.p, reg) {
                    ctx.p = np;
                    let nm = ctx.local_names.get(&reg).cloned().unwrap_or(dflt);
                    let o = if inc { "++" } else { "--" };
                    ctx.stack.push(if pre { format!("{o}{nm}") } else { format!("{nm}{o}") });
                } else {
                    ctx.stack.push(ctx.local_names.get(&reg).cloned().unwrap_or(dflt));
                }
            }
            OP_GETLOCAL => {
                let (idx, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                if let Some((pre, inc, np)) = match_local_incdec(ctx.code, ctx.p, idx) {
                    ctx.p = np;
                    let nm = Self::local_name(ctx, idx);
                    let o = if inc { "++" } else { "--" };
                    ctx.stack.push(if pre { format!("{o}{nm}") } else { format!("{nm}{o}") });
                } else {
                    let nm = Self::local_name(ctx, idx);
                    ctx.stack.push(nm);
                }
            }
            OP_SETLOCAL_0..=OP_SETLOCAL_3 => Self::do_setlocal((op - OP_SETLOCAL_0) as u32, ctx),
            OP_SETLOCAL => {
                let (idx, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                Self::do_setlocal(idx, ctx);
            }
            OP_INCLOCAL | OP_INCLOCAL_I => {
                let (idx, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let nm = Self::local_name(ctx, idx);
                ctx.stmts.push(format!("{nm}++;"));
            }
            _ => {
                let (idx, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let nm = Self::local_name(ctx, idx);
                ctx.stmts.push(format!("{nm}--;"));
            }
        }
    }

    fn do_setlocal(reg: u32, ctx: &mut Run) {
        let mut v = pop_or(&mut ctx.stack);
        if v == "__activation__" && !ctx.activation_slots.is_empty() {
            ctx.local_names.insert(reg, "__activation__".into());
            ctx.last_was_dup = false;
            return;
        }
        if v.starts_with("__catch_scope_") && ctx.catch_scope_vars.contains_key(&v) {
            ctx.local_names.insert(reg, v);
            ctx.last_was_dup = false;
            return;
        }
        let nm = Self::local_name(ctx, reg);
        ctx.local_names.entry(reg).or_insert_with(|| nm.clone());
        if ctx.was_dup && ctx.stack.last().map_or(false, |t| *t == v) && !RE_SIMPLE_IDENT.is_match(&v) {
            *ctx.stack.last_mut().unwrap() = nm.clone();
        }
        if reg > 0 && !v.is_empty() {
            if !ctx.declared_locals.contains(&reg) && reg > ctx.param_count {
                ctx.declared_locals.insert(reg);
                let ltype = ctx.local_types.get(&reg).cloned().unwrap_or_else(|| "*".into());
                v = strip_redundant_cast(&ltype, &v);
                v = add_type_cast_if_needed(&ltype, &v, &ctx.local_types, &ctx.local_names);
                if ltype == "Number" && RE_NEG_INT.is_match(&v) {
                    v.push_str(".0");
                }
                if is_type_default(&ltype, &v) {
                    ctx.stmts.push(format!("var {nm}:{ltype};"));
                } else {
                    ctx.stmts.push(format!("var {nm}:{ltype} = {v};"));
                }
            } else {
                ctx.stmts.push(format!("{nm} = {v};"));
            }
        }
    }

    fn h_push_ops(&mut self, op: u8, ctx: &mut Run) {
        let abc = ctx.abc;
        match op {
            OP_PUSHBYTE => {
                let mut val = at(ctx.code, ctx.p) as i32;
                if val > 127 {
                    val -= 256;
                }
                ctx.p += 1;
                ctx.stack.push(val.to_string());
            }
            OP_PUSHSHORT => {
                let (v, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let mut val = v as i64;
                if val >= 0x2000_0000 {
                    val -= 0x4000_0000;
                }
                ctx.stack.push(val.to_string());
            }
            OP_PUSHSTRING => {
                let (idx, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let s = abc.strings.get(idx as usize).cloned().unwrap_or_else(|| "?".into());
                ctx.stack.push(format!("\"{}\"", escape_str(&s)));
            }
            OP_PUSHINT => {
                let (idx, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                ctx.stack.push(abc.integers.get(idx as usize).copied().unwrap_or(0).to_string());
            }
            OP_PUSHUINT => {
                let (idx, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                ctx.stack.push(abc.uintegers.get(idx as usize).copied().unwrap_or(0).to_string());
            }
            OP_PUSHDOUBLE => {
                let (idx, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let v = abc.doubles.get(idx as usize).copied().unwrap_or(0.0);
                ctx.stack.push(Self::fmt_pushdouble(v));
            }
            OP_PUSHTRUE => ctx.stack.push("true".into()),
            OP_PUSHFALSE => ctx.stack.push("false".into()),
            OP_PUSHNULL => ctx.stack.push("null".into()),
            OP_PUSHUNDEFINED => ctx.stack.push("undefined".into()),
            OP_PUSHNAN => ctx.stack.push("NaN".into()),
            _ => {
                let (_, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                ctx.stack.push("<namespace>".into());
            }
        }
    }

    fn fmt_pushdouble(v: f64) -> String {
        if !v.is_finite() {
            fail("cannot convert float to integer");
        }
        if v == v.trunc() && v.abs() < 1e15 {
            let iv = v as i64;
            if iv >= 256 && iv == (iv & 0xFFFF_FFFF) {
                fmt_hex(iv as u128)
            } else {
                iv.to_string()
            }
        } else {
            fmt_g(v, 15)
        }
    }

    fn h_scope_ops(&mut self, op: u8, ctx: &mut Run) {
        match op {
            OP_PUSHSCOPE => {
                let v = pop_or(&mut ctx.stack);
                if v.starts_with("__catch_scope_") {
                    ctx.scope.push((ScopeKind::Catch, v));
                } else {
                    ctx.scope.push((ScopeKind::Scope, v));
                }
            }
            OP_POPSCOPE => {
                if let Some((kind, val)) = ctx.scope.pop() {
                    if kind == ScopeKind::With {
                        ctx.stmts.push("}".into());
                    } else if kind == ScopeKind::Catch && ctx.catch_scope_vars.contains_key(&val) {
                        ctx.catch_scope_vars.remove(&val);
                    }
                }
            }
            OP_PUSHWITH => {
                let v = pop_or(&mut ctx.stack);
                ctx.scope.push((ScopeKind::With, v.clone()));
                ctx.stmts.push(format!("with ({v})"));
                ctx.stmts.push("{".into());
            }
            OP_GETSCOPEOBJECT => {
                let idx = at(ctx.code, ctx.p) as usize;
                // getscopeobject's operand is a u30 in Python (`_ru30`).
                let (idx2, np) = ru30(ctx.code, ctx.p);
                let _ = idx;
                ctx.p = np;
                let idx = idx2 as usize;
                if idx < ctx.scope.len() {
                    ctx.stack.push(ctx.scope[idx].1.clone());
                } else if !ctx.class_name.is_empty() {
                    ctx.stack.push(ctx.class_name.clone());
                } else {
                    ctx.stack.push(format!("scope{idx}"));
                }
            }
            _ => ctx.stack.push(String::new()),
        }
    }

    /// Pop the runtime name / namespace implied by a multiname, in that order.
    fn pop_rt(ctx: &mut Run, mn: u32) -> (Option<String>, Option<String>) {
        let rt_name = if !ctx.stack.is_empty() && ctx.abc.mn_needs_rt_name(mn) { ctx.stack.pop() } else { None };
        let rt_ns = if !ctx.stack.is_empty() && ctx.abc.mn_needs_rt_ns(mn) { ctx.stack.pop() } else { None };
        (rt_name, rt_ns)
    }

    fn prop_target(ctx: &Run, obj: &str, name: &str) -> String {
        if obj.is_empty() || obj == "global" || obj == name {
            name.to_string()
        } else if obj == "this" && !is_global_fn(name) {
            format!("this.{name}")
        } else if obj == "this" {
            name.to_string()
        } else if obj == ctx.local0_name && ctx.is_static {
            name.to_string()
        } else {
            format!("{obj}.{name}")
        }
    }

    fn h_property_ops(&mut self, op: u8, ctx: &mut Run) {
        let abc = ctx.abc;
        match op {
            OP_GETPROPERTY => {
                let (mn, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let (rt_name, _rt_ns) = Self::pop_rt(ctx, mn);
                let obj = pop_or(&mut ctx.stack);
                let is_attr = abc.mn_is_attr(mn);
                if let Some(rn) = rt_name {
                    if is_attr {
                        ctx.stack.push(format!("{obj}.@[{rn}]"));
                    } else {
                        ctx.stack.push(format!("{obj}[{rn}]"));
                    }
                } else {
                    let mut name = abc.mn_name(mn);
                    if name.is_empty() || name == "*" {
                        name = "*".into();
                    }
                    let ap = if is_attr { "@" } else { "" };
                    if obj.is_empty() || obj == "global" || obj == name {
                        ctx.stack.push(format!("{ap}{name}"));
                    } else if obj == "this" && !is_global_fn(&name) {
                        ctx.stack.push(format!("this.{ap}{name}"));
                    } else if obj == "this" {
                        ctx.stack.push(format!("{ap}{name}"));
                    } else if obj == ctx.local0_name && ctx.is_static {
                        ctx.stack.push(format!("{ap}{name}"));
                    } else {
                        ctx.stack.push(format!("{obj}.{ap}{name}"));
                    }
                }
            }
            OP_SETPROPERTY | OP_INITPROPERTY => {
                let (mn, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let mut val = pop_or(&mut ctx.stack);
                if val.starts_with("!(") {
                    val = format!("({val})");
                }
                let (rt_name, _) = Self::pop_rt(ctx, mn);
                let obj = pop_or(&mut ctx.stack);
                if let Some(rn) = rt_name {
                    ctx.stmts.push(format!("{obj}[{rn}] = {val};"));
                } else {
                    let name = abc.mn_name(mn);
                    let prop = Self::prop_target(ctx, &obj, &name);
                    ctx.stmts.push(format!("{prop} = {val};"));
                }
            }
            OP_DELETEPROPERTY => {
                let (mn, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let (rt_name, _) = Self::pop_rt(ctx, mn);
                let obj = pop_or(&mut ctx.stack);
                if let Some(rn) = rt_name {
                    ctx.stack.push(format!("delete {obj}[{rn}]"));
                } else {
                    let name = abc.mn_name(mn);
                    ctx.stack.push(if obj != "this" { format!("delete {obj}.{name}") } else { format!("delete {name}") });
                }
            }
            OP_GETSLOT => {
                let (idx, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let obj = pop_or(&mut ctx.stack);
                if obj == "__activation__" && ctx.activation_slots.contains_key(&idx) {
                    ctx.stack.push(ctx.activation_slots[&idx].clone());
                } else if let Some(v) = ctx.catch_scope_vars.get(&obj) {
                    ctx.stack.push(v.clone());
                } else {
                    let own = obj == "this" || obj.is_empty() || obj == "global"
                        || (ctx.is_static && obj == ctx.local0_name)
                        || (!ctx.class_name.is_empty() && obj == ctx.class_name);
                    let sname = if own { ctx.slot_map.get(&idx).cloned() } else { None };
                    if let Some(sn) = sname {
                        if obj == "this" && !ctx.is_static {
                            ctx.stack.push(format!("this.{sn}"));
                        } else {
                            ctx.stack.push(sn);
                        }
                    } else if (obj.is_empty() || obj == "global") && !ctx.class_name.is_empty() {
                        ctx.stack.push(ctx.class_name.clone());
                    } else {
                        ctx.stack.push(format!("{obj}.slot{idx}"));
                    }
                }
            }
            OP_SETSLOT => {
                let (idx, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let mut val = pop_or(&mut ctx.stack);
                let obj = pop_or(&mut ctx.stack);
                if obj == "__activation__" && ctx.activation_slots.contains_key(&idx) {
                    let vname = ctx.activation_slots[&idx].clone();
                    let vtype = ctx.activation_slot_types.get(&idx).cloned().unwrap_or_else(|| "*".into());
                    if !ctx.declared_activation_slots.contains(&idx) {
                        ctx.declared_activation_slots.insert(idx);
                        val = strip_redundant_cast(&vtype, &val);
                        if is_type_default(&vtype, &val) {
                            ctx.stmts.push(format!("var {vname}:{vtype};"));
                        } else {
                            ctx.stmts.push(format!("var {vname}:{vtype} = {val};"));
                        }
                    } else {
                        ctx.stmts.push(format!("{vname} = {val};"));
                    }
                } else if ctx.catch_scope_vars.contains_key(&obj) {
                    // pass
                } else {
                    let own = obj == "this" || obj.is_empty() || obj == "global"
                        || (ctx.is_static && obj == ctx.local0_name)
                        || (!ctx.class_name.is_empty() && obj == ctx.class_name);
                    let sname = if own { ctx.slot_map.get(&idx).cloned() } else { None };
                    if let Some(sn) = sname {
                        if obj == "this" && !ctx.is_static {
                            ctx.stmts.push(format!("this.{sn} = {val};"));
                        } else {
                            ctx.stmts.push(format!("{sn} = {val};"));
                        }
                    } else {
                        ctx.stmts.push(format!("{obj}.slot{idx} = {val};"));
                    }
                }
            }
            OP_GETSUPER => {
                let (mn, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let name = abc.mn_name(mn);
                let _ = pop_or(&mut ctx.stack);
                ctx.stack.push(format!("super.{name}"));
            }
            _ => {
                let (mn, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let name = abc.mn_name(mn);
                let val = pop_or(&mut ctx.stack);
                let _ = pop_or(&mut ctx.stack);
                ctx.stmts.push(format!("super.{name} = {val};"));
            }
        }
    }

    fn h_find_ops(&mut self, op: u8, ctx: &mut Run) {
        let abc = ctx.abc;
        let (mn, np) = ru30(ctx.code, ctx.p);
        ctx.p = np;
        match op {
            OP_FINDPROPSTRICT => {
                let (rt_name, _) = Self::pop_rt(ctx, mn);
                if let Some(rn) = rt_name {
                    ctx.stack.push(rn);
                } else {
                    let name = abc.mn_name(mn);
                    if ctx.is_static && !ctx.class_name.is_empty() && ctx.static_trait_names.contains(&name) {
                        ctx.stack.push(String::new());
                    } else if ctx.is_static && !ctx.class_name.is_empty() && name == ctx.class_name {
                        ctx.stack.push(String::new());
                    } else {
                        ctx.stack.push(name);
                    }
                }
            }
            OP_FINDPROPERTY => {
                let (rt_name, _) = Self::pop_rt(ctx, mn);
                if let Some(rn) = rt_name {
                    ctx.stack.push(rn);
                } else {
                    ctx.stack.push(abc.mn_name(mn));
                }
            }
            _ => ctx.stack.push(abc.mn_name(mn)),
        }
    }

    fn h_call_ops(&mut self, op: u8, ctx: &mut Run) {
        let abc = ctx.abc;
        let pos = format!("0x{op:02X}");
        match op {
            OP_CALLPROPERTY | OP_CALLPROPLEX => {
                let (mn, np) = ru30(ctx.code, ctx.p);
                let (argc, np) = ru30(ctx.code, np);
                ctx.p = np;
                let args = pop_n(&mut ctx.stack, argc as usize, &mut ctx.error_log, &pos);
                let (rt_name, _) = Self::pop_rt(ctx, mn);
                let obj = pop_or(&mut ctx.stack);
                if let Some(rn) = rt_name {
                    ctx.stack.push(format!("{obj}[{rn}]({})", args.join(", ")));
                } else {
                    let name = abc.mn_name(mn);
                    ctx.stack.push(fmt_call(&obj, &name, &args));
                }
            }
            OP_CALLPROPVOID => {
                let (mn, np) = ru30(ctx.code, ctx.p);
                let (argc, np) = ru30(ctx.code, np);
                ctx.p = np;
                let args = pop_n(&mut ctx.stack, argc as usize, &mut ctx.error_log, &pos);
                let (rt_name, _) = Self::pop_rt(ctx, mn);
                let obj = pop_or(&mut ctx.stack);
                if let Some(rn) = rt_name {
                    ctx.stmts.push(format!("{obj}[{rn}]({});", args.join(", ")));
                } else {
                    let name = abc.mn_name(mn);
                    ctx.stmts.push(format!("{};", fmt_call(&obj, &name, &args)));
                }
            }
            OP_CALLSUPER => {
                let (mn, np) = ru30(ctx.code, ctx.p);
                let (argc, np) = ru30(ctx.code, np);
                ctx.p = np;
                let name = abc.mn_name(mn);
                let args = pop_n(&mut ctx.stack, argc as usize, &mut ctx.error_log, &pos);
                let _ = pop_or(&mut ctx.stack);
                ctx.stack.push(format!("super.{name}({})", args.join(", ")));
            }
            OP_CALLSUPERVOID => {
                let (mn, np) = ru30(ctx.code, ctx.p);
                let (argc, np) = ru30(ctx.code, np);
                ctx.p = np;
                let name = abc.mn_name(mn);
                let args = pop_n(&mut ctx.stack, argc as usize, &mut ctx.error_log, &pos);
                let _ = pop_or(&mut ctx.stack);
                ctx.stmts.push(format!("super.{name}({});", args.join(", ")));
            }
            OP_CALL => {
                let (argc, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let args = pop_n(&mut ctx.stack, argc as usize, &mut ctx.error_log, &pos);
                let func = pop_or(&mut ctx.stack);
                let recv = pop_or(&mut ctx.stack);
                let a = args.join(", ");
                if func.is_empty() || func == "this" || func == "global" {
                    ctx.stack.push(format!("{recv}({a})"));
                } else if recv.is_empty() || recv == "this" || recv == "global" || recv == func {
                    ctx.stack.push(format!("{func}({a})"));
                } else {
                    ctx.stack.push(format!("{recv}.{func}({a})"));
                }
            }
            OP_CALLMETHOD => {
                let (disp, np) = ru30(ctx.code, ctx.p);
                let (argc, np) = ru30(ctx.code, np);
                ctx.p = np;
                let args = pop_n(&mut ctx.stack, argc as usize, &mut ctx.error_log, &pos);
                let recv = pop_or(&mut ctx.stack);
                ctx.stack.push(format!("{recv}.<method{disp}>({})", args.join(", ")));
            }
            _ => {
                let (mi, np) = ru30(ctx.code, ctx.p);
                let (argc, np) = ru30(ctx.code, np);
                ctx.p = np;
                let args = pop_n(&mut ctx.stack, argc as usize, &mut ctx.error_log, &pos);
                let recv = pop_or(&mut ctx.stack);
                ctx.stack.push(format!("{recv}.<static{mi}>({})", args.join(", ")));
            }
        }
    }

    fn h_construct_ops(&mut self, op: u8, ctx: &mut Run) {
        let abc = ctx.abc;
        let pos = format!("0x{op:02X}");
        match op {
            OP_CONSTRUCT => {
                let (argc, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let args = pop_n(&mut ctx.stack, argc as usize, &mut ctx.error_log, &pos);
                let obj = pop_or(&mut ctx.stack);
                if obj.contains('(') && obj.ends_with(')') && !obj.starts_with("new ") {
                    ctx.construct_tmp_counter += 1;
                    let tmp = format!("_construct_cls_{}", ctx.construct_tmp_counter);
                    ctx.stmts.push(format!("var {tmp}:Class = {obj};"));
                    ctx.stack.push(format!("new {tmp}({})", args.join(", ")));
                } else {
                    ctx.stack.push(format!("new {obj}({})", args.join(", ")));
                }
            }
            OP_CONSTRUCTSUPER => {
                let (argc, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let args = pop_n(&mut ctx.stack, argc as usize, &mut ctx.error_log, &pos);
                let _ = pop_or(&mut ctx.stack);
                ctx.stmts.push(format!("super({});", args.join(", ")));
            }
            _ => {
                let (mn, np) = ru30(ctx.code, ctx.p);
                let (argc, np) = ru30(ctx.code, np);
                ctx.p = np;
                let args = pop_n(&mut ctx.stack, argc as usize, &mut ctx.error_log, &pos);
                let (rt_name, _) = Self::pop_rt(ctx, mn);
                let obj = pop_or(&mut ctx.stack);
                if let Some(rn) = rt_name {
                    ctx.stack.push(format!("new {obj}[{rn}]({})", args.join(", ")));
                } else {
                    let name = abc.mn_name(mn);
                    if obj == "this" || obj == name {
                        ctx.stack.push(format!("new {name}({})", args.join(", ")));
                    } else {
                        ctx.stack.push(format!("new {obj}.{name}({})", args.join(", ")));
                    }
                }
            }
        }
    }

    fn h_object_ops(&mut self, op: u8, ctx: &mut Run) {
        let abc = ctx.abc;
        let pos = format!("0x{op:02X}");
        match op {
            OP_NEWOBJECT => {
                let (np2, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let items = pop_n(&mut ctx.stack, np2 as usize * 2, &mut ctx.error_log, &pos);
                let pairs: Vec<String> = (0..items.len() / 2 * 2).step_by(2).map(|i| format!("{}:{}", items[i], items[i + 1])).collect();
                if pairs.len() >= 2 {
                    ctx.stack.push(format!("{{\n{}\n}}", pairs.join(",\n")));
                } else {
                    ctx.stack.push(format!("{{{}}}", pairs.join(", ")));
                }
            }
            OP_NEWARRAY => {
                let (count, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let items = pop_n(&mut ctx.stack, count as usize, &mut ctx.error_log, &pos);
                ctx.stack.push(format!("[{}]", items.join(", ")));
            }
            OP_NEWACTIVATION => ctx.stack.push("__activation__".into()),
            OP_NEWFUNCTION => {
                let (mi, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let f = self.decompile_inline_function(mi);
                ctx.stack.push(f);
            }
            OP_NEWCLASS => {
                let (ci, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let _ = pop_or(&mut ctx.stack);
                ctx.stack.push(format!("<class#{ci}>"));
            }
            OP_NEWCATCH => {
                let (idx, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let marker = format!("__catch_scope_{idx}__");
                if (idx as usize) < ctx.body.exceptions.len() {
                    let vn = ctx.body.exceptions[idx as usize].var_name;
                    ctx.catch_scope_vars.insert(marker.clone(), if vn != 0 { abc.mn_name(vn) } else { "e".into() });
                } else {
                    ctx.catch_scope_vars.insert(marker.clone(), "e".into());
                }
                ctx.stack.push(marker);
            }
            OP_APPLYTYPE => {
                let (argc, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let args = pop_n(&mut ctx.stack, argc as usize, &mut ctx.error_log, &pos);
                let args: Vec<String> = args.into_iter().map(|a| if a == "null" { "*".to_string() } else { a }).collect();
                let base = pop_or(&mut ctx.stack);
                ctx.stack.push(format!("{base}.<{}>", args.join(", ")));
            }
            _ => {
                let (mn, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let (rt_name, _) = Self::pop_rt(ctx, mn);
                let obj = pop_or(&mut ctx.stack);
                if let Some(rn) = rt_name {
                    ctx.stack.push(format!("{obj}..{rn}"));
                } else {
                    let name = abc.mn_name(mn);
                    ctx.stack.push(format!("{obj}..{name}"));
                }
            }
        }
    }

    fn is_sc_transparent(op: u8) -> bool {
        matches!(op, OP_CONVERT_B | OP_COERCE_A | OP_COERCE_B | OP_CONVERT_O | OP_COERCE_I | OP_COERCE_U | OP_COERCE_O)
    }

    fn h_stack_ops(&mut self, op: u8, ctx: &mut Run) {
        match op {
            OP_POP => {
                if let Some(v) = ctx.stack.pop() {
                    if (v.contains('(') || v.starts_with("delete ") || v.contains("++") || v.contains("--"))
                        && !v.starts_with('"')
                    {
                        ctx.stmts.push(format!("{v};"));
                    }
                }
            }
            OP_DUP => {
                let mut sc_detected = false;
                let code = ctx.code;
                let mut look_p = ctx.p;
                while look_p < code.len() && Self::is_sc_transparent(code[look_p]) {
                    look_p += 1;
                }
                if look_p < code.len() && (code[look_p] == OP_IFFALSE || code[look_p] == OP_IFTRUE) {
                    let next_op = code[look_p];
                    let (off, p_after) = rs24(code, look_p + 1);
                    let target = (p_after as i64 + off as i64) as usize;
                    let mut pop_p = p_after;
                    while pop_p < code.len() && Self::is_sc_transparent(code[pop_p]) {
                        pop_p += 1;
                    }
                    if pop_p < code.len() && code[pop_p] == OP_POP {
                        sc_detected = true;
                        let op_str = if next_op == OP_IFFALSE { "&&" } else { "||" };
                        let left = ctx.stack.last().cloned().unwrap_or_else(|| "?".into());
                        let entries = ctx.logical_combines.entry(target).or_default();
                        if entries.last().map_or(false, |e| e.0 == op_str) {
                            let (prev_op, prev_left) = entries.last().cloned().unwrap();
                            let wl = if prev_left.starts_with('(') { prev_left.clone() } else { format!("({prev_left})") };
                            let wr = if left.starts_with('(') { left.clone() } else { format!("({left})") };
                            *entries.last_mut().unwrap() = (op_str.to_string(), format!("({wl} {prev_op} {wr})"));
                        } else {
                            entries.push((op_str.to_string(), left));
                        }
                        ctx.stack.pop();
                        ctx.p = pop_p + 1;
                    }
                }
                if !sc_detected {
                    let top = ctx.stack.last().cloned().unwrap_or_else(|| "?".into());
                    ctx.stack.push(top);
                    ctx.last_was_dup = true;
                }
            }
            _ => {
                let n = ctx.stack.len();
                if n >= 2 {
                    ctx.stack.swap(n - 1, n - 2);
                }
            }
        }
    }

    fn h_coerce_ops(&mut self, op: u8, ctx: &mut Run) {
        let abc = ctx.abc;
        match op {
            OP_CONVERT_S => {
                if let Some(t) = ctx.stack.last_mut() {
                    if !t.starts_with('"') {
                        *t = format!("String({t})");
                    }
                }
            }
            OP_CONVERT_I => {
                if let Some(t) = ctx.stack.last_mut() {
                    if !py_isdigit(t.trim_start_matches('-')) {
                        *t = format!("int({t})");
                    }
                }
            }
            OP_CONVERT_U => {
                if let Some(t) = ctx.stack.last_mut() {
                    if !py_isdigit(t.trim_start_matches('-')) {
                        *t = format!("uint({t})");
                    }
                }
            }
            OP_CONVERT_D | OP_COERCE_D => {
                if let Some(t) = ctx.stack.last_mut() {
                    if t.starts_with('"') || t.starts_with('\'') {
                        *t = format!("Number({t})");
                    }
                }
                ctx.last_was_dup = ctx.was_dup;
            }
            OP_CONVERT_B | OP_COERCE_B => {
                if let Some(t) = ctx.stack.last_mut() {
                    if py_isdigit(t.trim_start_matches('-')) {
                        *t = format!("Boolean({t})");
                    }
                }
                ctx.last_was_dup = ctx.was_dup;
            }
            OP_COERCE_S => {
                if let Some(t) = ctx.stack.last_mut() {
                    let stripped = t.trim_start_matches('-');
                    let one = match stripped.find('.') {
                        Some(i) => format!("{}{}", &stripped[..i], &stripped[i + 1..]),
                        None => stripped.to_string(),
                    };
                    if py_isdigit(&one) {
                        *t = format!("String({t})");
                    }
                }
                ctx.last_was_dup = ctx.was_dup;
            }
            OP_CONVERT_O | OP_COERCE_A | OP_COERCE_I | OP_COERCE_U | OP_COERCE_O | OP_CHECKFILTER => {
                ctx.last_was_dup = ctx.was_dup;
            }
            OP_COERCE => {
                let (_, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                ctx.last_was_dup = ctx.was_dup;
            }
            OP_ASTYPE => {
                let (mn, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let name = abc.mn_name(mn);
                if let Some(t) = ctx.stack.last_mut() {
                    *t = format!("({t} as {name})");
                }
            }
            OP_ASTYPELATE => {
                let t = pop_or(&mut ctx.stack);
                let v = pop_or(&mut ctx.stack);
                ctx.stack.push(format!("({v} as {t})"));
            }
            OP_ISTYPE => {
                let (mn, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let name = abc.mn_name(mn);
                if let Some(t) = ctx.stack.last_mut() {
                    *t = format!("({t} is {name})");
                }
            }
            OP_ISTYPELATE => {
                let t = pop_or(&mut ctx.stack);
                let v = pop_or(&mut ctx.stack);
                ctx.stack.push(format!("({v} is {t})"));
            }
            OP_INSTANCEOF => {
                let t = pop_or(&mut ctx.stack);
                let v = pop_or(&mut ctx.stack);
                ctx.stack.push(format!("({v} instanceof {t})"));
            }
            OP_TYPEOF => {
                if let Some(t) = ctx.stack.last_mut() {
                    *t = format!("typeof({t})");
                }
            }
            _ => {}
        }
    }

    fn h_arithmetic_ops(&mut self, op: u8, ctx: &mut Run) {
        let stack = &mut ctx.stack;
        match op {
            OP_ADD | OP_ADD_I => binop(stack, "+"),
            OP_SUBTRACT | OP_SUBTRACT_I => binop(stack, "-"),
            OP_MULTIPLY | OP_MULTIPLY_I => binop(stack, "*"),
            OP_DIVIDE => binop(stack, "/"),
            OP_MODULO => binop(stack, "%"),
            OP_LSHIFT => binop(stack, "<<"),
            OP_RSHIFT => binop(stack, ">>"),
            OP_URSHIFT => binop(stack, ">>>"),
            OP_BITAND => bitwise_binop(stack, "&"),
            OP_BITOR => bitwise_binop(stack, "|"),
            OP_BITXOR => bitwise_binop(stack, "^"),
            OP_NEGATE | OP_NEGATE_I => {
                if let Some(t) = stack.last_mut() {
                    if t.starts_with('(') {
                        *t = format!("-{t}");
                    } else {
                        *t = format!("-({t})");
                    }
                }
            }
            OP_NOT => {
                if let Some(t) = stack.last_mut() {
                    let val = t.clone();
                    if let Some(c) = RE_EQ_MATCH.captures(&val) {
                        let negop = if &c[2] == "===" { "!==" } else { "!=" };
                        *t = format!("({} {negop} {})", &c[1], &c[3]);
                    } else if val.starts_with('(') || val.contains(").") {
                        *t = format!("!{val}");
                    } else {
                        *t = format!("!({val})");
                    }
                }
            }
            OP_BITNOT => {
                if let Some(t) = stack.last_mut() {
                    *t = format!("(~({}))", to_hex_if_int(t));
                }
            }
            OP_INCREMENT | OP_INCREMENT_I => {
                if let Some(t) = stack.last_mut() {
                    *t = format!("({t} + 1)");
                }
            }
            _ => {
                if let Some(t) = stack.last_mut() {
                    *t = format!("({t} - 1)");
                }
            }
        }
    }

    fn h_comparison_ops(&mut self, op: u8, ctx: &mut Run) {
        let stack = &mut ctx.stack;
        match op {
            OP_EQUALS => binop(stack, "=="),
            OP_STRICTEQUALS => binop(stack, "==="),
            OP_LESSTHAN => binop(stack, "<"),
            OP_LESSEQUALS => binop(stack, "<="),
            OP_GREATERTHAN => binop(stack, ">"),
            OP_GREATEREQUALS => binop(stack, ">="),
            _ => {
                let name = pop_or(stack);
                let obj = pop_or(stack);
                stack.push(format!("({obj} in {name})"));
            }
        }
    }

    fn cmp_symbol(op: u8) -> &'static str {
        match op {
            OP_IFEQ => "==",
            OP_IFNE => "!=",
            OP_IFLT => "<",
            OP_IFLE => "<=",
            OP_IFGT => ">",
            OP_IFGE => ">=",
            OP_IFSTRICTEQ => "===",
            OP_IFSTRICTNE => "!==",
            OP_IFNLT => "!<",
            OP_IFNLE => "!<=",
            OP_IFNGT => "!>",
            _ => "!>=",
        }
    }

    fn try_ternary_ctx(&self, ctx: &Run, true_start: usize, false_label: usize) -> Option<Ternary> {
        self.try_ternary(
            ctx.code,
            true_start,
            false_label,
            ctx.stack.clone(),
            &ctx.local_names,
            &ctx.slot_map,
            &ctx.local0_name,
            ctx.is_static,
            ctx.class_idx,
        )
    }

    fn h_branch_ops(&mut self, op: u8, ctx: &mut Run) {
        match op {
            OP_RETURNVOID => ctx.stmts.push("return;".into()),
            OP_RETURNVALUE => {
                let mut val = pop_or(&mut ctx.stack);
                if has_outer_parens(&val) {
                    val = val[1..val.len() - 1].to_string();
                }
                ctx.stmts.push(format!("return {val};"));
            }
            OP_JUMP => {
                let (off, np) = rs24(ctx.code, ctx.p);
                ctx.p = np;
                let target = np as i64 + off as i64;
                ctx.stmts.push(format!("goto __label_{target};"));
            }
            OP_IFTRUE => {
                let (off, np) = rs24(ctx.code, ctx.p);
                ctx.p = np;
                let target = (np as i64 + off as i64) as usize;
                let cond = pop_or(&mut ctx.stack);
                if let Some((fallthrough_val, target_val, end_pos)) = self.try_ternary_ctx(ctx, ctx.p, target) {
                    let c = if has_outer_parens(&cond) { cond.clone() } else { format!("({cond})") };
                    let tv = if needs_ternary_wrap(&target_val) { format!("({target_val})") } else { target_val };
                    let fv = if needs_ternary_wrap(&fallthrough_val) { format!("({fallthrough_val})") } else { fallthrough_val };
                    ctx.stack.push(format!("({c} ? {tv} : {fv})"));
                    ctx.p = end_pos;
                } else {
                    ctx.stmts.push(format!("if ({cond}) goto __label_{target};"));
                }
            }
            OP_IFFALSE => {
                let (off, np) = rs24(ctx.code, ctx.p);
                ctx.p = np;
                let target = (np as i64 + off as i64) as usize;
                let cond = pop_or(&mut ctx.stack);
                if let Some((true_val, false_val, end_pos)) = self.try_ternary_ctx(ctx, ctx.p, target) {
                    let c = if has_outer_parens(&cond) { cond.clone() } else { format!("({cond})") };
                    let tv = if needs_ternary_wrap(&true_val) { format!("({true_val})") } else { true_val };
                    let fv = if needs_ternary_wrap(&false_val) { format!("({false_val})") } else { false_val };
                    ctx.stack.push(format!("({c} ? {tv} : {fv})"));
                    ctx.p = end_pos;
                } else {
                    ctx.stmts.push(format!("if (!({cond})) goto __label_{target};"));
                }
            }
            OP_IFEQ | OP_IFNE | OP_IFLT | OP_IFLE | OP_IFGT | OP_IFGE | OP_IFSTRICTEQ | OP_IFSTRICTNE | OP_IFNLT
            | OP_IFNLE | OP_IFNGT | OP_IFNGE => {
                let (off, np) = rs24(ctx.code, ctx.p);
                ctx.p = np;
                let target_i = np as i64 + off as i64;
                let b = pop_or(&mut ctx.stack);
                let a = pop_or(&mut ctx.stack);
                let not_cond = match op {
                    OP_IFNGT => Some(">"),
                    OP_IFNLT => Some("<"),
                    OP_IFNLE => Some("<="),
                    OP_IFNGE => Some(">="),
                    _ => None,
                };
                let pos_neg = match op {
                    OP_IFEQ => Some("!="),
                    OP_IFNE => Some("=="),
                    OP_IFLT => Some(">="),
                    OP_IFLE => Some(">"),
                    OP_IFGT => Some("<="),
                    OP_IFGE => Some("<"),
                    OP_IFSTRICTEQ => Some("!=="),
                    OP_IFSTRICTNE => Some("==="),
                    _ => None,
                };
                let sym = not_cond.or(pos_neg);
                if let Some(s) = sym {
                    if target_i > ctx.p as i64 {
                        let cond_str = format!("{a} {s} {b}");
                        if let Some((true_val, false_val, end_pos)) = self.try_ternary_ctx(ctx, ctx.p, target_i as usize) {
                            let c = format!("({cond_str})");
                            let tv = if needs_ternary_wrap(&true_val) { format!("({true_val})") } else { true_val };
                            let fv = if needs_ternary_wrap(&false_val) { format!("({false_val})") } else { false_val };
                            ctx.stack.push(format!("({c} ? {tv} : {fv})"));
                            ctx.p = end_pos;
                            return;
                        }
                    }
                }
                ctx.stmts.push(format!("if ({a} {} {b}) goto __label_{target_i};", Self::cmp_symbol(op)));
            }
            _ => {
                let base = ctx.p as i64 - 1;
                let (default_off, np) = rs24(ctx.code, ctx.p);
                let (case_count, mut np) = ru30(ctx.code, np);
                let mut offsets = Vec::new();
                for _ in 0..=case_count {
                    let (o, q) = rs24(ctx.code, np);
                    np = q;
                    offsets.push(o);
                }
                ctx.p = np;
                let val = pop_or(&mut ctx.stack);
                ctx.stmts.push(format!("switch ({val}) {{"));
                for (i, o) in offsets.iter().enumerate() {
                    ctx.stmts.push(format!("  case {i}: goto __label_{};", base + *o as i64));
                }
                ctx.stmts.push(format!("  default: goto __label_{};", base + default_off as i64));
                ctx.stmts.push("}".into());
            }
        }
    }

    fn h_iteration_ops(&mut self, op: u8, ctx: &mut Run) {
        match op {
            OP_NEXTNAME | OP_NEXTVALUE | OP_HASNEXT => {
                let idx = pop_or(&mut ctx.stack);
                let obj = pop_or(&mut ctx.stack);
                let f = match op {
                    OP_NEXTNAME => "nextname",
                    OP_NEXTVALUE => "nextvalue",
                    _ => "hasnext",
                };
                ctx.stack.push(format!("{f}({obj}, {idx})"));
            }
            _ => {
                let (obj_reg, np) = ru30(ctx.code, ctx.p);
                let (idx_reg, np) = ru30(ctx.code, np);
                ctx.p = np;
                let o = Self::local_name(ctx, obj_reg);
                let i = Self::local_name(ctx, idx_reg);
                ctx.stack.push(format!("hasnext2({o}, {i})"));
            }
        }
    }

    fn h_misc_ops(&mut self, op: u8, ctx: &mut Run) {
        match op {
            OP_THROW => {
                let val = pop_or(&mut ctx.stack);
                ctx.stmts.push(format!("throw {val};"));
            }
            OP_KILL => {
                let (idx, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
                let pc = if ctx.method_idx >= 0 && (ctx.method_idx as usize) < ctx.abc.methods.len() {
                    ctx.abc.methods[ctx.method_idx as usize].param_count
                } else {
                    0
                };
                if !ctx.local_names.contains_key(&idx) || idx > pc {
                    ctx.local_names.remove(&idx);
                }
            }
            OP_DXNS => {
                let (_, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
            }
            _ => {
                ctx.stack.pop();
            }
        }
    }

    fn h_memory_ops(&mut self, op: u8, ctx: &mut Run) {
        match op {
            OP_LI8 | OP_LI16 | OP_LI32 | OP_LF32 | OP_LF64 => {
                let addr = pop_or(&mut ctx.stack);
                let n = match op {
                    OP_LI8 => "li8",
                    OP_LI16 => "li16",
                    OP_LI32 => "li32",
                    OP_LF32 => "lf32",
                    _ => "lf64",
                };
                ctx.stack.push(format!("{n}({addr})"));
            }
            OP_SI8 | OP_SI16 | OP_SI32 | OP_SF32 | OP_SF64 => {
                let val = pop_or(&mut ctx.stack);
                let addr = pop_or(&mut ctx.stack);
                let n = match op {
                    OP_SI8 => "si8",
                    OP_SI16 => "si16",
                    OP_SI32 => "si32",
                    OP_SF32 => "sf32",
                    _ => "sf64",
                };
                ctx.stmts.push(format!("{n}({val}, {addr});"));
            }
            _ => {}
        }
    }

    fn h_debug_ops(&mut self, op: u8, ctx: &mut Run) {
        match op {
            OP_DEBUG => {
                let (debug_type, np) = ru8(ctx.code, ctx.p);
                let (name_idx, np) = ru30(ctx.code, np);
                let (reg, np) = ru8(ctx.code, np);
                let (_, np) = ru30(ctx.code, np);
                ctx.p = np;
                if debug_type == 1 && (name_idx as usize) < ctx.abc.strings.len() {
                    let var_name = ctx.abc.strings[name_idx as usize].clone();
                    if !var_name.is_empty() && reg as u32 > ctx.param_count {
                        let existing = ctx.local_names.get(&(reg as u32));
                        if existing.map_or(true, |e| e.starts_with("_local_")) {
                            ctx.local_names.insert(reg as u32, var_name);
                        }
                    }
                }
            }
            _ => {
                let (_, np) = ru30(ctx.code, ctx.p);
                ctx.p = np;
            }
        }
    }

    fn h_global_slot_ops(&mut self, op: u8, ctx: &mut Run) {
        let (idx, np) = ru30(ctx.code, ctx.p);
        ctx.p = np;
        match op {
            OP_GETGLOBALSLOT => ctx.stack.push(format!("globalSlot{idx}")),
            OP_SETGLOBALSLOT => {
                let val = pop_or(&mut ctx.stack);
                ctx.stmts.push(format!("globalSlot{idx} = {val};"));
            }
            _ => ctx.stack.push(ctx.abc.mn_name(idx)),
        }
    }

    // ------------------------------------------------ inline functions

    fn params_string(&self, m: &MethodInfo) -> Vec<String> {
        let abc = self.abc;
        let mut params = Vec::new();
        let num_required = m.param_count as i64 - m.optional_values.len() as i64;
        for i in 0..m.param_count as i64 {
            let mut pname = String::new();
            if (i as usize) < m.param_names.len() {
                let pn = m.param_names[i as usize] as usize;
                if pn < abc.strings.len() {
                    pname = abc.strings[pn].clone();
                }
            }
            if pname.is_empty() {
                pname = format!("_arg_{}", i + 1);
            }
            let ptype = if (i as usize) < m.param_types.len() && m.param_types[i as usize] != 0 {
                abc.type_name(m.param_types[i as usize])
            } else {
                "*".into()
            };
            let mut s = format!("{pname}:{ptype}");
            if i >= num_required {
                let opt_idx = (i - num_required) as usize;
                if opt_idx < m.optional_values.len() {
                    let (vkind, vindex) = m.optional_values[opt_idx];
                    s.push_str(&format!("={}", abc.default_value_str(vkind, vindex)));
                }
            }
            params.push(s);
        }
        if m.flags & METHOD_NEED_REST != 0 {
            params.push("...rest".into());
        }
        params
    }

    fn decompile_inline_function(&mut self, mi: u32) -> String {
        let abc = self.abc;
        if mi as usize >= abc.methods.len() {
            return format!("function(/*method#{mi}*/)");
        }
        let m = &abc.methods[mi as usize];
        let params = self.params_string(m);
        let rtype = if m.return_type != 0 { abc.type_name(m.return_type) } else { "*".into() };
        let ret_str = if rtype.is_empty() { String::new() } else { format!(":{rtype}") };
        let sig = format!("function ({}){ret_str}", params.join(", "));
        let Some(body) = abc.method_bodies.get(&mi) else { return sig };
        let res = catch(|| {
            let mut stmts = self.run(&body.code, body, mi as i64, -1, false, "");
            stmts = Self::fold_increments(stmts);
            stmts = Self::fold_compound_assign(stmts);
            stmts = Self::fold_inline_assignment(stmts);
            stmts = Self::fold_short_circuit_conditions(stmts);
            if !body.exceptions.is_empty() {
                stmts = self.fold_try_catch(stmts, body, &body.code);
            }
            stmts = Self::fold_switch(stmts);
            stmts = self.structure_flow(stmts);
            stmts = Self::fold_goto_dowhile(stmts);
            stmts = Self::fold_while_to_for(stmts);
            stmts = Self::fold_for_each_in(stmts);
            stmts = Self::fold_if_else_return_chains(stmts);
            stmts = Self::fold_regexp_literals(stmts);
            stmts = Self::fold_redundant_casts(stmts);
            Self::remove_stray_breaks(stmts)
        });
        let Ok(stmts) = res else { return sig };
        let mut lines = vec![sig, "{".to_string()];
        for s in &stmts {
            if !s.is_empty() {
                for sub in s.split('\n') {
                    lines.push(sub.trim_start_matches(' ').to_string());
                }
            }
        }
        lines.push("}".into());
        lines.join("\n")
    }

    // ------------------------------------------------------------ passes

    fn fold_increments(stmts: Vec<String>) -> Vec<String> {
        let mut result = Vec::new();
        let mut i = 0;
        while i < stmts.len() {
            if i + 3 < stmts.len() {
                if let (Some(m0), Some(m1)) = (RE_TEMP_ASSIGN.captures(&stmts[i]), RE_TEMP_ASSIGN.captures(&stmts[i + 1])) {
                    let var_obj = m0[1].to_string();
                    let arr_expr = m0[2].to_string();
                    let var_idx = m1[1].to_string();
                    let idx_expr = m1[2].to_string();
                    if let Some(m2) = RE_INC_DEC_EXPR.captures(&stmts[i + 2]) {
                        let var_val = m2[1].to_string();
                        let inc_expr = m2[2].to_string();
                        let op = m2[3].to_string();
                        let want = format!("{var_obj}[{var_idx}] = {var_val};");
                        if stmts[i + 3] == want {
                            let expected = format!("{arr_expr}[{var_idx}]");
                            if inc_expr == expected {
                                let o = if op == "+" { "++" } else { "--" };
                                result.push(format!("{arr_expr}[{idx_expr}]{o};"));
                                i += 4;
                                continue;
                            }
                        }
                    }
                }
            }
            if i + 2 < stmts.len() {
                if let Some(m0) = RE_TEMP_ASSIGN.captures(&stmts[i]) {
                    let var1 = m0[1].to_string();
                    let obj = m0[2].to_string();
                    if let Some(m1) = RE_INC_DEC_EXPR.captures(&stmts[i + 1]) {
                        let var2 = m1[1].to_string();
                        let expr = m1[2].to_string();
                        let op = m1[3].to_string();
                        let o = if op == "+" { "++" } else { "--" };
                        let r1 = rx(&format!(r"^{}\.(\w+) = {};$", esc(&var1), esc(&var2)));
                        if let Some(m2) = r1.captures(&stmts[i + 2]) {
                            let prop = m2[1].to_string();
                            if expr == format!("{obj}.{prop}") {
                                result.push(format!("{obj}.{prop}{o};"));
                                i += 3;
                                continue;
                            }
                        }
                        let r2 = rx(&format!(r"^{}\[(.+?)\] = {};$", esc(&var1), esc(&var2)));
                        if let Some(m2b) = r2.captures(&stmts[i + 2]) {
                            let idx_expr = m2b[1].to_string();
                            if expr == format!("{obj}[{idx_expr}]") {
                                result.push(format!("{obj}[{idx_expr}]{o};"));
                                i += 3;
                                continue;
                            }
                        }
                    }
                }
            }
            // Pattern 3: X = (X + 1);  X = int((X + 1));  X = uint((X + 1));
            if let Some((target, plus)) = match_inc_dec_stmt(&stmts[i]) {
                result.push(format!("{target}{};", if plus { "++" } else { "--" }));
                i += 1;
                continue;
            }
            result.push(stmts[i].clone());
            i += 1;
        }
        result
    }

    fn fold_compound_assign(stmts: Vec<String>) -> Vec<String> {
        const OPS: [&str; 11] = ["+", "-", "*", "/", "%", "&", "|", "^", "<<", ">>>", ">>"];
        let mut result = Vec::new();
        for s in stmts {
            let mut folded = false;
            for op in OPS {
                if let Some((target, val)) = match_compound(&s, op, false) {
                    if (op == "+" || op == "-") && val.trim() == "1" {
                        break;
                    }
                    result.push(format!("{target} {op}= {val};"));
                    folded = true;
                    break;
                }
                if let Some((target, val)) = match_compound(&s, op, true) {
                    if (op == "+" || op == "-") && val.trim() == "1" {
                        break;
                    }
                    result.push(format!("{target} {op}= {val};"));
                    folded = true;
                    break;
                }
            }
            if !folded {
                result.push(s);
            }
        }
        result
    }

    fn fold_regexp_literals(stmts: Vec<String>) -> Vec<String> {
        stmts
            .into_iter()
            .map(|s| {
                RE_REGEXP_NEW
                    .replace_all(&s, |m: &regex::Captures| {
                        let pattern = &m[1];
                        let flags = m.get(2).map_or("", |x| x.as_str());
                        let p1 = pattern.replace("\\\\", "\0ESCAPE\0");
                        let p2 = p1.replace('\\', "");
                        let p3 = p2.replace("\0ESCAPE\0", "\\");
                        if p3.contains('/') {
                            m[0].to_string()
                        } else {
                            format!("/{p3}/{flags}")
                        }
                    })
                    .into_owned()
            })
            .collect()
    }

    fn fold_redundant_casts(stmts: Vec<String>) -> Vec<String> {
        let mut var_types: HashMap<String, String> = HashMap::new();
        for s in &stmts {
            for m in RE_VAR_DECL_INT.captures_iter(s) {
                var_types.insert(m[1].to_string(), m[2].to_string());
            }
        }
        let mut result = Vec::new();
        for s in stmts {
            let mut s = strip_double_cast(&s);
            if let Some(m) = RE_ASSIGN_CAST.captures(&s) {
                let lhs = m[1].to_string();
                let var_name = m[2].to_string();
                let cast_fn = m[3].to_string();
                let inner = m[4].to_string();
                let mut depth = 0i32;
                let mut balanced = true;
                for ch in inner.chars() {
                    if ch == '(' {
                        depth += 1;
                    } else if ch == ')' {
                        depth -= 1;
                        if depth < 0 {
                            balanced = false;
                            break;
                        }
                    }
                }
                if !balanced || depth != 0 {
                    result.push(s);
                    continue;
                }
                if var_types.get(&var_name).map_or(false, |t| *t == cast_fn) {
                    s = format!("{lhs}{inner};");
                }
            }
            result.push(s);
        }
        result
    }

    fn remove_stray_breaks(stmts: Vec<String>) -> Vec<String> {
        let mut pending = false;
        let mut scope_stack: Vec<bool> = Vec::new();
        let mut depth: i64 = 0;
        let mut result = Vec::new();
        for s in stmts {
            let stripped = s.trim();
            if RE_BREAK_KW.is_match(stripped) {
                pending = true;
            }
            let b = stripped.as_bytes();
            let mut in_string = false;
            let mut sc = 0u8;
            let mut i = 0;
            while i < b.len() {
                let ch = b[i];
                if in_string {
                    if ch == b'\\' {
                        i += 1;
                    } else if ch == sc {
                        in_string = false;
                    }
                } else if ch == b'"' || ch == b'\'' {
                    in_string = true;
                    sc = ch;
                } else if ch == b'{' {
                    scope_stack.push(pending);
                    if pending {
                        depth += 1;
                    }
                    pending = false;
                } else if ch == b'}' {
                    if let Some(was) = scope_stack.pop() {
                        if was {
                            depth -= 1;
                        }
                    }
                    pending = false;
                }
                i += 1;
            }
            if stripped == "break;" && depth <= 0 {
                continue;
            }
            result.push(s);
        }
        result
    }

    fn fold_short_circuit_conditions(stmts: Vec<String>) -> Vec<String> {
        let mut result = Vec::new();
        let mut i = 0;
        while i < stmts.len() {
            let s = stmts[i].trim().to_string();
            if let Some(m) = RE_IF_GOTO.captures(&s) {
                let target = m[2].to_string();
                let mut conds = vec![m[1].to_string()];
                let mut j = i + 1;
                let rj = rx(&format!(r"^if \((.+)\) goto {};$", esc(&target)));
                while j < stmts.len() {
                    let sj = stmts[j].trim();
                    if let Some(mj) = rj.captures(sj) {
                        conds.push(mj[1].to_string());
                        j += 1;
                    } else {
                        break;
                    }
                }
                if conds.len() > 1 {
                    let mut parts = Vec::new();
                    for c in &conds {
                        let neg = Self::negate_cond(c);
                        if neg.contains(' ') && !has_outer_parens(&neg) && !neg.contains("&&") && !neg.contains("||") {
                            parts.push(format!("({neg})"));
                        } else {
                            parts.push(neg);
                        }
                    }
                    result.push(format!("if (!({})) goto {target};", parts.join(" && ")));
                    i = j;
                } else {
                    result.push(stmts[i].clone());
                    i += 1;
                }
            } else {
                result.push(stmts[i].clone());
                i += 1;
            }
        }
        result
    }

    fn fold_inline_assignment(stmts: Vec<String>) -> Vec<String> {
        let mut result = Vec::new();
        let mut i = 0;
        while i < stmts.len() {
            if i + 2 < stmts.len() {
                if let Some(m0) = RE_VAR_LOCAL.captures(&stmts[i]) {
                    let tmp = m0[1].to_string();
                    let expr = m0[2].to_string();
                    let r1 = rx(&format!(r"^(.+?) = {};$", esc(&tmp)));
                    if let Some(m1) = r1.captures(&stmts[i + 1]) {
                        let target = m1[1].to_string();
                        if stmts[i + 2] == format!("return {tmp};") {
                            result.push(format!("return ({target} = {expr});"));
                            i += 3;
                            continue;
                        }
                    }
                }
            }
            result.push(stmts[i].clone());
            i += 1;
        }
        result
    }
}

/// `_RE_INC_DEC`: `X = (X + 1);` / `X = int((X - 1));` ... returning `(X, is_plus)`.
/// Python's pattern uses a backreference, which the `regex` crate lacks, so the
/// left-hand side is enumerated in the same preference order the engine tries.
fn match_inc_dec_stmt(s: &str) -> Option<(String, bool)> {
    for e in lhs_candidates(s) {
        let target = &s[..e];
        let Some(rest) = s[e..].strip_prefix(" = ") else { continue };
        // (?:(?:int|uint)\()?  \(  \1  " "  ([+-])  " 1"  \)  \)?  ;
        let opts: [&str; 3] = ["int(", "uint(", ""];
        for cast in opts {
            let Some(r) = rest.strip_prefix(cast) else { continue };
            let Some(r) = r.strip_prefix('(') else { continue };
            let Some(r) = r.strip_prefix(target) else { continue };
            let Some(r) = r.strip_prefix(' ') else { continue };
            let mut ch = r.chars();
            let Some(opc) = ch.next() else { continue };
            if opc != '+' && opc != '-' {
                continue;
            }
            let r = ch.as_str();
            let Some(r) = r.strip_prefix(" 1)") else { continue };
            for tail in [")", ""] {
                if let Some(r2) = r.strip_prefix(tail) {
                    if r2 == ";" {
                        return Some((target.to_string(), opc == '+'));
                    }
                }
            }
        }
    }
    None
}

/// Compound assignment `X = (X OP val);` or `X = int|uint((X OP val));`.
fn match_compound(s: &str, op: &str, cast: bool) -> Option<(String, String)> {
    for e in lhs_candidates(s) {
        let target = &s[..e];
        let Some(rest) = s[e..].strip_prefix(" = ") else { continue };
        let rest = if cast {
            let r = rest.strip_prefix("int(").or_else(|| rest.strip_prefix("uint("));
            match r {
                Some(r) => r,
                None => continue,
            }
        } else {
            rest
        };
        let Some(r) = rest.strip_prefix('(') else { continue };
        let Some(r) = r.strip_prefix(target) else { continue };
        let Some(r) = r.strip_prefix(&format!(" {op} ")) else { continue };
        let tail = if cast { ");" } else { ");" };
        let want = if cast { ")" } else { "" };
        // value is greedy `(.+)` followed by `\)` [`\)`] `;`
        let suffix = format!("{want}{tail}");
        if let Some(val) = r.strip_suffix(suffix.as_str()) {
            if !val.is_empty() && !val.contains('\n') {
                return Some((target.to_string(), val.to_string()));
            }
        }
    }
    None
}

/// End offsets of `\w[\w.]*(?:\[.+?\])?` prefixes of `s`, in the order a
/// backtracking engine would try them (longest word first; bracket group
/// present before absent; lazy bracket shortest first).
fn lhs_candidates(s: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut it = s.char_indices();
    let Some((_, c0)) = it.next() else { return out };
    if !(c0.is_alphanumeric() || c0 == '_') {
        return out;
    }
    let mut ends = vec![c0.len_utf8()];
    for (i, c) in it {
        if c.is_alphanumeric() || c == '_' || c == '.' {
            ends.push(i + c.len_utf8());
        } else {
            break;
        }
    }
    for &w in ends.iter().rev() {
        if s[w..].starts_with('[') {
            // `.+?` needs at least one char (not newline), then `]`
            let inner = &s[w + 1..];
            let mut pos = w + 1;
            let mut first = true;
            for (k, c) in inner.char_indices() {
                if c == '\n' {
                    break;
                }
                if !first && c == ']' {
                    out.push(w + 1 + k + 1);
                }
                first = false;
                pos = w + 1 + k + c.len_utf8();
            }
            let _ = pos;
        }
        out.push(w);
    }
    out
}

fn strip_double_cast(s: &str) -> String {
    let mut s = s.to_string();
    loop {
        let b = s.as_bytes();
        let mut found: Option<(usize, usize)> = None;
        let mut i = 0;
        while i < b.len() {
            let prev_word = i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_' || b[i - 1] >= 0x80);
            if !prev_word {
                if b[i..].starts_with(b"int(int(") {
                    found = Some((i, 3));
                    break;
                }
                if b[i..].starts_with(b"uint(uint(") {
                    found = Some((i, 4));
                    break;
                }
            }
            i += 1;
        }
        let Some((outer_start, fnlen)) = found else { break };
        let open_pos = outer_start + fnlen;
        let mut depth = 0i32;
        let mut close_pos: Option<usize> = None;
        for (k, &c) in b.iter().enumerate().skip(open_pos) {
            if c == b'(' {
                depth += 1;
            } else if c == b')' {
                depth -= 1;
                if depth == 0 {
                    close_pos = Some(k);
                    break;
                }
            }
        }
        let Some(cp) = close_pos else { break };
        let inner = s[open_pos + 1..cp].to_string();
        s = format!("{}{}{}", &s[..outer_start], inner, &s[cp + 1..]);
    }
    s
}

// ------------------------------------------------------------ try / catch

#[derive(Clone, Debug)]
struct ExcInfo {
    from: u32,
    to: u32,
    target: u32,
    merge_si: i64,
    var: String,
    typ: String,
    target_si: i64,
}

impl<'a> MethodDecompiler<'a> {
    fn fold_try_catch(&mut self, stmts: Vec<String>, body: &MethodBody, code: &[u8]) -> Vec<String> {
        let abc = self.abc;
        if body.exceptions.is_empty() {
            return stmts;
        }
        let mut label_pos: HashMap<i64, i64> = HashMap::new();
        for (si, s) in stmts.iter().enumerate() {
            if let Some(m) = RE_LABEL_NUM_COLON.captures(s.trim()) {
                if let Ok(n) = m[1].parse::<i64>() {
                    label_pos.insert(n, si as i64);
                }
            }
        }
        let lp = |k: i64| -> i64 { *label_pos.get(&k).unwrap_or(&-1) };
        let mut exc_info: Vec<ExcInfo> = Vec::new();
        for ex in &body.exceptions {
            let var_name = if ex.var_name != 0 { abc.mn_name(ex.var_name) } else { "e".into() };
            let exc_type = if ex.exc_type != 0 { abc.type_name(ex.exc_type) } else { String::new() };
            let mut merge_offset: i64 = -1;
            if (ex.to_pos as usize) < code.len() && code[ex.to_pos as usize] == OP_JUMP {
                let (off, _) = rs24(code, ex.to_pos as usize + 1);
                merge_offset = ex.to_pos as i64 + 4 + off as i64;
            }
            exc_info.push(ExcInfo {
                from: ex.from_pos,
                to: ex.to_pos,
                target: ex.target,
                merge_si: lp(merge_offset),
                var: var_name,
                typ: exc_type,
                target_si: lp(ex.target as i64),
            });
        }
        // ordered groups
        let mut try_groups: Vec<((u32, u32), Vec<ExcInfo>)> = Vec::new();
        for ei in &exc_info {
            let key = (ei.from, ei.to);
            if let Some(g) = try_groups.iter_mut().find(|g| g.0 == key) {
                g.1.push(ei.clone());
            } else {
                try_groups.push((key, vec![ei.clone()]));
            }
        }
        let mut finally_map: Vec<((u32, u32), ExcInfo)> = Vec::new();
        let mut regular_groups: Vec<((u32, u32), Vec<ExcInfo>)> = Vec::new();
        let mut typed_ranges: Vec<(u32, u32)> = Vec::new();
        for (key, group) in &try_groups {
            if group.iter().any(|ei| !ei.typ.is_empty()) {
                typed_ranges.push(*key);
            }
        }
        for (key, group) in &try_groups {
            let typed_in_group: Vec<ExcInfo> = group.iter().filter(|e| !e.typ.is_empty()).cloned().collect();
            let catchall: Vec<ExcInfo> = group.iter().filter(|e| e.typ.is_empty()).cloned().collect();
            if !typed_in_group.is_empty() {
                regular_groups.push((*key, typed_in_group.clone()));
            }
            for ei in catchall {
                let mut is_finally = false;
                if !typed_in_group.is_empty() {
                    'o: for (other_key, other_group) in &try_groups {
                        if other_key == key {
                            continue;
                        }
                        for other_ei in other_group {
                            if other_ei.typ.is_empty() && other_key.0 <= key.0 && other_key.1 >= key.1 {
                                is_finally = true;
                                break 'o;
                            }
                        }
                    }
                } else {
                    for tkey in &typed_ranges {
                        if key.0 <= tkey.0 && key.1 >= tkey.1 && key != tkey {
                            is_finally = true;
                            break;
                        }
                    }
                }
                if is_finally {
                    if let Some(e) = finally_map.iter_mut().find(|e| e.0 == *key) {
                        e.1 = ei;
                    } else {
                        finally_map.push((*key, ei));
                    }
                } else if let Some(g) = regular_groups.iter_mut().find(|g| g.0 == *key) {
                    g.1.push(ei);
                } else {
                    regular_groups.push((*key, vec![ei]));
                }
            }
        }

        let n = stmts.len() as i64;
        let mut replacements: Vec<(i64, i64, Vec<String>)> = Vec::new();
        for (key, catches) in &regular_groups {
            let (from_pos, to_pos) = *key;
            let from_si = lp(from_pos as i64);
            let to_si = lp(to_pos as i64);
            if from_si < 0 || to_si < 0 {
                continue;
            }
            let mut try_body = Vec::new();
            let mut k = from_si + 1;
            while k < to_si {
                try_body.push(stmts[k as usize].clone());
                k += 1;
            }
            let mut catch_blocks: Vec<(String, Vec<String>)> = Vec::new();
            for ei in catches {
                let target_si = ei.target_si;
                if target_si < 0 {
                    continue;
                }
                let catch_clause = if ei.typ.is_empty() { format!("catch({})", ei.var) } else { format!("catch({}:{})", ei.var, ei.typ) };
                let mut next_targets: Vec<i64> = catches.iter().map(|e| e.target_si).filter(|&t| t > target_si).collect();
                for (fkey, fei) in &finally_map {
                    if fkey.0 == from_pos && fei.target_si > target_si {
                        next_targets.push(fei.target_si);
                    }
                }
                next_targets.sort();
                let catch_end_si;
                if let Some(&t) = next_targets.first() {
                    catch_end_si = t;
                } else if ei.merge_si >= 0 {
                    catch_end_si = ei.merge_si;
                } else {
                    let mut ce = n;
                    let mut km = target_si + 1;
                    while km < n {
                        let ms = stmts[km as usize].trim();
                        if RE_GOTO_LABEL_BARE.is_match(ms) {
                            ce = km + 1;
                            break;
                        }
                        km += 1;
                    }
                    catch_end_si = ce;
                }
                let mut catch_body = Vec::new();
                let mut k = target_si + 1;
                while k < catch_end_si {
                    let s = stmts[k as usize].trim();
                    if RE_GOTO_LABEL_BARE.is_match(s) {
                        k += 1;
                        continue;
                    }
                    if s.ends_with(':') {
                        k += 1;
                        continue;
                    }
                    catch_body.push(stmts[k as usize].clone());
                    k += 1;
                }
                catch_blocks.push((catch_clause, catch_body));
            }
            let all_catch_targets: Vec<i64> = catches.iter().map(|e| e.target_si).filter(|&t| t >= 0).collect();
            let max_catch_target = all_catch_targets.iter().copied().max().unwrap_or(to_si);
            let merge_si = catches.first().map_or(-1, |e| e.merge_si);
            let mut region_end_si: i64 = -1;
            if merge_si >= 0 {
                region_end_si = merge_si;
            } else {
                let mut k = max_catch_target + 1;
                while k < n {
                    if RE_LABEL_NUM_COLON.is_match(stmts[k as usize].trim()) {
                        region_end_si = k;
                        break;
                    }
                    k += 1;
                }
            }
            if region_end_si < 0 {
                region_end_si = max_catch_target + 2;
            }
            let mut repl: Vec<String> = vec!["try".into(), "{".into()];
            repl.extend(try_body);
            repl.push("}".into());
            for (cc, cb) in catch_blocks {
                repl.push(cc);
                repl.push("{".into());
                repl.extend(cb);
                repl.push("}".into());
            }
            repl.push(";".into());

            let finally_block = finally_map.iter().find(|(fk, _)| fk.0 == from_pos && fk.1 > to_pos).map(|(_, f)| f.clone());
            if let Some(fb) = finally_block {
                let ftarget_si = fb.target_si;
                if ftarget_si >= 0 {
                    let mut farthest = ftarget_si;
                    let mut k = region_end_si;
                    while k < n {
                        let ms = stmts[k.max(0) as usize].trim();
                        if let Some(gm) = RE_DEFAULT_GOTO.captures(ms) {
                            let target_off: i64 = gm[2].parse().unwrap_or(-1);
                            let tsi = lp(target_off);
                            if tsi > farthest {
                                farthest = tsi;
                            }
                        }
                        if RE_LABEL_NUM_COLON.is_match(ms) && k > farthest {
                            break;
                        }
                        if let Some(cm) = RE_CASE_GOTO.captures(ms) {
                            let target_off: i64 = cm[1].parse().unwrap_or(-1);
                            let tsi = lp(target_off);
                            if tsi > farthest {
                                farthest = tsi;
                            }
                        }
                        k += 1;
                    }
                    region_end_si = farthest;
                }
            }
            replacements.push((from_si, region_end_si, repl));
        }
        if replacements.is_empty() {
            return stmts;
        }
        replacements.sort_by_key(|r| (r.1 - r.0, r.0));
        let mut starts: Vec<(i64, Vec<(i64, i64, Vec<String>)>)> = Vec::new();
        for r in &replacements {
            if let Some(g) = starts.iter_mut().find(|g| g.0 == r.0) {
                g.1.push(r.clone());
            } else {
                starts.push((r.0, vec![r.clone()]));
            }
        }
        let mut final_replacements: Vec<(i64, i64, Vec<String>)> = Vec::new();
        for (_, mut group) in starts {
            if group.len() == 1 {
                final_replacements.push(group.remove(0));
            } else {
                group.sort_by_key(|r| r.1 - r.0);
                let mut inner = group[0].clone();
                for outer in group.iter().skip(1) {
                    let (outer_start, outer_end, outer_repl) = outer.clone();
                    let inner_repl = inner.2.clone();
                    let mut new_repl = Vec::new();
                    let mut in_try_body = false;
                    let mut try_body_emitted = false;
                    for line in outer_repl {
                        if line == "{" && !in_try_body && !try_body_emitted {
                            new_repl.push(line);
                            new_repl.extend(inner_repl.iter().cloned());
                            in_try_body = true;
                            try_body_emitted = true;
                            continue;
                        }
                        if in_try_body {
                            if line == "}" {
                                in_try_body = false;
                                new_repl.push(line);
                            }
                            continue;
                        }
                        new_repl.push(line);
                    }
                    inner = (outer_start, outer_end, new_repl);
                }
                final_replacements.push(inner);
            }
        }
        final_replacements.sort_by_key(|r| r.0);
        let mut repl_by_start: HashMap<i64, (i64, i64, Vec<String>)> = HashMap::new();
        for r in final_replacements {
            repl_by_start.insert(r.0, r);
        }
        let mut result = Vec::new();
        let mut skip_until: i64 = -1;
        for idx in 0..stmts.len() {
            if (idx as i64) < skip_until {
                continue;
            }
            if let Some((_, end, repl)) = repl_by_start.get(&(idx as i64)) {
                result.extend(repl.iter().cloned());
                skip_until = *end;
            } else {
                result.push(stmts[idx].clone());
            }
        }
        result
    }
}

// --------------------------------------------------------------- switch

struct SwitchRange {
    range_start: usize,
    first_body_pos: usize,
    chain_start_pos: usize,
    switch_block_end: usize,
    break_label: Option<String>,
    break_label_pos: usize,
    case_targets: Vec<(i64, String)>,
    default_label: Option<String>,
    all_case_labels: HashSet<String>,
    switch_var: String,
    case_values: HashMap<i64, String>,
}

impl<'a> MethodDecompiler<'a> {
    fn fold_switch(stmts: Vec<String>) -> Vec<String> {
        let mut label_pos: HashMap<String, usize> = HashMap::new();
        for (idx, s) in stmts.iter().enumerate() {
            if let Some(m) = RE_LABEL_COLON.captures(s.trim()) {
                label_pos.insert(m[1].to_string(), idx);
            }
        }
        let mut switch_ranges: Vec<SwitchRange> = Vec::new();
        for (idx, s) in stmts.iter().enumerate() {
            if !s.trim().starts_with("switch (") {
                continue;
            }
            let mut j = idx + 1;
            let mut case_targets: Vec<(i64, String)> = Vec::new();
            let mut default_label: Option<String> = None;
            while j < stmts.len() {
                let cs = stmts[j].trim();
                j += 1;
                if cs == "}" {
                    break;
                }
                if let Some(cm) = RE_CASE_NUM_GOTO.captures(cs) {
                    let k: i64 = cm[1].parse().unwrap_or(0);
                    if let Some(e) = case_targets.iter_mut().find(|e| e.0 == k) {
                        e.1 = cm[2].to_string();
                    } else {
                        case_targets.push((k, cm[2].to_string()));
                    }
                }
                if let Some(dm) = RE_DEFAULT_GOTO2.captures(cs) {
                    default_label = Some(dm[1].to_string());
                }
            }
            let switch_block_end = j;
            if case_targets.is_empty() {
                continue;
            }
            let mut all_case_labels: HashSet<String> = case_targets.iter().map(|e| e.1.clone()).collect();
            if let Some(d) = &default_label {
                all_case_labels.insert(d.clone());
            }
            let mut body_positions: Vec<usize> =
                all_case_labels.iter().filter_map(|l| label_pos.get(l).copied()).collect();
            body_positions.sort();
            if body_positions.is_empty() || body_positions[0] >= idx {
                continue;
            }
            let first_body_pos = body_positions[0];
            let mut initial_goto_idx: Option<usize> = None;
            let mut chain_label: Option<String> = None;
            let mut k = first_body_pos as i64 - 1;
            while k >= 0 {
                let cs = stmts[k as usize].trim();
                if let Some(mg) = RE_GOTO_LABEL.captures(cs) {
                    initial_goto_idx = Some(k as usize);
                    chain_label = Some(mg[1].to_string());
                    break;
                }
                if !cs.is_empty() && !cs.ends_with(':') && !cs.starts_with("var ") {
                    break;
                }
                k -= 1;
            }
            let chain_start_pos = match &chain_label {
                Some(l) => label_pos.get(l).copied().unwrap_or(idx),
                None => idx,
            };
            let mut break_label: Option<String> = None;
            for k in switch_block_end..stmts.len() {
                if let Some(ml) = RE_LABEL_COLON.captures(stmts[k].trim()) {
                    break_label = Some(ml[1].to_string());
                    break;
                }
                if !stmts[k].trim().is_empty() {
                    break;
                }
            }
            let mut goto_counts: Vec<(String, usize)> = Vec::new();
            for k in first_body_pos..chain_start_pos {
                if let Some(mg) = RE_GOTO_LABEL.captures(stmts[k].trim()) {
                    let tgt = mg[1].to_string();
                    if !all_case_labels.contains(&tgt) && Some(&tgt) != chain_label.as_ref() {
                        let tgt_pos: i64 = label_pos.get(&tgt).map_or(-1, |&p| p as i64);
                        if tgt_pos >= switch_block_end as i64 || tgt_pos < first_body_pos as i64 {
                            if let Some(e) = goto_counts.iter_mut().find(|e| e.0 == tgt) {
                                e.1 += 1;
                            } else {
                                goto_counts.push((tgt, 1));
                            }
                        }
                    }
                }
            }
            if !goto_counts.is_empty() {
                // max() over insertion order: first maximal entry wins
                let mut best = &goto_counts[0];
                for e in &goto_counts {
                    if e.1 > best.1 {
                        best = e;
                    }
                }
                let likely = best.0.clone();
                let lc = best.1;
                let bc = break_label
                    .as_ref()
                    .and_then(|b| goto_counts.iter().find(|e| &e.0 == b))
                    .map_or(0, |e| e.1);
                if break_label.is_none() || lc > bc {
                    break_label = Some(likely);
                }
            }
            let mut switch_var: Option<String> = None;
            let mut case_values: HashMap<i64, String> = HashMap::new();
            let mut cmp_count = 0i64;
            for k in chain_start_pos..idx {
                let cs = stmts[k].trim();
                if let Some(m) = RE_IF_CMP_GOTO.captures(cs) {
                    let val_str = m[1].to_string();
                    let var_str = m[3].to_string();
                    if switch_var.is_none() {
                        switch_var = Some(var_str);
                    }
                    case_values.insert(cmp_count, val_str);
                    cmp_count += 1;
                }
            }
            if let Some(sv) = switch_var.clone() {
                let r = rx(&format!(r"^var {}:\* = (.+);$", esc(&sv)));
                for k in chain_start_pos..idx {
                    if let Some(m) = r.captures(stmts[k].trim()) {
                        switch_var = Some(m[1].to_string());
                        break;
                    }
                }
            }
            let switch_var = switch_var.unwrap_or_else(|| "?".into());
            let range_start = initial_goto_idx.unwrap_or(first_body_pos);
            let break_label_pos = match &break_label {
                Some(b) => label_pos.get(b).copied().unwrap_or(switch_block_end),
                None => switch_block_end,
            };
            switch_ranges.push(SwitchRange {
                range_start,
                first_body_pos,
                chain_start_pos,
                switch_block_end,
                break_label,
                break_label_pos,
                case_targets,
                default_label,
                all_case_labels,
                switch_var,
                case_values,
            });
        }
        if switch_ranges.is_empty() {
            return stmts;
        }
        let _ = switch_ranges.iter().map(|s| s.first_body_pos).count();
        let mut result: Vec<String> = Vec::new();
        let mut skip_until: usize = 0;
        let mut have_skip = false;
        for idx in 0..stmts.len() {
            if have_skip && idx < skip_until {
                continue;
            }
            let sw = switch_ranges.iter().find(|sr| idx == sr.range_start);
            let Some(sw) = sw else {
                result.push(stmts[idx].clone());
                continue;
            };
            let mut label_to_cases: Vec<(String, Vec<i64>)> = Vec::new();
            for (ci2, lbl) in &sw.case_targets {
                if let Some(e) = label_to_cases.iter_mut().find(|e| &e.0 == lbl) {
                    e.1.push(*ci2);
                } else {
                    label_to_cases.push((lbl.clone(), vec![*ci2]));
                }
            }
            let mut sorted_targets: Vec<(usize, String)> =
                sw.all_case_labels.iter().map(|l| (label_pos.get(l).copied().unwrap_or(9999), l.clone())).collect();
            sorted_targets.sort();
            result.push(format!("switch ({})", sw.switch_var));
            result.push("{".into());
            let mut processed: HashSet<String> = HashSet::new();
            for (bpos, blabel) in &sorted_targets {
                if processed.contains(blabel) {
                    continue;
                }
                processed.insert(blabel.clone());
                let mut cases = label_to_cases.iter().find(|e| &e.0 == blabel).map_or(vec![], |e| e.1.clone());
                cases.sort();
                for ci2 in cases {
                    let val = sw.case_values.get(&ci2).cloned().unwrap_or_else(|| ci2.to_string());
                    result.push(format!("{INDENT_UNIT}case {val}:"));
                }
                if sw.default_label.as_ref() == Some(blabel) {
                    result.push(format!("{INDENT_UNIT}default:"));
                }
                let body_start = bpos + 1;
                let mut body_end = sw.chain_start_pos;
                for (bpos2, _) in &sorted_targets {
                    if bpos2 > bpos {
                        body_end = *bpos2;
                        break;
                    }
                }
                let mut has_break = false;
                for k in body_start..body_end {
                    let cs = stmts[k].trim();
                    if cs.is_empty() {
                        continue;
                    }
                    if cs.ends_with(':') {
                        continue;
                    }
                    if let Some(bl) = &sw.break_label {
                        if cs == format!("goto {bl};") {
                            has_break = true;
                            continue;
                        }
                    }
                    if let Some(mg) = RE_GOTO_LABEL.captures(cs) {
                        if sw.all_case_labels.contains(&mg[1]) {
                            continue;
                        }
                    }
                    result.push(format!("{INDENT_UNIT}{INDENT_UNIT}{cs}"));
                }
                if has_break {
                    result.push(format!("{INDENT_UNIT}{INDENT_UNIT}break;"));
                }
            }
            result.push("}".into());
            skip_until = if sw.break_label_pos < stmts.len() { sw.break_label_pos + 1 } else { sw.switch_block_end };
            have_skip = true;
            if let Some(blab) = &sw.break_label {
                if sw.break_label_pos < stmts.len() {
                    let range_s = sw.range_start;
                    let range_e = skip_until;
                    let needle = format!("goto {blab};");
                    for (ext_idx, ext_s) in stmts.iter().enumerate() {
                        if ext_idx >= range_s && ext_idx < range_e {
                            continue;
                        }
                        if ext_s.contains(&needle) {
                            result.push(format!("{blab}:"));
                            break;
                        }
                    }
                }
            }
        }
        result
    }

    fn fold_if_else_return_chains(stmts: Vec<String>) -> Vec<String> {
        let mut result = stmts;
        fn block_ends_with_return(result: &[String], end_idx: usize) -> bool {
            for k in (0..end_idx).rev() {
                let prev = result[k].trim();
                if !prev.is_empty() && prev != "{" && !prev.starts_with("//") {
                    return prev.starts_with("return ")
                        || prev.starts_with("return(")
                        || prev == "return;"
                        || prev.starts_with("throw ");
                }
            }
            false
        }
        fn indent_of(s: &str) -> usize {
            s.chars().count() - s.trim_start().chars().count()
        }
        let mut i = 0;
        while i < result.len() {
            let s = result[i].trim().to_string();
            if s == "};" && i + 1 < result.len() {
                let next_s = result[i + 1].trim().to_string();
                if next_s.starts_with("if (") && block_ends_with_return(&result, i) {
                    let i1 = indent_of(&result[i]);
                    let i2 = indent_of(&result[i + 1]);
                    if i1 == i2 {
                        result[i] = result[i].replace("};", "}");
                        result.insert(i + 1, "else".into());
                    }
                }
            }
            i += 1;
        }
        let mut i = 0;
        while i < result.len() {
            let s = result[i].trim().to_string();
            if s == "};" && i + 1 < result.len() {
                let next_s = result[i + 1].trim().to_string();
                if (next_s.starts_with("return ") || next_s.starts_with("return(") || next_s == "return;" || next_s.starts_with("throw "))
                    && block_ends_with_return(&result, i)
                {
                    let i1 = indent_of(&result[i]);
                    let i2 = indent_of(&result[i + 1]);
                    if i1 != i2 {
                        i += 1;
                        continue;
                    }
                    let mut in_chain = false;
                    let lo = (i as i64 - 30).max(-1);
                    let mut k = i as i64 - 1;
                    while k > lo {
                        let pk = result[k as usize].trim();
                        if pk == "else" {
                            in_chain = true;
                            break;
                        }
                        if pk == "{" || pk == "};" || pk.starts_with("if (") {
                            k -= 1;
                            continue;
                        }
                        if pk.starts_with("return ") || pk.starts_with("return(") || pk.starts_with("throw ") {
                            k -= 1;
                            continue;
                        }
                        break;
                    }
                    if in_chain {
                        result[i] = result[i].replace("};", "}");
                        result.insert(i + 1, "else".into());
                        result.insert(i + 2, "{".into());
                        let ret_idx = i + 3;
                        result.insert(ret_idx + 1, "};".into());
                    }
                }
            }
            i += 1;
        }
        result
    }

    fn fold_goto_dowhile(stmts: Vec<String>) -> Vec<String> {
        let mut result = Vec::new();
        let mut i = 0;
        while i < stmts.len() {
            let s = stmts[i].trim();
            if RE_GOTO_LABEL_BARE.is_match(s) && i + 1 < stmts.len() && stmts[i + 1].trim() == "do" {
                let do_line = &stmts[i + 1];
                let indent: String = do_line.chars().take(do_line.chars().count() - do_line.trim_start().chars().count()).collect();
                let mut j = i + 2;
                if j < stmts.len() && stmts[j].trim() == "{" {
                    let mut depth = 1;
                    j += 1;
                    while j < stmts.len() && depth > 0 {
                        let line = stmts[j].trim();
                        if line == "{" {
                            depth += 1;
                        } else if line.starts_with("} while (") || line == "}" {
                            depth -= 1;
                        }
                        j += 1;
                    }
                    let close_line = stmts[j - 1].trim();
                    if let Some(m) = RE_WHILE_CLOSE.captures(close_line) {
                        let cond = m[1].to_string();
                        result.push(format!("{indent}while ({cond})"));
                        result.push(format!("{indent}{{"));
                        for k in i + 3..j - 1 {
                            result.push(stmts[k].clone());
                        }
                        result.push(format!("{indent}}};"));
                        i = j;
                        continue;
                    }
                }
            }
            result.push(stmts[i].clone());
            i += 1;
        }
        result
    }

    fn count_net_braces(line: &str) -> i32 {
        let s = line.trim();
        if s.starts_with("//") {
            return 0;
        }
        let b = s.as_bytes();
        let mut count = 0;
        let mut in_str: Option<u8> = None;
        let mut idx = 0;
        while idx < b.len() {
            let c = b[idx];
            if let Some(q) = in_str {
                if c == b'\\' {
                    idx += 2;
                    continue;
                }
                if c == q {
                    in_str = None;
                }
            } else if c == b'"' || c == b'\'' {
                in_str = Some(c);
            } else if c == b'{' {
                count += 1;
            } else if c == b'}' {
                count -= 1;
            }
            idx += 1;
        }
        count
    }

    fn fold_while_to_for(stmts: Vec<String>) -> Vec<String> {
        let mut result: Vec<String> = Vec::new();
        let mut i = 0;
        while i < stmts.len() {
            let mut matched = false;
            let s_stripped = stmts[i].trim().to_string();
            let (loop_label_prefix, while_core) = match RE_LOOP_LABEL.captures(&s_stripped) {
                Some(m) => (m[1].to_string(), s_stripped[m.get(0).unwrap().end()..].to_string()),
                None => (String::new(), s_stripped.clone()),
            };
            if let Some(mw) = RE_WHILE_COND.captures(&while_core) {
                if while_core != "while (true)" {
                    let cond = mw[1].to_string();
                    if i + 1 < stmts.len() && stmts[i + 1].trim() == "{" {
                        let mut depth = 1;
                        let mut j = i + 2;
                        while j < stmts.len() && depth > 0 {
                            depth += Self::count_net_braces(&stmts[j]);
                            j += 1;
                        }
                        let close_idx = j - 1;
                        if depth == 0 && close_idx > i + 2 {
                            let last_body_idx = close_idx - 1;
                            let last_s = stmts[last_body_idx].trim().to_string();
                            if let Some((var_name, init_expr, remove_idx)) = Self::find_for_init(&result, &cond, &last_s) {
                                if let Some(step_expr) = Self::match_step(&var_name, &last_s) {
                                    result.remove(remove_idx);
                                    result.push(format!("{loop_label_prefix}for ({init_expr}; {cond}; {step_expr})"));
                                    result.push("{".into());
                                    for k in i + 2..last_body_idx {
                                        result.push(stmts[k].clone());
                                    }
                                    result.push(stmts[close_idx].clone());
                                    i = close_idx + 1;
                                    matched = true;
                                }
                            }
                        }
                    }
                }
            }
            if !matched {
                result.push(stmts[i].clone());
                i += 1;
            }
        }
        Self::fold_while_to_for_recursive(result)
    }

    fn find_for_init(result: &[String], cond: &str, last_body: &str) -> Option<(String, String, usize)> {
        let max_scan = result.len().min(6);
        for back in 1..=max_scan {
            let idx = result.len() as i64 - back as i64;
            if idx < 0 {
                break;
            }
            let idx = idx as usize;
            let candidate = result[idx].trim();
            let var_name;
            let init_expr;
            if let Some(m) = RE_FOR_VAR_INIT.captures(candidate) {
                var_name = m[1].to_string();
                let vt = m.get(2).map_or("", |x| x.as_str());
                init_expr = format!("var {var_name}{vt} = {}", &m[3]);
            } else if let Some(m) = RE_FOR_VAR_DECL.captures(candidate) {
                var_name = m[1].to_string();
                let vt = m.get(2).map_or("", |x| x.as_str());
                init_expr = format!("var {var_name}{vt} = 0");
            } else if let Some(m) = RE_FOR_ASSIGN.captures(candidate) {
                var_name = m[1].to_string();
                init_expr = format!("{var_name} = {}", &m[2]);
            } else {
                break;
            }
            let wre = rx(&format!(r"\b{}\b", esc(&var_name)));
            if wre.is_match(cond) && wre.is_match(last_body) {
                return Some((var_name, init_expr, idx));
            }
            if !RE_FOR_VAR_DECL.is_match(candidate) && !RE_FOR_VAR_INIT.is_match(candidate) {
                break;
            }
        }
        None
    }

    fn match_step(var_name: &str, last_s: &str) -> Option<String> {
        let vn = esc(var_name);
        if last_s == format!("{var_name}++;") {
            return Some(format!("{var_name}++"));
        }
        if last_s == format!("{var_name}--;") {
            return Some(format!("{var_name}--"));
        }
        let pats: [(String, &str); 10] = [
            (format!(r"^{vn} \+= (.+);$"), "+="),
            (format!(r"^{vn} -= (.+);$"), "-="),
            (format!(r"^{vn} = {vn} \+ (.+);$"), "+="),
            (format!(r"^{vn} = {vn} - (.+);$"), "-="),
            (format!(r"^{vn} = \({vn} \+ (.+)\);$"), "+="),
            (format!(r"^{vn} = \({vn} - (.+)\);$"), "-="),
            (format!(r"^{vn} = int\(\({vn} \+ (.+)\)\);$"), "+="),
            (format!(r"^{vn} = int\(\({vn} - (.+)\)\);$"), "-="),
            (format!(r"^{vn} = uint\(\({vn} \+ (.+)\)\);$"), "+="),
            (format!(r"^{vn} = uint\(\({vn} - (.+)\)\);$"), "-="),
        ];
        for (p, o) in pats {
            if let Some(m) = rx(&p).captures(last_s) {
                return Some(format!("{var_name} {o} {}", &m[1]));
            }
        }
        None
    }

    fn fold_while_to_for_recursive(stmts: Vec<String>) -> Vec<String> {
        let mut result = Vec::new();
        let mut i = 0;
        while i < stmts.len() {
            let s = stmts[i].trim().to_string();
            if i + 1 < stmts.len() && stmts[i + 1].trim() == "{" {
                let mut depth = 1;
                let mut j = i + 2;
                while j < stmts.len() && depth > 0 {
                    depth += Self::count_net_braces(&stmts[j]);
                    j += 1;
                }
                let close_idx = j - 1;
                if depth == 0 {
                    result.push(stmts[i].clone());
                    result.push(stmts[i + 1].clone());
                    let inner = Self::fold_while_to_for(stmts[i + 2..close_idx].to_vec());
                    result.extend(inner);
                    result.push(stmts[close_idx].clone());
                    i = close_idx + 1;
                    continue;
                }
            }
            let net = Self::count_net_braces(&s);
            if net > 0 && s != "{" && !s.starts_with("//") {
                let mut depth = net;
                let mut j = i + 1;
                while j < stmts.len() && depth > 0 {
                    depth += Self::count_net_braces(&stmts[j]);
                    j += 1;
                }
                let close_idx = j - 1;
                if depth == 0 {
                    result.push(stmts[i].clone());
                    let inner = Self::fold_while_to_for(stmts[i + 1..close_idx].to_vec());
                    result.extend(inner);
                    result.push(stmts[close_idx].clone());
                    i = close_idx + 1;
                    continue;
                }
            }
            result.push(stmts[i].clone());
            i += 1;
        }
        result
    }

    fn fold_for_each_in(stmts: Vec<String>) -> Vec<String> {
        let mut result: Vec<String> = Vec::new();
        let mut i = 0;
        while i < stmts.len() {
            let s = stmts[i].trim().to_string();
            if let Some(mw) = RE_WHILE_HASNEXT.captures(&s) {
                let obj_var = mw[1].to_string();
                let idx_var = mw[2].to_string();
                if i + 1 < stmts.len() && stmts[i + 1].trim() == "{" {
                    let brace_start = i + 1;
                    let mut depth = 1;
                    let mut j = brace_start + 1;
                    while j < stmts.len() && depth > 0 {
                        let line = stmts[j].trim();
                        if line == "{" {
                            depth += 1;
                        } else if line == "};" || line == "}" {
                            depth -= 1;
                        }
                        j += 1;
                    }
                    let brace_end = j - 1;
                    if brace_start + 1 < brace_end {
                        let first_body = stmts[brace_start + 1].trim().to_string();
                        let (eo, ei) = (esc(&obj_var), esc(&idx_var));
                        let m_nv = rx(&format!(r"^(\w+)\s*=\s*(?:\w+\()?\s*nextvalue\({eo},\s*{ei}\)\)?;$")).captures(&first_body).map(|c| (c[1].to_string(), None::<String>));
                        let m_nn = rx(&format!(r"^(\w+)\s*=\s*nextname\({eo},\s*{ei}\);$")).captures(&first_body).map(|c| (c[1].to_string(), None::<String>));
                        let m_nv_var = rx(&format!(r"^var\s+(\w+)(:\w+\*?)?\s*=\s*(?:\w+\()?\s*nextvalue\({eo},\s*{ei}\)\)?;$"))
                            .captures(&first_body)
                            .map(|c| (c[1].to_string(), c.get(2).map(|x| x.as_str().to_string())));
                        let m_nn_var = rx(&format!(r"^var\s+(\w+)(:\w+\*?)?\s*=\s*nextname\({eo},\s*{ei}\);$"))
                            .captures(&first_body)
                            .map(|c| (c[1].to_string(), c.get(2).map(|x| x.as_str().to_string())));
                        let is_for_each = m_nv.is_some() || m_nv_var.is_some();
                        let is_for_in = m_nn.is_some() || m_nn_var.is_some();
                        if is_for_each || is_for_in {
                            let loop_var = m_nv
                                .as_ref()
                                .or(m_nv_var.as_ref())
                                .or(m_nn.as_ref())
                                .or(m_nn_var.as_ref())
                                .unwrap()
                                .0
                                .clone();
                            let mut collection = obj_var.clone();
                            let mut remove: HashSet<usize> = HashSet::new();
                            let r_obj = rx(&format!(r"^(?:var\s+)?{eo}(?::\S+)?\s*=\s*(.+);$"));
                            for k in (0..result.len()).rev() {
                                if let Some(m) = r_obj.captures(result[k].trim()) {
                                    collection = m[1].to_string();
                                    remove.insert(k);
                                    break;
                                }
                            }
                            let r_idx0 = rx(&format!(r"^(?:var\s+)?{ei}(?::\w+)?\s*=\s*0;$"));
                            let r_idx1 = rx(&format!(r"^var\s+{ei}:int;$"));
                            for k in (0..result.len()).rev() {
                                let rl = result[k].trim();
                                if r_idx0.is_match(rl) {
                                    remove.insert(k);
                                    break;
                                } else if r_idx1.is_match(rl) {
                                    remove.insert(k);
                                    break;
                                }
                            }
                            let mut loop_var_type = String::new();
                            if let Some((_, Some(t))) = &m_nv_var {
                                loop_var_type = t.clone();
                            } else if let Some((_, Some(t))) = &m_nn_var {
                                loop_var_type = t.clone();
                            } else {
                                let r_decl = rx(&format!(r"^var\s+{}(:\S+?)?\s*(?:=.*)?;$", esc(&loop_var)));
                                for k in (0..result.len()).rev() {
                                    if let Some(m) = r_decl.captures(result[k].trim()) {
                                        if let Some(g) = m.get(1) {
                                            loop_var_type = g.as_str().to_string();
                                        }
                                        remove.insert(k);
                                        break;
                                    }
                                }
                            }
                            let r_o1 = rx(&format!(r"^var\s+{eo}:\S+\s*=\s*.+;$"));
                            let r_o2 = rx(&format!(r"^var\s+{eo}:\S+;$"));
                            for k in (0..result.len()).rev() {
                                let rl = result[k].trim();
                                if r_o1.is_match(rl) {
                                    break;
                                } else if r_o2.is_match(rl) {
                                    remove.insert(k);
                                    break;
                                }
                            }
                            if !remove.is_empty() {
                                result = result.into_iter().enumerate().filter(|(ri, _)| !remove.contains(ri)).map(|(_, r)| r).collect();
                            }
                            let keyword = if is_for_each { "for each" } else { "for" };
                            result.push(format!("{keyword} (var {loop_var}{loop_var_type} in {collection})"));
                            result.push("{".into());
                            for k in brace_start + 2..brace_end {
                                result.push(stmts[k].clone());
                            }
                            result.push("};".into());
                            i = brace_end + 1;
                            continue;
                        }
                    }
                }
            }
            result.push(stmts[i].clone());
            i += 1;
        }
        result
    }

    // ------------------------------------------------------ structuring

    fn structure_flow(&mut self, stmts: Vec<String>) -> Vec<String> {
        let mut label_pos: HashMap<String, usize> = HashMap::new();
        for (i, s) in stmts.iter().enumerate() {
            if let Some(m) = RE_LABEL_NUM_COLON.captures(s.trim()) {
                label_pos.insert(format!("__label_{}", &m[1]), i);
            }
        }
        if label_pos.is_empty() {
            let mut stmts = stmts;
            if stmts.last().map_or(false, |l| l.trim() == "return;") {
                stmts.pop();
            }
            return stmts;
        }
        let prev_counter = self.loop_label_counter;
        let prev_labels = std::mem::take(&mut self.needs_loop_label);
        self.loop_label_counter = 0;
        let mut result = self.struct_block(&stmts, 0, stmts.len(), &label_pos, None, 0);
        while result.last().map_or(false, |l| l.trim() == "return;") {
            result.pop();
        }
        for _ in 0..25 {
            let mut changed = false;
            let mut tmp = Vec::new();
            for line in result {
                let st = line.trim();
                if st.contains("goto __label_") {
                    changed = true;
                    continue;
                }
                if RE_LABEL_WS.is_match(st) {
                    changed = true;
                    continue;
                }
                tmp.push(line);
            }
            result = tmp;
            if !changed {
                break;
            }
        }
        while result.last().map_or(false, |l| l.trim().is_empty()) {
            result.pop();
        }
        self.loop_label_counter = prev_counter;
        self.needs_loop_label = prev_labels;
        result
    }

    fn next_loop_label(&mut self) -> String {
        self.loop_label_counter += 1;
        format!("_loop_{}", self.loop_label_counter)
    }

    fn build_break_label_map(&self, own: &HashSet<String>, outer: Option<&LoopCtx>) -> HashMap<String, Option<String>> {
        let mut m: HashMap<String, Option<String>> = HashMap::new();
        for l in own {
            m.insert(l.clone(), None);
        }
        if let Some(o) = outer {
            for (lbl, info) in &o.break_label_map {
                if !m.contains_key(lbl) {
                    match info {
                        None => {
                            m.insert(lbl.clone(), Some(o.loop_label.clone()));
                        }
                        Some(x) => {
                            m.insert(lbl.clone(), Some(x.clone()));
                        }
                    }
                }
            }
        }
        m
    }

    fn find_back_goto(stmts: &[String], label_idx: usize, end: usize, label_name: &str) -> Option<usize> {
        let goto_exact = format!("goto {label_name};");
        let suffix = format!(") goto {label_name};");
        for j in label_idx + 1..end {
            let s = stmts[j].trim();
            if s == goto_exact {
                return Some(j);
            }
            if s.starts_with("if (") && s.ends_with(&suffix) {
                return Some(j);
            }
        }
        None
    }

    fn indent_block(out: &mut Vec<String>, lines: Vec<String>) {
        for l in lines {
            out.push(format!("{INDENT_UNIT}{l}"));
        }
    }

    fn struct_block(
        &mut self,
        stmts: &[String],
        start: usize,
        end: usize,
        label_pos: &HashMap<String, usize>,
        loop_ctx: Option<&LoopCtx>,
        depth: usize,
    ) -> Vec<String> {
        if depth > MAX_STRUCT_DEPTH {
            return (start..end).filter(|&j| !stmts[j].trim().is_empty()).map(|j| stmts[j].clone()).collect();
        }
        let mut result: Vec<String> = Vec::new();
        let mut i = start;
        while i < end {
            let s = stmts[i].trim().to_string();
            if s.is_empty() {
                i += 1;
                continue;
            }
            if s.starts_with("__label_") && s.ends_with(':') {
                let label_name = s[..s.len() - 1].to_string();
                if let Some(back_pos) = Self::find_back_goto(stmts, i, end, &label_name) {
                    i = self.emit_loop(stmts, i, back_pos, end, &label_name, label_pos, &mut result, loop_ctx, depth);
                    continue;
                }
                i += 1;
                continue;
            }
            if let Some(m) = RE_IF_GOTO.captures(&s) {
                let cond = m[1].to_string();
                let target = m[2].to_string();
                let target_pos = label_pos.get(&target).map_or(-1, |&p| p as i64);
                if target_pos > i as i64 {
                    i = self.emit_if(stmts, i, &cond, &target, target_pos as usize, end, label_pos, &mut result, loop_ctx, depth);
                    continue;
                }
                result.push(s);
                i += 1;
                continue;
            }
            if s.starts_with("switch (") {
                result.push(s);
                i += 1;
                while i < end {
                    let si = stmts[i].trim().to_string();
                    result.push(si.clone());
                    i += 1;
                    if si == "}" {
                        break;
                    }
                }
                continue;
            }
            if let Some(mg) = RE_GOTO_LABEL.captures(&s) {
                let target = mg[1].to_string();
                let target_pos = label_pos.get(&target).map_or(-1, |&p| p as i64);
                if let Some(lc) = loop_ctx {
                    if lc.continue_labels.contains(&target) {
                        result.push("continue;".into());
                        i += 1;
                        continue;
                    }
                    if let Some(info) = lc.break_label_map.get(&target) {
                        match info {
                            None => result.push("break;".into()),
                            Some(l) => {
                                self.needs_loop_label.insert(l.clone());
                                result.push(format!("break {l};"));
                            }
                        }
                        i += 1;
                        continue;
                    }
                }
                if target_pos > i as i64 {
                    let tp = target_pos as usize;
                    let next_i = i + 1;
                    if next_i < end {
                        let next_s = stmts[next_i].trim();
                        if let Some(mb) = RE_LABEL_COLON.captures(next_s) {
                            let body_label = mb[1].to_string();
                            let mut cpos = tp;
                            while cpos < end && RE_LABEL_NUM_COLON.is_match(stmts[cpos].trim()) {
                                cpos += 1;
                            }
                            if cpos < end {
                                let rc = rx(&format!(r"^if \((.+)\) goto {};$", esc(&body_label)));
                                if let Some(mc) = rc.captures(stmts[cpos].trim()) {
                                    let cond = mc[1].to_string();
                                    let loop_exit_pos = cpos + 1;
                                    let mut exit_labels = HashSet::new();
                                    if loop_exit_pos < stmts.len() {
                                        if let Some(me) = RE_LABEL_COLON.captures(stmts[loop_exit_pos].trim()) {
                                            exit_labels.insert(me[1].to_string());
                                        }
                                    }
                                    let mut cont_labels = HashSet::new();
                                    if let Some(mcl) = RE_LABEL_COLON.captures(stmts[tp].trim()) {
                                        cont_labels.insert(mcl[1].to_string());
                                    }
                                    let mut cl_idx = tp as i64 - 1;
                                    while cl_idx > next_i as i64 {
                                        let cl_s = stmts[cl_idx as usize].trim();
                                        if let Some(mcl) = RE_LABEL_COLON.captures(cl_s) {
                                            cont_labels.insert(mcl[1].to_string());
                                        } else if !cl_s.is_empty() && (cl_s.contains("goto") || cl_s.starts_with("if ")) {
                                            break;
                                        }
                                        cl_idx -= 1;
                                    }
                                    let w_loop_label = self.next_loop_label();
                                    let inner_ctx = LoopCtx {
                                        continue_labels: cont_labels,
                                        break_label_map: self.build_break_label_map(&exit_labels, loop_ctx),
                                        loop_label: w_loop_label.clone(),
                                    };
                                    let inner = self.struct_block(stmts, next_i + 1, tp, label_pos, Some(&inner_ctx), depth + 1);
                                    let mut while_line = format!("while ({cond})");
                                    if self.needs_loop_label.contains(&w_loop_label) {
                                        while_line = format!("{w_loop_label}: {while_line}");
                                    }
                                    result.push(while_line);
                                    result.push("{".into());
                                    Self::indent_block(&mut result, inner);
                                    result.push("};".into());
                                    i = cpos + 1;
                                    continue;
                                }
                            }
                            if cpos < end && stmts[cpos].trim() == format!("goto {body_label};") {
                                let mut exit_labels = HashSet::new();
                                for el_idx in cpos + 1..(cpos + 3).min(stmts.len()) {
                                    if let Some(me) = RE_LABEL_COLON.captures(stmts[el_idx].trim()) {
                                        exit_labels.insert(me[1].to_string());
                                        break;
                                    }
                                }
                                let wt = self.next_loop_label();
                                let inner_ctx = LoopCtx {
                                    continue_labels: HashSet::new(),
                                    break_label_map: self.build_break_label_map(&exit_labels, loop_ctx),
                                    loop_label: wt.clone(),
                                };
                                let inner = self.struct_block(stmts, next_i + 1, tp, label_pos, Some(&inner_ctx), depth + 1);
                                let mut while_line = "while (true)".to_string();
                                if self.needs_loop_label.contains(&wt) {
                                    while_line = format!("{wt}: {while_line}");
                                }
                                result.push(while_line);
                                result.push("{".into());
                                Self::indent_block(&mut result, inner);
                                result.push("};".into());
                                i = cpos + 1;
                                continue;
                            }
                        }
                    }
                }
                if target_pos > i as i64 {
                    let tp = target_pos as usize;
                    let next_i = i + 1;
                    if next_i < end {
                        if let Some(mb) = RE_LABEL_COLON.captures(stmts[next_i].trim()) {
                            let body_label = mb[1].to_string();
                            if let Some(back_pos) = Self::find_back_goto(stmts, next_i, end, &body_label) {
                                if tp <= back_pos {
                                    i += 1;
                                    continue;
                                }
                            }
                        }
                    }
                }
                if target_pos >= 0 && target_pos < i as i64 {
                    result.push("continue;".into());
                } else if target_pos >= end as i64 {
                    result.push("break;".into());
                } else {
                    result.push(s);
                }
                i += 1;
                continue;
            }
            result.push(s);
            i += 1;
        }
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_loop(
        &mut self,
        stmts: &[String],
        label_idx: usize,
        back_pos: usize,
        end: usize,
        label_name: &str,
        label_pos: &HashMap<String, usize>,
        result: &mut Vec<String>,
        outer: Option<&LoopCtx>,
        depth: usize,
    ) -> usize {
        let back_stmt = stmts[back_pos].trim().to_string();
        let loop_label = self.next_loop_label();
        if back_stmt == format!("goto {label_name};") {
            let body_start = label_idx + 1;
            if body_start < back_pos {
                let first = stmts[body_start].trim();
                if let Some(m) = RE_IF_GOTO.captures(first) {
                    let exit_label = m[2].to_string();
                    let exit_pos = label_pos.get(&exit_label).map_or(-1, |&p| p as i64);
                    if exit_pos >= back_pos as i64 {
                        let cond = Self::negate_cond(&m[1]);
                        let mut exit_labels = HashSet::new();
                        exit_labels.insert(exit_label.clone());
                        let mut cont_labels = HashSet::new();
                        cont_labels.insert(label_name.to_string());
                        let mut cl_idx = back_pos as i64 - 1;
                        while cl_idx > body_start as i64 {
                            let cl_s = stmts[cl_idx as usize].trim();
                            if let Some(mcl) = RE_LABEL_COLON.captures(cl_s) {
                                cont_labels.insert(mcl[1].to_string());
                            } else if !cl_s.is_empty() && (cl_s.contains("goto") || cl_s.starts_with("if ")) {
                                break;
                            }
                            cl_idx -= 1;
                        }
                        let inner_ctx = LoopCtx {
                            continue_labels: cont_labels,
                            break_label_map: self.build_break_label_map(&exit_labels, outer),
                            loop_label: loop_label.clone(),
                        };
                        let inner = self.struct_block(stmts, body_start + 1, back_pos, label_pos, Some(&inner_ctx), depth + 1);
                        let mut while_line = format!("while ({cond})");
                        if self.needs_loop_label.contains(&loop_label) {
                            while_line = format!("{loop_label}: {while_line}");
                        }
                        result.push(while_line);
                        result.push("{".into());
                        Self::indent_block(result, inner);
                        result.push("};".into());
                        let mut nxt = exit_pos as usize;
                        if nxt < end && stmts[nxt].trim().starts_with("__label_") && stmts[nxt].trim().ends_with(':') {
                            nxt += 1;
                        }
                        return nxt;
                    }
                }
            }
            let mut exit_labels = HashSet::new();
            for el_idx in back_pos + 1..(back_pos + 3).min(stmts.len()) {
                if let Some(me) = RE_LABEL_COLON.captures(stmts[el_idx].trim()) {
                    exit_labels.insert(me[1].to_string());
                    break;
                }
            }
            let mut cont = HashSet::new();
            cont.insert(label_name.to_string());
            let inner_ctx = LoopCtx {
                continue_labels: cont,
                break_label_map: self.build_break_label_map(&exit_labels, outer),
                loop_label: loop_label.clone(),
            };
            let inner = self.struct_block(stmts, label_idx + 1, back_pos, label_pos, Some(&inner_ctx), depth + 1);
            let mut while_line = "while (true)".to_string();
            if self.needs_loop_label.contains(&loop_label) {
                while_line = format!("{loop_label}: {while_line}");
            }
            result.push(while_line);
            result.push("{".into());
            Self::indent_block(result, inner);
            result.push("};".into());
            return back_pos + 1;
        }
        let r = rx(&format!(r"^if \((.+)\) goto {};$", esc(label_name)));
        if let Some(m) = r.captures(&back_stmt) {
            let cond = m[1].to_string();
            let mut exit_labels = HashSet::new();
            for el_idx in back_pos + 1..(back_pos + 3).min(stmts.len()) {
                if let Some(me) = RE_LABEL_COLON.captures(stmts[el_idx].trim()) {
                    exit_labels.insert(me[1].to_string());
                    break;
                }
            }
            let mut cont = HashSet::new();
            cont.insert(label_name.to_string());
            let inner_ctx = LoopCtx {
                continue_labels: cont,
                break_label_map: self.build_break_label_map(&exit_labels, outer),
                loop_label: loop_label.clone(),
            };
            let inner = self.struct_block(stmts, label_idx + 1, back_pos, label_pos, Some(&inner_ctx), depth + 1);
            let mut do_line = "do".to_string();
            if self.needs_loop_label.contains(&loop_label) {
                do_line = format!("{loop_label}: do");
            }
            result.push(do_line);
            result.push("{".into());
            Self::indent_block(result, inner);
            result.push(format!("}} while ({cond});"));
            return back_pos + 1;
        }
        label_idx + 1
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_if(
        &mut self,
        stmts: &[String],
        if_idx: usize,
        cond: &str,
        _target: &str,
        target_pos: usize,
        end: usize,
        label_pos: &HashMap<String, usize>,
        result: &mut Vec<String>,
        loop_ctx: Option<&LoopCtx>,
        depth: usize,
    ) -> usize {
        let pre_target = target_pos as i64 - 1;
        if pre_target > if_idx as i64 {
            let pre = pre_target as usize;
            let pre_stmt = stmts[pre].trim();
            if let Some(m2) = RE_GOTO_LABEL.captures(pre_stmt) {
                let end_label = m2[1].to_string();
                let end_pos = label_pos.get(&end_label).map_or(-1, |&p| p as i64);
                let mut skip_ifelse = false;
                if pre == if_idx + 1 {
                    if let Some(lc) = loop_ctx {
                        if lc.continue_labels.contains(&end_label) || lc.break_label_map.contains_key(&end_label) {
                            skip_ifelse = true;
                        }
                    }
                }
                if !skip_ifelse && end_pos > target_pos as i64 && end_pos <= end as i64 {
                    let ep = end_pos as usize;
                    let neg = Self::negate_cond(cond);
                    let then_block = self.struct_block(stmts, if_idx + 1, pre, label_pos, loop_ctx, depth + 1);
                    let else_block = self.struct_block(stmts, target_pos + 1, ep, label_pos, loop_ctx, depth + 1);
                    result.push(format!("if ({neg})"));
                    result.push("{".into());
                    Self::indent_block(result, then_block);
                    result.push("}".into());
                    result.push("else".into());
                    result.push("{".into());
                    Self::indent_block(result, else_block);
                    result.push("};".into());
                    let mut nxt = ep;
                    if nxt < end && stmts[nxt].trim().starts_with("__label_") && stmts[nxt].trim().ends_with(':') {
                        nxt += 1;
                    }
                    return nxt;
                } else if end_pos > target_pos as i64 && end_pos > end as i64 {
                    if let Some(lc) = loop_ctx {
                        if let Some(brk) = lc.break_label_map.get(&end_label) {
                            let neg = Self::negate_cond(cond);
                            let mut then_block = self.struct_block(stmts, if_idx + 1, pre, label_pos, loop_ctx, depth + 1);
                            match brk {
                                None => then_block.push("break;".into()),
                                Some(l) => {
                                    self.needs_loop_label.insert(l.clone());
                                    then_block.push(format!("break {l};"));
                                }
                            }
                            let else_block = self.struct_block(stmts, target_pos + 1, end, label_pos, loop_ctx, depth + 1);
                            Self::emit_if_else_tail(result, &neg, then_block, else_block);
                            return end;
                        } else if lc.continue_labels.contains(&end_label) {
                            let neg = Self::negate_cond(cond);
                            let mut then_block = self.struct_block(stmts, if_idx + 1, pre, label_pos, loop_ctx, depth + 1);
                            then_block.push("continue;".into());
                            let else_block = self.struct_block(stmts, target_pos + 1, end, label_pos, loop_ctx, depth + 1);
                            Self::emit_if_else_tail(result, &neg, then_block, else_block);
                            return end;
                        }
                    }
                }
            }
        }
        let neg = Self::negate_cond(cond);
        let then_block = self.struct_block(stmts, if_idx + 1, target_pos, label_pos, loop_ctx, depth + 1);
        result.push(format!("if ({neg})"));
        result.push("{".into());
        Self::indent_block(result, then_block);
        result.push("};".into());
        let mut nxt = target_pos;
        if nxt < end && stmts[nxt].trim().starts_with("__label_") && stmts[nxt].trim().ends_with(':') {
            nxt += 1;
        }
        nxt
    }

    fn emit_if_else_tail(result: &mut Vec<String>, neg: &str, then_block: Vec<String>, else_block: Vec<String>) {
        if !else_block.is_empty() {
            result.push(format!("if ({neg})"));
            result.push("{".into());
            Self::indent_block(result, then_block);
            result.push("}".into());
            result.push("else".into());
            result.push("{".into());
            Self::indent_block(result, else_block);
            result.push("};".into());
        } else {
            result.push(format!("if ({neg})"));
            result.push("{".into());
            Self::indent_block(result, then_block);
            result.push("};".into());
        }
    }

    /// Negate a condition expression for structured flow.
    pub fn negate_cond(cond: &str) -> String {
        let cond = cond.trim();
        if cond.starts_with("!(") && cond.ends_with(')') {
            let inner = &cond[2..cond.len() - 1];
            let mut depth = 0i32;
            let mut balanced = true;
            for c in inner.chars() {
                if c == '(' {
                    depth += 1;
                } else if c == ')' {
                    depth -= 1;
                }
                if depth < 0 {
                    balanced = false;
                    break;
                }
            }
            if balanced && depth == 0 {
                return inner.to_string();
            }
        }
        if cond.starts_with('!') && !cond.contains('(') && !cond.contains(' ') {
            return cond[1..].to_string();
        }
        let b = cond.as_bytes();
        let mut has_logical = false;
        let mut depth = 0i32;
        let mut i = 0;
        while i + 1 < b.len() {
            if b[i] == b'(' {
                depth += 1;
            } else if b[i] == b')' {
                depth -= 1;
            } else if b[i] == b'"' {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            } else if depth == 0 && (&b[i..i + 2] == b"&&" || &b[i..i + 2] == b"||") {
                has_logical = true;
                break;
            }
            i += 1;
        }
        if has_logical {
            return if has_outer_parens(cond) { format!("!{cond}") } else { format!("!({cond})") };
        }
        let op_neg: [(&str, &str); 12] = [
            ("==", "!="),
            ("!=", "=="),
            ("===", "!=="),
            ("!==", "==="),
            ("<", ">="),
            (">=", "<"),
            (">", "<="),
            ("<=", ">"),
            ("!<", "<"),
            ("!<=", "<="),
            ("!>", ">"),
            ("!>=", ">="),
        ];
        // sorted(op_neg, key=len, reverse=True): stable, longer first
        let mut order: Vec<&(&str, &str)> = op_neg.iter().collect();
        order.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
        for (pos_op, neg_op) in order {
            let idx = find_op_outside_parens(cond, pos_op);
            if idx >= 0 {
                let idx = idx as usize;
                let left = cond[..idx].trim();
                let right = cond[idx + pos_op.len()..].trim();
                return format!("{left} {neg_op} {right}");
            }
        }
        if has_outer_parens(cond) {
            return format!("!{cond}");
        }
        if cond.ends_with(')')
            || cond.contains(").")
            || (!cond.replace('.', "").replace('_', "").is_empty()
                && cond.replace('.', "").replace('_', "").chars().all(|c| c.is_alphanumeric()))
        {
            return format!("!{cond}");
        }
        format!("!({cond})")
    }

    // ---------------------------------------------------------- ternary

    #[allow(clippy::too_many_arguments)]
    fn try_ternary(
        &self,
        code: &[u8],
        true_start: usize,
        false_label: usize,
        stack_copy: Vec<String>,
        local_names: &HashMap<u32, String>,
        slot_map: &HashMap<u32, String>,
        local0_name: &str,
        is_static: bool,
        class_idx: i64,
    ) -> Option<Ternary> {
        if false_label <= true_start || false_label > code.len() {
            return None;
        }
        let mut jump_pos: i64 = -1;
        let mut end_label: i64 = -1;
        let mut p = true_start;
        while p < false_label {
            let op = at(code, p);
            let op_start = p;
            p += 1;
            if op == OP_JUMP {
                let (off, np) = rs24(code, p);
                p = np;
                let jump_target = p as i64 + off as i64;
                if p == false_label {
                    jump_pos = op_start as i64;
                    end_label = jump_target;
                    break;
                }
            } else if matches!(
                op,
                OP_IFFALSE | OP_IFTRUE | OP_IFEQ | OP_IFNE | OP_IFLT | OP_IFLE | OP_IFGT | OP_IFGE | OP_IFSTRICTEQ
                    | OP_IFSTRICTNE | OP_IFNLT | OP_IFNLE | OP_IFNGT | OP_IFNGE
            ) {
                p = rs24(code, p).1;
            } else if op == OP_LOOKUPSWITCH {
                p = rs24(code, p).1;
                let (cc, mut q) = ru30(code, p);
                for _ in 0..=cc {
                    q = rs24(code, q).1;
                }
                p = q;
            } else {
                p = skip_operands(op, code, p);
            }
        }
        if jump_pos < 0 || end_label < 0 {
            return None;
        }
        let true_val = self.eval_branch(code, true_start, jump_pos as usize, stack_copy.clone(), local_names, slot_map, local0_name, is_static, class_idx)?;
        let false_val = self.eval_branch(code, false_label, end_label as usize, stack_copy, local_names, slot_map, local0_name, is_static, class_idx)?;
        Some((true_val, false_val, end_label as usize))
    }

    #[allow(clippy::too_many_arguments)]
    fn eval_branch(
        &self,
        code: &[u8],
        start: usize,
        end: usize,
        stack: Vec<String>,
        local_names: &HashMap<u32, String>,
        slot_map: &HashMap<u32, String>,
        local0_name: &str,
        is_static: bool,
        class_idx: i64,
    ) -> Option<String> {
        let initial_depth = stack.len();
        let mut e = Eval { abc: self.abc, code, stack, local_names, slot_map, local0_name, is_static, class_idx, p: start, bail: false };
        while e.p < end {
            let op = at(code, e.p);
            e.p += 1;
            if !self.eval_dispatch(op, &mut e) {
                return None;
            }
            if e.bail {
                return None;
            }
        }
        if e.stack.len() > initial_depth {
            e.stack.last().cloned()
        } else {
            None
        }
    }

    fn eval_dispatch(&self, op: u8, e: &mut Eval) -> bool {
        match op {
            OP_PUSHBYTE | OP_PUSHSHORT | OP_PUSHSTRING | OP_PUSHINT | OP_PUSHUINT | OP_PUSHDOUBLE | OP_PUSHTRUE
            | OP_PUSHFALSE | OP_PUSHNULL | OP_PUSHUNDEFINED | OP_PUSHNAN | OP_PUSHNAMESPACE => self.eh_push_ops(op, e),
            OP_GETLOCAL_0 | OP_GETLOCAL_1 | OP_GETLOCAL_2 | OP_GETLOCAL_3 | OP_GETLOCAL => {
                match op {
                    OP_GETLOCAL_0 => e.stack.push(e.local_names.get(&0).cloned().unwrap_or_else(|| "this".into())),
                    OP_GETLOCAL_1..=OP_GETLOCAL_3 => {
                        let r = (op - OP_GETLOCAL_0) as u32;
                        e.stack.push(e.local_names.get(&r).cloned().unwrap_or_else(|| format!("_local_{r}")));
                    }
                    _ => {
                        let (idx, np) = ru30(e.code, e.p);
                        e.p = np;
                        e.stack.push(e.local_names.get(&idx).cloned().unwrap_or_else(|| format!("_local_{idx}")));
                    }
                }
            }
            OP_GETPROPERTY | OP_GETLEX | OP_GETSLOT | OP_GETDESCENDANTS => self.eh_property_ops(op, e),
            OP_FINDPROPSTRICT | OP_FINDPROPERTY => {
                let (mn, np) = ru30(e.code, e.p);
                e.p = np;
                if e.abc.mn_needs_rt_name(mn) && !e.stack.is_empty() {
                    e.stack.pop();
                }
                if e.abc.mn_needs_rt_ns(mn) && !e.stack.is_empty() {
                    e.stack.pop();
                }
                e.stack.push(e.abc.mn_name(mn));
            }
            OP_COERCE_A | OP_COERCE_S | OP_CONVERT_S | OP_CONVERT_I | OP_CONVERT_U | OP_CONVERT_D | OP_CONVERT_B
            | OP_CONVERT_O | OP_COERCE | OP_ASTYPE => {
                if op == OP_COERCE {
                    e.p = ru30(e.code, e.p).1;
                } else if op == OP_ASTYPE {
                    let (idx, np) = ru30(e.code, e.p);
                    e.p = np;
                    let tn = if (idx as usize) < e.abc.multinames.len() { e.abc.mn_name(idx) } else { "?".into() };
                    let obj = pop_or(&mut e.stack);
                    e.stack.push(format!("({obj} as {tn})"));
                }
            }
            OP_ADD | OP_SUBTRACT | OP_MULTIPLY | OP_DIVIDE | OP_MODULO | OP_NEGATE | OP_NEGATE_I | OP_NOT | OP_TYPEOF
            | OP_BITOR | OP_BITAND | OP_BITXOR | OP_BITNOT | OP_LSHIFT | OP_RSHIFT | OP_URSHIFT | OP_INCREMENT
            | OP_INCREMENT_I | OP_DECREMENT | OP_DECREMENT_I => self.eh_arithmetic_ops(op, e),
            OP_EQUALS | OP_STRICTEQUALS | OP_LESSTHAN | OP_LESSEQUALS | OP_GREATERTHAN | OP_GREATEREQUALS | OP_IN
            | OP_INSTANCEOF | OP_ISTYPELATE | OP_ASTYPELATE => self.eh_comparison_ops(op, e),
            OP_NEWOBJECT => {
                let (count, np) = ru30(e.code, e.p);
                e.p = np;
                let mut pairs = Vec::new();
                for _ in 0..count {
                    let v = pop_or(&mut e.stack);
                    let k = pop_or(&mut e.stack);
                    pairs.push(format!("{k}:{v}"));
                }
                pairs.reverse();
                e.stack.push(format!("{{{}}}", pairs.join(", ")));
            }
            OP_NEWARRAY => {
                let (count, np) = ru30(e.code, e.p);
                e.p = np;
                let mut items: Vec<String> = Vec::new();
                if !e.stack.is_empty() {
                    for _ in 0..count {
                        items.push(pop_or(&mut e.stack));
                    }
                }
                items.reverse();
                e.stack.push(format!("[{}]", items.join(", ")));
            }
            OP_CALLPROPERTY | OP_CALLPROPLEX | OP_CALLMETHOD | OP_CALLSTATIC | OP_CALLSUPER => self.eh_call_ops(op, e),
            OP_CONSTRUCT | OP_CONSTRUCTPROP | OP_APPLYTYPE => self.eh_construct_ops(op, e),
            OP_DUP => {
                if let Some(t) = e.stack.last().cloned() {
                    e.stack.push(t);
                }
            }
            OP_SWAP => {
                let n = e.stack.len();
                if n >= 2 {
                    e.stack.swap(n - 1, n - 2);
                }
            }
            OP_POP => {
                e.stack.pop();
            }
            OP_IFFALSE | OP_IFTRUE | OP_JUMP | OP_IFEQ | OP_IFNE | OP_IFLT | OP_IFLE | OP_IFGT | OP_IFGE
            | OP_IFSTRICTEQ | OP_IFSTRICTNE | OP_IFNLT | OP_IFNLE | OP_IFNGT | OP_IFNGE => self.eh_branch_ops(op, e),
            OP_PUSHSCOPE | OP_POPSCOPE | OP_PUSHWITH | OP_GETSCOPEOBJECT | OP_GETGLOBALSCOPE | OP_DXNS | OP_DXNSLATE => {
                e.bail = true;
            }
            _ => return false,
        }
        true
    }

    fn eh_push_ops(&self, op: u8, e: &mut Eval) {
        let abc = e.abc;
        match op {
            OP_PUSHBYTE => {
                let mut val = at(e.code, e.p) as i32;
                if val > 127 {
                    val -= 256;
                }
                e.p += 1;
                e.stack.push(val.to_string());
            }
            OP_PUSHSHORT => {
                let (v, np) = ru30(e.code, e.p);
                e.p = np;
                let mut val = v as i64;
                if val >= 0x2000_0000 {
                    val -= 0x4000_0000;
                }
                e.stack.push(val.to_string());
            }
            OP_PUSHSTRING => {
                let (idx, np) = ru30(e.code, e.p);
                e.p = np;
                let s = abc.strings.get(idx as usize).cloned().unwrap_or_else(|| "?".into());
                e.stack.push(format!("\"{}\"", escape_str(&s)));
            }
            OP_PUSHINT => {
                let (idx, np) = ru30(e.code, e.p);
                e.p = np;
                e.stack.push(abc.integers.get(idx as usize).copied().unwrap_or(0).to_string());
            }
            OP_PUSHUINT => {
                let (idx, np) = ru30(e.code, e.p);
                e.p = np;
                e.stack.push(abc.uintegers.get(idx as usize).copied().unwrap_or(0).to_string());
            }
            OP_PUSHDOUBLE => {
                let (idx, np) = ru30(e.code, e.p);
                e.p = np;
                let v = abc.doubles.get(idx as usize).copied().unwrap_or(0.0);
                e.stack.push(Self::fmt_pushdouble(v));
            }
            OP_PUSHTRUE => e.stack.push("true".into()),
            OP_PUSHFALSE => e.stack.push("false".into()),
            OP_PUSHNULL => e.stack.push("null".into()),
            OP_PUSHUNDEFINED => e.stack.push("undefined".into()),
            OP_PUSHNAN => e.stack.push("NaN".into()),
            _ => {} // pushnamespace: no handler action in eval mode (operand not consumed, as in Python)
        }
    }

    fn eh_property_ops(&self, op: u8, e: &mut Eval) {
        let abc = e.abc;
        match op {
            OP_GETPROPERTY => {
                let (mn, np) = ru30(e.code, e.p);
                e.p = np;
                let rt_name = if !e.stack.is_empty() && abc.mn_needs_rt_name(mn) { e.stack.pop() } else { None };
                let _rt_ns = if !e.stack.is_empty() && abc.mn_needs_rt_ns(mn) { e.stack.pop() } else { None };
                let obj = pop_or(&mut e.stack);
                if let Some(rn) = rt_name {
                    e.stack.push(format!("{obj}[{rn}]"));
                } else {
                    let name = abc.mn_name(mn);
                    if obj.is_empty() || obj == "global" || obj == name {
                        e.stack.push(name);
                    } else if obj == "this" {
                        e.stack.push(format!("this.{name}"));
                    } else if obj == e.local0_name && e.is_static {
                        e.stack.push(name);
                    } else {
                        e.stack.push(format!("{obj}.{name}"));
                    }
                }
            }
            OP_GETLEX => {
                let (mn, np) = ru30(e.code, e.p);
                e.p = np;
                e.stack.push(abc.mn_name(mn));
            }
            OP_GETSLOT => {
                let (idx, np) = ru30(e.code, e.p);
                e.p = np;
                let obj = pop_or(&mut e.stack);
                let slot_name = e.slot_map.get(&idx).cloned().unwrap_or_else(|| format!("slot{idx}"));
                if obj.is_empty() || obj == "this" || obj == "global" || obj == e.local0_name {
                    e.stack.push(slot_name);
                } else {
                    e.stack.push(format!("{obj}.{slot_name}"));
                }
            }
            _ => {
                // getdescendants shares the getproperty handler in Python's table
                // (the same `_eh_property_ops`), whose branches ignore it: nothing happens.
            }
        }
    }

    fn eh_arithmetic_ops(&self, op: u8, e: &mut Eval) {
        let stack = &mut e.stack;
        let bin = |stack: &mut Vec<String>, o: &str, hex: bool| {
            let b = pop_or(stack);
            let a = pop_or(stack);
            if hex {
                stack.push(format!("{} {o} {}", to_hex_if_int(&a), to_hex_if_int(&b)));
            } else {
                stack.push(format!("{a} {o} {b}"));
            }
        };
        match op {
            OP_ADD => bin(stack, "+", false),
            OP_SUBTRACT => bin(stack, "-", false),
            OP_MULTIPLY => bin(stack, "*", false),
            OP_DIVIDE => bin(stack, "/", false),
            OP_MODULO => bin(stack, "%", false),
            OP_NEGATE | OP_NEGATE_I => {
                let a = pop_or(stack);
                stack.push(format!("-({a})"));
            }
            OP_NOT => {
                let a = pop_or(stack);
                if let Some(c) = RE_EQ_MATCH.captures(&a) {
                    let negop = if &c[2] == "===" { "!==" } else { "!=" };
                    stack.push(format!("({} {negop} {})", &c[1], &c[3]));
                } else {
                    stack.push(format!("!{a}"));
                }
            }
            OP_TYPEOF => {
                let a = pop_or(stack);
                stack.push(format!("typeof {a}"));
            }
            OP_BITOR => bin(stack, "|", true),
            OP_BITAND => bin(stack, "&", true),
            OP_BITXOR => bin(stack, "^", true),
            OP_BITNOT => {
                let a = pop_or(stack);
                stack.push(format!("(~({}))", to_hex_if_int(&a)));
            }
            OP_LSHIFT => bin(stack, "<<", false),
            OP_RSHIFT => bin(stack, ">>", false),
            OP_URSHIFT => bin(stack, ">>>", false),
            OP_INCREMENT | OP_INCREMENT_I => {
                if let Some(t) = stack.last_mut() {
                    *t = format!("({t} + 1)");
                }
            }
            _ => {
                if let Some(t) = stack.last_mut() {
                    *t = format!("({t} - 1)");
                }
            }
        }
    }

    fn eh_comparison_ops(&self, op: u8, e: &mut Eval) {
        let stack = &mut e.stack;
        let sym = match op {
            OP_EQUALS => Some("=="),
            OP_STRICTEQUALS => Some("==="),
            OP_LESSTHAN => Some("<"),
            OP_LESSEQUALS => Some("<="),
            OP_GREATERTHAN => Some(">"),
            OP_GREATEREQUALS => Some(">="),
            _ => None,
        };
        if let Some(s) = sym {
            let b = pop_or(stack);
            let a = pop_or(stack);
            stack.push(format!("{a} {s} {b}"));
            return;
        }
        let ty = pop_or(stack);
        let obj = pop_or(stack);
        let kw = match op {
            OP_IN => {
                // `in`: (name = pop; obj = pop) -> (obj in name)
                stack.push(format!("({obj} in {ty})"));
                return;
            }
            OP_INSTANCEOF => "instanceof",
            OP_ISTYPELATE => "is",
            _ => "as",
        };
        stack.push(format!("({obj} {kw} {ty})"));
    }

    fn eh_call_ops(&self, op: u8, e: &mut Eval) {
        let abc = e.abc;
        match op {
            OP_CALLPROPERTY | OP_CALLPROPLEX => {
                let (mn, np) = ru30(e.code, e.p);
                let (argc, np) = ru30(e.code, np);
                e.p = np;
                let mut args: Vec<String> = Vec::new();
                if !e.stack.is_empty() {
                    for _ in 0..argc {
                        args.push(pop_or(&mut e.stack));
                    }
                }
                args.reverse();
                let obj = pop_or(&mut e.stack);
                let name = abc.mn_name(mn);
                if obj.is_empty() || obj == "global" {
                    e.stack.push(format!("{name}({})", args.join(", ")));
                } else {
                    e.stack.push(format!("{obj}.{name}({})", args.join(", ")));
                }
            }
            OP_CALLMETHOD => {
                let (method_idx, np) = ru30(e.code, e.p);
                let (argc, np) = ru30(e.code, np);
                e.p = np;
                let mut args: Vec<String> = Vec::new();
                if !e.stack.is_empty() {
                    for _ in 0..argc {
                        args.push(pop_or(&mut e.stack));
                    }
                }
                args.reverse();
                let obj = pop_or(&mut e.stack);
                e.stack.push(format!("callMethod({obj}, {method_idx}, {})", args.join(", ")));
            }
            OP_CALLSTATIC => {
                let (method_idx, np) = ru30(e.code, e.p);
                let (argc, np) = ru30(e.code, np);
                e.p = np;
                let mut args: Vec<String> = Vec::new();
                if !e.stack.is_empty() {
                    for _ in 0..argc {
                        args.push(pop_or(&mut e.stack));
                    }
                }
                args.reverse();
                e.stack.push(format!("callStatic({method_idx}, {})", args.join(", ")));
            }
            _ => {
                let (mn, np) = ru30(e.code, e.p);
                let (argc, np) = ru30(e.code, np);
                e.p = np;
                let mut args: Vec<String> = Vec::new();
                if !e.stack.is_empty() {
                    for _ in 0..argc {
                        args.push(pop_or(&mut e.stack));
                    }
                }
                args.reverse();
                let name = abc.mn_name(mn);
                e.stack.push(format!("super.{name}({})", args.join(", ")));
            }
        }
    }

    fn eh_construct_ops(&self, op: u8, e: &mut Eval) {
        let abc = e.abc;
        match op {
            OP_CONSTRUCT => {
                let (argc, np) = ru30(e.code, e.p);
                e.p = np;
                let mut args: Vec<String> = Vec::new();
                if !e.stack.is_empty() {
                    for _ in 0..argc {
                        args.push(pop_or(&mut e.stack));
                    }
                }
                args.reverse();
                let obj = pop_or(&mut e.stack);
                e.stack.push(format!("new {obj}({})", args.join(", ")));
            }
            OP_CONSTRUCTPROP => {
                let (mn, np) = ru30(e.code, e.p);
                let (argc, np) = ru30(e.code, np);
                e.p = np;
                let mut args: Vec<String> = Vec::new();
                if !e.stack.is_empty() {
                    for _ in 0..argc {
                        args.push(pop_or(&mut e.stack));
                    }
                }
                args.reverse();
                let rt_name = if !e.stack.is_empty() && abc.mn_needs_rt_name(mn) { e.stack.pop() } else { None };
                let _rt_ns = if !e.stack.is_empty() && abc.mn_needs_rt_ns(mn) { e.stack.pop() } else { None };
                let obj = pop_or(&mut e.stack);
                if let Some(rn) = rt_name {
                    e.stack.push(format!("new {obj}[{rn}]({})", args.join(", ")));
                } else {
                    let name = abc.mn_name(mn);
                    if obj.is_empty() || obj == "this" || obj == name {
                        e.stack.push(format!("new {name}({})", args.join(", ")));
                    } else {
                        e.stack.push(format!("new {obj}.{name}({})", args.join(", ")));
                    }
                }
            }
            _ => {
                let (argc, np) = ru30(e.code, e.p);
                e.p = np;
                let mut args: Vec<String> = Vec::new();
                if !e.stack.is_empty() {
                    for _ in 0..argc {
                        args.push(pop_or(&mut e.stack));
                    }
                }
                args.reverse();
                let args: Vec<String> = args.into_iter().map(|a| if a == "null" { "*".into() } else { a }).collect();
                let obj = pop_or(&mut e.stack);
                e.stack.push(format!("{obj}.<{}>", args.join(", ")));
            }
        }
    }

    fn eh_branch_ops(&self, op: u8, e: &mut Eval) {
        let tern = |e: &Eval, p: usize, target: usize| -> Option<Ternary> {
            self.try_ternary(e.code, p, target, e.stack.clone(), e.local_names, e.slot_map, e.local0_name, e.is_static, e.class_idx)
        };
        match op {
            OP_IFFALSE => {
                let (off, p2) = rs24(e.code, e.p);
                let false_target = (p2 as i64 + off as i64) as usize;
                e.p = p2;
                let cond = pop_or(&mut e.stack);
                if let Some((tv, fv, end_pos)) = tern(e, e.p, false_target) {
                    let c = if has_outer_parens(&cond) { cond.clone() } else { format!("({cond})") };
                    let tv = if needs_ternary_wrap(&tv) { format!("({tv})") } else { tv };
                    let fv = if needs_ternary_wrap(&fv) { format!("({fv})") } else { fv };
                    e.stack.push(format!("({c} ? {tv} : {fv})"));
                    e.p = end_pos;
                } else {
                    e.bail = true;
                }
            }
            OP_IFEQ | OP_IFNE | OP_IFLT | OP_IFLE | OP_IFGT | OP_IFGE | OP_IFSTRICTEQ | OP_IFSTRICTNE | OP_IFNLT
            | OP_IFNLE | OP_IFNGT | OP_IFNGE => {
                let (off, p2) = rs24(e.code, e.p);
                let target = p2 as i64 + off as i64;
                e.p = p2;
                let b = pop_or(&mut e.stack);
                let a = pop_or(&mut e.stack);
                let not_cond = match op {
                    OP_IFNGT => Some(">"),
                    OP_IFNLT => Some("<"),
                    OP_IFNLE => Some("<="),
                    OP_IFNGE => Some(">="),
                    _ => None,
                };
                if let (Some(s), true) = (not_cond, target > e.p as i64) {
                    let cond_str = format!("{a} {s} {b}");
                    if let Some((tv, fv, end_pos)) = tern(e, e.p, target as usize) {
                        let c = format!("({cond_str})");
                        let tv = if needs_ternary_wrap(&tv) { format!("({tv})") } else { tv };
                        let fv = if needs_ternary_wrap(&fv) { format!("({fv})") } else { fv };
                        e.stack.push(format!("({c} ? {tv} : {fv})"));
                        e.p = end_pos;
                    } else {
                        e.bail = true;
                    }
                } else {
                    e.bail = true;
                }
            }
            _ => e.bail = true,
        }
    }

    // -------------------------------------------------------- prescans

    fn prescan_branches(code: &[u8], targets: &mut HashSet<usize>) {
        let mut p = 0;
        while p < code.len() {
            let op = code[p];
            p += 1;
            if is_branch_op(op) {
                let (off, np) = rs24(code, p);
                p = np;
                targets.insert((p as i64 + off as i64) as usize);
            } else if op == OP_LOOKUPSWITCH {
                let base = p as i64 - 1;
                let (def, np) = rs24(code, p);
                targets.insert((base + def as i64) as usize);
                let (cc, mut q) = ru30(code, np);
                for _ in 0..=cc {
                    let (o, r) = rs24(code, q);
                    q = r;
                    targets.insert((base + o as i64) as usize);
                }
                p = q;
            } else {
                p = skip_operands(op, code, p);
            }
        }
    }

    fn prescan_local_types(code: &[u8], abc: &AbcFile) -> HashMap<u32, String> {
        let mut local_types: HashMap<u32, String> = HashMap::new();
        let mut branch_targets: HashSet<usize> = HashSet::new();
        Self::prescan_branches(code, &mut branch_targets);

        let mut p = 0;
        let mut last_coerce: Option<String> = None;
        let mut last_push: Option<String> = None;
        let mut last_default = false;
        let mut last_pushnull = false;
        while p < code.len() {
            if branch_targets.contains(&p) {
                last_coerce = None;
                last_push = None;
                last_default = false;
                last_pushnull = false;
            }
            let op = code[p];
            p += 1;
            match op {
                OP_COERCE => {
                    let (mn, np) = ru30(code, p);
                    p = np;
                    last_coerce = if mn != 0 { Some(abc.type_name(mn)) } else { None };
                    if last_pushnull && last_coerce.is_some() {
                        last_default = false;
                        last_pushnull = false;
                    } else if last_default {
                        last_coerce = None;
                    }
                    last_push = None;
                }
                OP_COERCE_I | OP_CONVERT_I => {
                    last_coerce = if !last_default { Some("int".into()) } else { None };
                    last_push = None;
                }
                OP_COERCE_D | OP_CONVERT_D => {
                    last_coerce = if !last_default { Some("Number".into()) } else { None };
                    last_push = None;
                }
                OP_COERCE_U | OP_CONVERT_U => {
                    last_coerce = if !last_default { Some("uint".into()) } else { None };
                    last_push = None;
                }
                OP_COERCE_S | OP_CONVERT_S => {
                    last_coerce = if !last_default { Some("String".into()) } else { None };
                    last_push = None;
                }
                OP_COERCE_B | OP_CONVERT_B => {
                    last_coerce = if !last_default { Some("Boolean".into()) } else { None };
                    last_push = None;
                }
                OP_COERCE_O | OP_CONVERT_O => {
                    last_coerce = if !last_default { Some("Object".into()) } else { None };
                    last_push = None;
                }
                OP_PUSHBYTE | OP_PUSHSHORT | OP_PUSHINT => {
                    last_push = Some("int".into());
                    last_coerce = None;
                    last_default = false;
                    last_pushnull = false;
                    p = skip_operands(op, code, p);
                }
                OP_PUSHUINT => {
                    last_push = Some("uint".into());
                    last_coerce = None;
                    last_default = false;
                    last_pushnull = false;
                    p = skip_operands(op, code, p);
                }
                OP_PUSHDOUBLE => {
                    last_coerce = None;
                    let (idx, _) = ru30(code, p);
                    let v = abc.doubles.get(idx as usize).copied().unwrap_or(0.0);
                    last_default = v == 0.0;
                    last_push = if last_default { None } else { Some("Number".into()) };
                    last_pushnull = false;
                    p = skip_operands(op, code, p);
                }
                OP_PUSHTRUE | OP_PUSHFALSE => {
                    last_push = Some("Boolean".into());
                    last_coerce = None;
                    last_default = false;
                    last_pushnull = false;
                }
                OP_PUSHNULL => {
                    last_default = true;
                    last_pushnull = true;
                    last_coerce = None;
                    last_push = None;
                }
                OP_PUSHNAMESPACE => {
                    last_default = true;
                    last_pushnull = false;
                    last_coerce = None;
                    last_push = None;
                    p = skip_operands(op, code, p);
                }
                OP_SETLOCAL_0..=OP_SETLOCAL_3 | OP_SETLOCAL => {
                    let reg = if op == OP_SETLOCAL {
                        let (idx, np) = ru30(code, p);
                        p = np;
                        idx
                    } else {
                        (op - OP_SETLOCAL_0) as u32
                    };
                    if !local_types.contains_key(&reg) {
                        let detected = last_coerce.clone().or_else(|| last_push.clone());
                        if let Some(d) = detected {
                            local_types.insert(reg, d);
                        } else if last_default {
                            local_types.insert(reg, "*".into());
                        }
                    }
                    last_coerce = None;
                    last_push = None;
                    last_default = false;
                    last_pushnull = false;
                }
                _ => {
                    if is_branch_op(op) {
                        p = rs24(code, p).1;
                    } else if op == OP_LOOKUPSWITCH {
                        p = rs24(code, p).1;
                        let (cc, mut q) = ru30(code, p);
                        for _ in 0..=cc {
                            q = rs24(code, q).1;
                        }
                        p = q;
                    } else {
                        p = skip_operands(op, code, p);
                    }
                    if !matches!(op, OP_DUP | OP_KILL | OP_POP) {
                        last_coerce = None;
                        last_push = None;
                        last_default = false;
                        last_pushnull = false;
                    }
                }
            }
        }
        local_types
    }
}

// ------------------------------------------------------- class decompiler

/// Information about one class for listings.
#[derive(Clone, Debug)]
pub struct ClassListing {
    pub index: usize,
    pub name: String,
    pub package: String,
    pub full_name: String,
    pub super_name: String,
    pub is_interface: bool,
    pub trait_count: usize,
}

/// Class / interface / script source emitter (`AS3Decompiler`).
pub struct As3Decompiler<'a> {
    pub abc: &'a AbcFile,
}

impl<'a> As3Decompiler<'a> {
    pub fn new(abc: &'a AbcFile) -> Self {
        As3Decompiler { abc }
    }

    fn md(&self) -> MethodDecompiler<'a> {
        MethodDecompiler::new(self.abc)
    }

    fn scan_wildcard_imports(abc: &AbcFile, code: &[u8], result: &mut Vec<String>) {
        let mut p = 0;
        while p < code.len() {
            let op = code[p];
            p += 1;
            match op {
                OP_GETSUPER | OP_SETSUPER | OP_GETPROPERTY | OP_SETPROPERTY | OP_INITPROPERTY | OP_DELETEPROPERTY
                | OP_GETDESCENDANTS | OP_FINDPROPSTRICT | OP_FINDPROPERTY | OP_FINDDEF | OP_GETLEX | OP_COERCE
                | OP_ASTYPE | OP_ISTYPE => {
                    let (mn, np) = ru30(code, p);
                    p = np;
                    if matches!(op, OP_COERCE | OP_ASTYPE | OP_ISTYPE | OP_FINDPROPSTRICT | OP_FINDPROPERTY | OP_FINDDEF | OP_GETLEX) {
                        check_mn_ns_set_typed(abc, mn, result);
                    }
                }
                OP_CALLSUPER | OP_CALLPROPERTY | OP_CONSTRUCTPROP | OP_CALLPROPLEX | OP_CALLSUPERVOID | OP_CALLPROPVOID => {
                    let (mn, np) = ru30(code, p);
                    let (_, np) = ru30(code, np);
                    p = np;
                    if op == OP_CONSTRUCTPROP {
                        check_mn_ns_set_typed(abc, mn, result);
                    }
                }
                _ if is_branch_op(op) => p += 3,
                OP_LOOKUPSWITCH => {
                    p += 3;
                    let (cnt, np) = ru30(code, p);
                    p = np + (cnt as usize + 1) * 3;
                }
                _ => p = skip_operands(op, code, p),
            }
        }
    }

    fn scan_body_imports(abc: &AbcFile, code: &[u8], add: &mut dyn FnMut(&str)) {
        let mut p = 0;
        while p < code.len() {
            let op = code[p];
            p += 1;
            match op {
                OP_COERCE | OP_ASTYPE | OP_ISTYPE => {
                    let (mn, np) = ru30(code, p);
                    p = np;
                    if (mn as usize) < abc.multinames.len() {
                        let fqn = abc.mn_full(mn);
                        if fqn.contains('.') && fqn != "*" {
                            add(&fqn);
                        }
                    }
                }
                OP_FINDPROPSTRICT | OP_GETLEX => {
                    let (mn, np) = ru30(code, p);
                    p = np;
                    if (mn as usize) < abc.multinames.len() {
                        let fqn = abc.mn_full(mn);
                        if fqn.contains('.') && fqn != "*" {
                            let fin = fqn.rsplit('.').next().unwrap_or("");
                            if fin.chars().next().map_or(false, |c| c.is_uppercase()) {
                                add(&fqn);
                            }
                        }
                    }
                }
                _ if is_branch_op(op) => p += 3,
                OP_LOOKUPSWITCH => {
                    p += 3;
                    let (cnt, np) = ru30(code, p);
                    p = np + (cnt as usize + 1) * 3;
                }
                _ => p = skip_operands(op, code, p),
            }
        }
    }

    pub fn list_classes(&self) -> Vec<ClassListing> {
        let abc = self.abc;
        abc.instances
            .iter()
            .enumerate()
            .map(|(ci, inst)| {
                let name = abc.mn_name(inst.name_idx);
                let pkg = abc.mn_ns(inst.name_idx);
                ClassListing {
                    index: ci,
                    full_name: if pkg.is_empty() { name.clone() } else { format!("{pkg}.{name}") },
                    super_name: if inst.super_idx != 0 { abc.mn_full(inst.super_idx) } else { String::new() },
                    is_interface: inst.flags & INSTANCE_INTERFACE != 0,
                    trait_count: inst.traits.len() + abc.classes.get(ci).map_or(0, |c| c.traits.len()),
                    name,
                    package: pkg,
                }
            })
            .collect()
    }

    /// Decompile a single class into a `.as` source file. Panics on malformed
    /// indices (caught by [`As3Decompiler::decompile_class`]).
    fn decompile_class_inner(&self, class_idx: usize) -> String {
        let abc = self.abc;
        let Some(inst) = abc.instances.get(class_idx) else { oob() };
        let Some(cls) = abc.classes.get(class_idx) else { oob() };
        let mut md = self.md();
        let ci = class_idx as i64;

        let class_name = abc.mn_name(inst.name_idx);
        let pkg = abc.mn_ns(inst.name_idx);
        let super_full = if inst.super_idx != 0 { abc.mn_full(inst.super_idx) } else { String::new() };
        let super_name = if inst.super_idx != 0 { abc.mn_name(inst.super_idx) } else { String::new() };
        let is_interface = inst.flags & INSTANCE_INTERFACE != 0;
        let is_final = inst.flags & INSTANCE_FINAL != 0;
        let is_sealed = inst.flags & INSTANCE_SEALED != 0;

        let mut imports: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut add_import = |fqn: &str, imports: &mut Vec<String>, seen: &mut HashSet<String>| {
            if fqn.contains('.') && fqn != "*" && !seen.contains(fqn) {
                seen.insert(fqn.to_string());
                imports.push(fqn.to_string());
            }
        };
        let mut wildcard_imports: Vec<String> = Vec::new();
        if !super_full.is_empty() {
            add_import(&super_full, &mut imports, &mut seen);
        }
        for &intf_idx in &inst.interfaces {
            let intf_full = abc.mn_full(intf_idx);
            if intf_full.contains('.') {
                add_import(&intf_full, &mut imports, &mut seen);
            } else {
                let intf_name = abc.mn_name(intf_idx);
                let mut found = false;
                for inst2 in &abc.instances {
                    let cn = abc.mn_name(inst2.name_idx);
                    if cn == intf_name {
                        let cp = abc.mn_ns(inst2.name_idx);
                        if !cp.is_empty() {
                            add_import(&format!("{cp}.{cn}"), &mut imports, &mut seen);
                            found = true;
                            break;
                        }
                    }
                }
                if !found {
                    let Some(m) = abc.multinames.get(intf_idx as usize) else { oob() };
                    if matches!(m.kind, MN_MULTINAME | MN_MULTINAME_A) && m.data.len() >= 2 {
                        let Some(ns_set) = abc.ns_sets.get(m.data[1] as usize) else { oob() };
                        for &ns_idx in ns_set {
                            let ns_nm = abc.ns_name(ns_idx);
                            if ns_nm.chars().next().map_or(false, |c| c.is_lowercase())
                                && !ns_nm.contains(':')
                                && ns_nm != "http://adobe.com/AS3/2006/builtin"
                            {
                                add_import(&format!("{ns_nm}.*"), &mut imports, &mut seen);
                            }
                        }
                    }
                }
            }
        }
        for init_mi in [inst.iinit, cls.cinit] {
            if let Some(m) = abc.methods.get(init_mi as usize) {
                for &pt in &m.param_types {
                    if pt != 0 {
                        add_import(&abc.mn_full(pt), &mut imports, &mut seen);
                    }
                }
                if m.return_type != 0 {
                    add_import(&abc.mn_full(m.return_type), &mut imports, &mut seen);
                }
            }
        }
        for t in inst.traits.iter().chain(cls.traits.iter()) {
            if matches!(t.kind, TRAIT_SLOT | TRAIT_CONST) && t.type_name != 0 {
                add_import(&abc.mn_full(t.type_name), &mut imports, &mut seen);
            } else if matches!(t.kind, TRAIT_METHOD | TRAIT_GETTER | TRAIT_SETTER | TRAIT_FUNCTION) {
                if let Some(m) = abc.methods.get(t.method_idx as usize) {
                    for &pt in &m.param_types {
                        if pt != 0 {
                            add_import(&abc.mn_full(pt), &mut imports, &mut seen);
                        }
                    }
                    if m.return_type != 0 {
                        add_import(&abc.mn_full(m.return_type), &mut imports, &mut seen);
                    }
                }
            }
        }
        let mut method_indices: Vec<u32> = vec![inst.iinit, cls.cinit];
        for t in inst.traits.iter().chain(cls.traits.iter()) {
            if matches!(t.kind, TRAIT_METHOD | TRAIT_GETTER | TRAIT_SETTER | TRAIT_FUNCTION) {
                method_indices.push(t.method_idx);
            }
        }
        let mut scanned: HashSet<u32> = method_indices.iter().copied().collect();
        let mut closure_indices: Vec<u32> = Vec::new();
        let mut queue: std::collections::VecDeque<u32> = method_indices.iter().copied().collect();
        while let Some(mi) = queue.pop_front() {
            if let Some(body) = abc.method_bodies.get(&mi) {
                let code = &body.code;
                let mut p2 = 0;
                while p2 < code.len() {
                    let op2 = code[p2];
                    p2 += 1;
                    if op2 == OP_NEWFUNCTION {
                        let (child, np) = ru30(code, p2);
                        p2 = np;
                        if !scanned.contains(&child) {
                            scanned.insert(child);
                            closure_indices.push(child);
                            queue.push_back(child);
                        }
                    } else {
                        p2 = skip_operands(op2, code, p2);
                    }
                }
            }
        }
        for &mi in &closure_indices {
            if let Some(m) = abc.methods.get(mi as usize) {
                for &pt in &m.param_types {
                    if pt != 0 {
                        add_import(&abc.mn_full(pt), &mut imports, &mut seen);
                    }
                }
                if m.return_type != 0 {
                    add_import(&abc.mn_full(m.return_type), &mut imports, &mut seen);
                }
            }
        }
        let all_method_indices: Vec<u32> = method_indices.iter().chain(closure_indices.iter()).copied().collect();
        for mi in &all_method_indices {
            if let Some(body) = abc.method_bodies.get(mi) {
                Self::scan_wildcard_imports(abc, &body.code, &mut wildcard_imports);
                let mut adder = |f: &str| add_import(f, &mut imports, &mut seen);
                Self::scan_body_imports(abc, &body.code, &mut adder);
            }
        }
        let imports: Vec<String> = imports
            .into_iter()
            .filter(|imp| !imp.starts_with(&format!("{pkg}.")) || imp.matches('.').count() > pkg.matches('.').count() + 1)
            .filter(|imp| !imp.contains(':'))
            .filter(|imp| !imp.starts_with("__AS3__"))
            .collect();

        let mut lines: Vec<String> = Vec::new();
        lines.push(if pkg.is_empty() { "package".into() } else { format!("package {pkg}") });
        lines.push("{".into());

        const WILD_PRIORITY: [&str; 6] = ["flash.display", "flash.geom", "flash.events", "flash.media", "flash.filters", "flash.utils"];
        let mut wild_pkgs: Vec<String> = Vec::new();
        for w in &wildcard_imports {
            if !w.is_empty() && *w != pkg && !w.starts_with("__AS3__") && !wild_pkgs.contains(w) {
                wild_pkgs.push(w.clone());
            }
        }
        let mut explicit_simple: HashMap<String, String> = HashMap::new();
        for imp in &imports {
            if let Some((pk, cn)) = imp.rsplit_once('.') {
                explicit_simple.insert(cn.to_string(), pk.to_string());
            }
        }
        let mut pkg_classes: HashMap<String, HashSet<String>> = HashMap::new();
        for inst2 in &abc.instances {
            let fqn = abc.mn_full(inst2.name_idx);
            if let Some((cp, cn)) = fqn.rsplit_once('.') {
                pkg_classes.entry(cp.to_string()).or_default().insert(cn.to_string());
            }
        }
        for m in &abc.multinames {
            if matches!(m.kind, MN_QNAME | MN_QNAME_A) && m.data.len() >= 2 {
                let (ns_idx, name_idx) = (m.data[0], m.data[1]);
                if (ns_idx as usize) < abc.namespaces.len() && (name_idx as usize) < abc.strings.len() {
                    if abc.ns_kind(ns_idx) == NS_PACKAGE {
                        let ns_nm = abc.ns_name(ns_idx);
                        let cn = abc.strings[name_idx as usize].clone();
                        if !ns_nm.is_empty() && cn.chars().next().map_or(false, |c| c.is_uppercase()) {
                            pkg_classes.entry(ns_nm).or_default().insert(cn);
                        }
                    }
                }
            }
        }
        let wild_pkgs: Vec<String> = wild_pkgs
            .into_iter()
            .filter(|w| {
                let empty = HashSet::new();
                let classes = pkg_classes.get(w).unwrap_or(&empty);
                !classes.iter().any(|cn| explicit_simple.get(cn).map_or(false, |p| p != w))
            })
            .collect();
        let priority: Vec<&String> = WILD_PRIORITY.iter().filter_map(|w| wild_pkgs.iter().find(|x| x.as_str() == *w)).collect();
        let rest: Vec<&String> = wild_pkgs.iter().filter(|w| !WILD_PRIORITY.contains(&w.as_str())).collect();
        let wild_list: Vec<String> = priority.into_iter().chain(rest).map(|w| format!("{w}.*")).collect();
        for imp in &imports {
            lines.push(format!("    import {imp};"));
        }
        for imp in &wild_list {
            lines.push(format!("    import {imp};"));
        }
        if !imports.is_empty() || !wild_list.is_empty() {
            lines.push(String::new());
        }

        let mut decl: Vec<String> = vec!["    public".into()];
        if is_final {
            decl.push("final".into());
        }
        if !is_sealed && !is_interface {
            decl.push("dynamic".into());
        }
        decl.push(if is_interface { "interface".into() } else { "class".into() });
        decl.push(class_name.clone());
        if !super_name.is_empty() && super_name != "Object" && super_name != "*" {
            decl.push(format!("extends {super_name}"));
        }
        if !inst.interfaces.is_empty() {
            let names: Vec<String> = inst.interfaces.iter().map(|&ii| abc.mn_name(ii)).collect();
            let kw = if is_interface { "extends" } else { "implements" };
            decl.push(format!("{kw} {}", names.join(", ")));
        }
        lines.push(decl.join(" "));
        lines.push("    {".into());
        lines.push(String::new());

        let i2 = INDENT_UNIT.repeat(2);
        let i3 = INDENT_UNIT.repeat(3);
        let mut static_vars: Vec<&TraitInfo> = cls.traits.iter().filter(|t| matches!(t.kind, TRAIT_SLOT | TRAIT_CONST)).collect();
        let static_methods: Vec<&TraitInfo> =
            cls.traits.iter().filter(|t| matches!(t.kind, TRAIT_METHOD | TRAIT_GETTER | TRAIT_SETTER | TRAIT_FUNCTION)).collect();
        static_vars.sort_by_key(|t| if t.kind == TRAIT_CONST { 0 } else { 1 }); // stable
        let mut last_kind: Option<u8> = None;
        for t in &static_vars {
            if last_kind == Some(TRAIT_CONST) && t.kind != TRAIT_CONST {
                lines.push(String::new());
            }
            lines.push(self.decompile_var_trait(t, true));
            last_kind = Some(t.kind);
        }

        let mut cinit_block_stmts: Vec<String> = Vec::new();
        if !is_interface {
            let cinit_src = md.decompile(cls.cinit, "", ci, true, &class_name);
            let raw: Vec<String> = cinit_src.split('\n').map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
            let mut cinit_stmts: Vec<String> = Vec::new();
            let mut accum = String::new();
            let mut brace_depth: i64 = 0;
            for raw_line in raw {
                if !accum.is_empty() {
                    accum.push('\n');
                    accum.push_str(&raw_line);
                } else {
                    accum = raw_line.clone();
                }
                brace_depth += raw_line.matches('{').count() as i64 - raw_line.matches('}').count() as i64;
                if brace_depth <= 0 {
                    cinit_stmts.push(std::mem::take(&mut accum));
                    brace_depth = 0;
                }
            }
            if !accum.is_empty() {
                cinit_stmts.push(accum);
            }
            let var_names: HashSet<String> = static_vars.iter().map(|t| abc.mn_name(t.name_idx)).collect();
            for stmt in cinit_stmts {
                let mut matched_var = false;
                // Python iterates a set (arbitrary order); a statement can only start
                // with one `name = ` prefix, so any order finds the same name.
                for vn in &var_names {
                    if stmt.starts_with(&format!("{vn} = ")) && stmt.ends_with(';') {
                        let init_val = stmt[vn.len() + 3..stmt.len() - 1].to_string();
                        for idx in 0..lines.len() {
                            if lines[idx].contains(&format!(" {vn}:")) && lines[idx].trim().ends_with(';') {
                                if !lines[idx].contains(" = ") {
                                    if init_val.contains('\n') {
                                        let base_indent = lines[idx].chars().count() - lines[idx].trim_start_matches(' ').chars().count();
                                        let decl_prefix = format!("{} = ", &lines[idx][..lines[idx].len() - 1]);
                                        let val_lines: Vec<&str> = init_val.split('\n').collect();
                                        let mut expanded = vec![format!("{decl_prefix}{}", val_lines[0])];
                                        for vl in &val_lines[1..] {
                                            expanded.push(format!("{}{vl}", " ".repeat(base_indent + INDENT_UNIT.len())));
                                        }
                                        let last = expanded.last().unwrap().trim().to_string();
                                        *expanded.last_mut().unwrap() = format!("{}{last}", " ".repeat(base_indent));
                                        expanded.last_mut().unwrap().push(';');
                                        lines.splice(idx..idx + 1, expanded);
                                    } else {
                                        let l = lines[idx].clone();
                                        lines[idx] = format!("{} = {init_val};", &l[..l.len() - 1]);
                                    }
                                }
                                matched_var = true;
                                break;
                            }
                        }
                        break;
                    }
                }
                if !matched_var {
                    cinit_block_stmts.push(stmt);
                }
            }
        }

        let inst_vars: Vec<&TraitInfo> = inst.traits.iter().filter(|t| matches!(t.kind, TRAIT_SLOT | TRAIT_CONST)).collect();
        let inst_methods: Vec<&TraitInfo> =
            inst.traits.iter().filter(|t| matches!(t.kind, TRAIT_METHOD | TRAIT_GETTER | TRAIT_SETTER | TRAIT_FUNCTION)).collect();
        if !static_vars.is_empty() && !inst_vars.is_empty() {
            lines.push(String::new());
        }
        for t in &inst_vars {
            lines.push(self.decompile_var_trait(t, false));
        }
        let mut ctor_src: Option<String> = None;
        if !is_interface {
            ctor_src = self.decompile_constructor(inst, &class_name, ci);
        }
        let has_ctor = ctor_src.is_some();
        if !static_vars.is_empty() || !inst_vars.is_empty() {
            lines.push(String::new());
        }
        if !static_vars.is_empty() && !inst_vars.is_empty() && !has_ctor {
            lines.push(String::new());
        }
        if !cinit_block_stmts.is_empty() {
            lines.push(format!("{i2}{{"));
            for stmt in &cinit_block_stmts {
                lines.push(format!("{i3}{stmt}"));
            }
            lines.push(format!("{i2}}}"));
            lines.push(String::new());
            lines.push(String::new());
        }
        if let Some(c) = ctor_src {
            lines.push(c);
            lines.push(String::new());
        }
        if !has_ctor
            && cinit_block_stmts.is_empty()
            && (!static_vars.is_empty() || !inst_vars.is_empty())
            && !(!static_vars.is_empty() && !inst_vars.is_empty())
            && (!inst_methods.is_empty() || !static_methods.is_empty())
        {
            lines.push(String::new());
        }
        for t in &static_methods {
            lines.push(self.decompile_method_trait(t, true, is_interface, ci, &class_name));
            lines.push(String::new());
        }
        if !static_methods.is_empty() && !inst_methods.is_empty() {
            lines.push(String::new());
        }
        for t in &inst_methods {
            lines.push(self.decompile_method_trait(t, false, is_interface, ci, &class_name));
            lines.push(String::new());
        }
        let has_methods = !inst_methods.is_empty() || !static_methods.is_empty();
        if has_methods || !has_ctor {
            lines.push(String::new());
        }
        lines.push(format!("{INDENT_UNIT}}}"));
        lines.push("}".into());

        let fs = self.emit_file_scope_classes(class_idx);
        if !fs.is_empty() {
            lines.push(String::new());
            lines.push(fs);
            lines.push(String::new());
        }
        lines.join("\n") + "\n\n"
    }

    /// Decompile a class; `Err` when the class data is malformed.
    pub fn decompile_class(&self, class_idx: usize) -> Result<String, String> {
        catch(|| self.decompile_class_inner(class_idx))
    }

    fn emit_file_scope_classes(&self, main: usize) -> String {
        let abc = self.abc;
        let mut script: Option<&ScriptInfo> = None;
        'o: for si in &abc.scripts {
            for t in &si.traits {
                if t.kind == TRAIT_CLASS && t.class_idx as usize == main {
                    script = Some(si);
                    break 'o;
                }
            }
        }
        let Some(script) = script else { return String::new() };
        let siblings: Vec<usize> =
            script.traits.iter().filter(|t| t.kind == TRAIT_CLASS && t.class_idx as usize != main).map(|t| t.class_idx as usize).collect();
        if siblings.is_empty() {
            return String::new();
        }
        let mut parts = Vec::new();
        for sci in siblings {
            let Some(sinst) = abc.instances.get(sci) else { oob() };
            let sname = abc.mn_name(sinst.name_idx);
            let mut lines = vec![format!("class {sname}"), "{".to_string(), String::new()];
            for tr in &sinst.traits {
                if matches!(tr.kind, TRAIT_SLOT | TRAIT_CONST) {
                    let var_line = self.decompile_var_trait(tr, false);
                    lines.push(format!("{INDENT_UNIT}{}", var_line.trim_start()));
                }
            }
            lines.push(String::new());
            lines.push(String::new());
            lines.push("}".into());
            parts.push(lines.join("\n"));
        }
        parts.join("\n")
    }

    fn decompile_var_trait(&self, trait_: &TraitInfo, is_static: bool) -> String {
        let abc = self.abc;
        let name = abc.mn_name(trait_.name_idx);
        let type_str = if trait_.type_name != 0 { abc.type_name(trait_.type_name) } else { "*".into() };
        let ns_kind = abc.mn_ns_kind(trait_.name_idx);
        let access = access_modifier(ns_kind);
        let kw = if trait_.kind == TRAIT_CONST { "const" } else { "var" };
        let prefix = if is_static { "static " } else { "" };
        let mut default = String::new();
        if trait_.vindex != 0 {
            let mut val_str = abc.default_value_str(trait_.vkind, trait_.vindex);
            if trait_.vkind == CONSTANT_INT && type_str == "int" {
                let ival = abc.integers.get(trait_.vindex as usize).copied().unwrap_or(0);
                if ival >= 256 {
                    val_str = fmt_hex_const(ival as u128);
                }
            } else if trait_.vkind == CONSTANT_UINT && type_str == "uint" {
                let uval = abc.uintegers.get(trait_.vindex as usize).copied().unwrap_or(0);
                if uval >= 256 {
                    val_str = fmt_hex_const(uval as u128);
                }
            }
            if type_str == "Number" && RE_NEG_INT.is_match(&val_str) {
                val_str.push_str(".0");
            }
            default = format!(" = {val_str}");
        }
        format!("{}{access} {prefix}{kw} {name}:{type_str}{default};", INDENT_UNIT.repeat(2))
    }

    fn decompile_constructor(&self, inst: &InstanceInfo, class_name: &str, class_idx: i64) -> Option<String> {
        let abc = self.abc;
        let mi = inst.iinit;
        let m = abc.methods.get(mi as usize);
        let params_str = m.map_or(String::new(), |m| self.format_params(m));
        let ret_type = m.map_or("void".to_string(), |m| abc.type_name(m.return_type));
        let ret_suffix = if !ret_type.is_empty() && ret_type != "*" { format!(":{ret_type}") } else { String::new() };
        let mut md = self.md();
        let i3 = INDENT_UNIT.repeat(3);
        let i2 = INDENT_UNIT.repeat(2);
        let body_src = md.decompile(mi, &i3, class_idx, false, class_name);
        let super_name = if inst.super_idx != 0 { abc.mn_name(inst.super_idx) } else { String::new() };
        let trimmed_end = body_src.trim_end();
        let mut body_lines: Vec<String> =
            if !body_src.trim().is_empty() { trimmed_end.split('\n').map(|s| s.to_string()).collect() } else { vec![] };
        if let Some(fr) = body_lines.iter().position(|l| !l.trim().is_empty()) {
            if body_lines[fr].trim() == "super();" && (super_name.is_empty() || super_name == "Object" || super_name == "*") {
                body_lines.remove(fr);
            }
        }
        let body_src = body_lines.join("\n");
        if params_str.is_empty() && body_src.trim().is_empty() {
            return None;
        }
        let mut lines = vec![format!("{i2}public function {class_name}({params_str}){ret_suffix}"), format!("{i2}{{")];
        if !body_src.trim().is_empty() {
            lines.push(body_src);
        }
        lines.push(format!("{i2}}}"));
        Some(lines.join("\n"))
    }

    fn decompile_method_trait(&self, trait_: &TraitInfo, is_static: bool, is_interface: bool, class_idx: i64, class_name: &str) -> String {
        let abc = self.abc;
        let name = abc.mn_name(trait_.name_idx);
        let mi = trait_.method_idx;
        let m = abc.methods.get(mi as usize);
        let ns_kind = abc.mn_ns_kind(trait_.name_idx);
        let access = access_modifier(ns_kind);
        let mut prefix_parts: Vec<String> = Vec::new();
        if trait_.attr & 0x01 != 0 && !is_static {
            prefix_parts.push("final".into());
        }
        if trait_.attr & 0x02 != 0 {
            prefix_parts.push("override".into());
        }
        if !is_interface {
            prefix_parts.push(access.into());
        }
        if is_static {
            prefix_parts.push("static".into());
        }
        prefix_parts.push(match trait_.kind {
            TRAIT_GETTER => "function get".into(),
            TRAIT_SETTER => "function set".into(),
            _ => "function".into(),
        });
        let params_str = m.map_or(String::new(), |m| self.format_params(m));
        let mut ret_type = m.map_or("*".to_string(), |m| abc.type_name(m.return_type));
        if trait_.kind == TRAIT_SETTER {
            ret_type = "void".into();
        }
        let i2 = INDENT_UNIT.repeat(2);
        let sig = format!("{i2}{} {name}({params_str}):{ret_type}", prefix_parts.join(" "));
        if is_interface {
            return format!("{sig};");
        }
        let mut lines = vec![sig, format!("{i2}{{")];
        let mut md = self.md();
        let body_src = md.decompile(mi, &INDENT_UNIT.repeat(3), class_idx, is_static, class_name);
        if !body_src.trim().is_empty() {
            lines.push(body_src.trim_end().to_string());
        }
        lines.push(format!("{i2}}}"));
        lines.join("\n")
    }

    pub fn format_params(&self, m: &MethodInfo) -> String {
        let abc = self.abc;
        let mut params = Vec::new();
        let num_required = m.param_count as i64 - m.optional_values.len() as i64;
        for i in 0..m.param_count as i64 {
            let mut pname = String::new();
            if (i as usize) < m.param_names.len() {
                let pn = m.param_names[i as usize] as usize;
                if pn < abc.strings.len() {
                    pname = abc.strings[pn].clone();
                }
            }
            if pname.is_empty() {
                pname = format!("_arg_{}", i + 1);
            }
            let ptype = if (i as usize) < m.param_types.len() && m.param_types[i as usize] != 0 {
                abc.type_name(m.param_types[i as usize])
            } else {
                "*".into()
            };
            let mut s = format!("{pname}:{ptype}");
            if i >= num_required {
                let oi = (i - num_required) as usize;
                if oi < m.optional_values.len() {
                    let (vk, vi) = m.optional_values[oi];
                    s.push_str(&format!("={}", abc.default_value_str(vk, vi)));
                }
            }
            params.push(s);
        }
        if m.flags & METHOD_NEED_REST != 0 {
            params.push("...rest".into());
        }
        params.join(", ")
    }

    /// Script-level members: `(index, sinit, functions[(name, method)], variables[(name, type, kind)], class_count)`.
    pub fn list_scripts(&self) -> Vec<ScriptListing> {
        let abc = self.abc;
        abc.scripts
            .iter()
            .enumerate()
            .map(|(si_idx, si)| {
                let mut s = ScriptListing { index: si_idx, sinit: si.sinit, functions: vec![], variables: vec![], class_count: 0 };
                for t in &si.traits {
                    let name = abc.mn_name(t.name_idx);
                    match t.kind {
                        TRAIT_CLASS => s.class_count += 1,
                        TRAIT_FUNCTION | TRAIT_METHOD | TRAIT_GETTER | TRAIT_SETTER => s.functions.push((name, t.method_idx)),
                        TRAIT_SLOT | TRAIT_CONST => {
                            let ty = if t.type_name != 0 { abc.type_name(t.type_name) } else { "*".into() };
                            s.variables.push((name, ty, if t.kind == TRAIT_CONST { "const" } else { "var" }.to_string()));
                        }
                        _ => {}
                    }
                }
                s
            })
            .collect()
    }

    pub fn decompile_script(&self, script_idx: usize) -> Result<String, String> {
        catch(|| self.decompile_script_inner(script_idx))?
    }

    fn decompile_script_inner(&self, script_idx: usize) -> Result<String, String> {
        let abc = self.abc;
        let Some(si) = abc.scripts.get(script_idx) else {
            return Err(format!("Script index {script_idx} out of range (0..{})", abc.scripts.len() as i64 - 1));
        };
        let mut md = self.md();
        let mut lines: Vec<String> = Vec::new();
        for t in &si.traits {
            if matches!(t.kind, TRAIT_SLOT | TRAIT_CONST) {
                let name = abc.mn_name(t.name_idx);
                let ts = if t.type_name != 0 { abc.type_name(t.type_name) } else { "*".into() };
                let kw = if t.kind == TRAIT_CONST { "const" } else { "var" };
                if t.vindex != 0 {
                    lines.push(format!("{kw} {name}:{ts} = {};", abc.default_value_str(t.vkind, t.vindex)));
                } else {
                    lines.push(format!("{kw} {name}:{ts};"));
                }
            }
        }
        for t in &si.traits {
            if matches!(t.kind, TRAIT_FUNCTION | TRAIT_METHOD | TRAIT_GETTER | TRAIT_SETTER) {
                let name = abc.mn_name(t.name_idx);
                let mi = t.method_idx;
                let m = abc.methods.get(mi as usize);
                let params_str = m.map_or(String::new(), |m| self.format_params(m));
                let mut ret = m.map_or("*".to_string(), |m| abc.type_name(m.return_type));
                let mut prefix = "function";
                if t.kind == TRAIT_GETTER {
                    prefix = "function get";
                } else if t.kind == TRAIT_SETTER {
                    prefix = "function set";
                    ret = "void".into();
                }
                lines.push(String::new());
                lines.push(format!("{prefix} {name}({params_str}):{ret}"));
                lines.push("{".into());
                let body = md.decompile(mi, INDENT_UNIT, -1, false, "");
                if !body.trim().is_empty() {
                    lines.push(body.trim_end().to_string());
                }
                lines.push("}".into());
            }
        }
        let sinit_body = md.decompile(si.sinit, INDENT_UNIT, -1, false, "");
        let stripped = sinit_body.trim();
        if !stripped.is_empty() && stripped != "return;" {
            lines.push(String::new());
            lines.push("// script initializer".into());
            lines.push("{".into());
            lines.push(sinit_body.trim_end().to_string());
            lines.push("}".into());
        }
        Ok(if lines.is_empty() { String::new() } else { lines.join("\n") + "\n" })
    }

    /// Decompile a single method body (class context optional).
    pub fn decompile_method(&self, method_idx: u32, class_idx: i64, is_static: bool, class_name: &str) -> String {
        let mut md = self.md();
        md.decompile(method_idx, INDENT_UNIT, class_idx, is_static, class_name)
    }
}

#[derive(Clone, Debug)]
pub struct ScriptListing {
    pub index: usize,
    pub sinit: u32,
    pub functions: Vec<(String, u32)>,
    pub variables: Vec<(String, String, String)>,
    pub class_count: usize,
}
