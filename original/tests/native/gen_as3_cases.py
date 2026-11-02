"""Re-derive the recorded golden of the native AS3/ABC core (common/flash
ac_as3_{abc,decompile,patch}.rs) from the ORIGINAL Python implementation.

The Python original is Next2Flash's `app/lib/as3_decompiler` (MIT). It is NOT
vendored in this repository any more; point NEXT2FLASH_DIR at a checkout of
https://github.com/SSF2-Mods-Official/Next2Flash (default
/home/user/ssf2/Next2Flash). The script runs the original on

  * real ABC blocks (tests/flash_fixtures/as3/*.swf, from the Ruffle `swf`
    crate test-suite, MIT/Apache-2.0), and
  * synthetic ABC files built with the original's own ABCEditor/Assembler:
    hand-written classes plus a seeded generator of random structured methods
    (if/else, loops, for, for-each, switch, try/catch, ternary, &&/||,
    closures, increments ...) so the method decompiler's structuring passes are
    all exercised,

and records classes' sources, per-method decompiles and disassembly, ABC
serialize round trips, class transplants, string patches, difflib opcodes and
float formatting into flare/sources/common/flash/as3_cases.json, which
`ac_as3_tests.rs` replays.

Usage: python tests/native/gen_as3_cases.py [--check]
  --check   do not write; exit 1 when the committed golden differs.
Not part of the test run; the golden is committed.
"""
import json
import os
import random
import struct
import sys
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
GOLDEN = os.path.join(REPO, "flare", "sources", "common", "flash", "as3_cases.json")
FIXDIR = os.path.join(REPO, "tests", "flash_fixtures", "as3")
N2F = os.environ.get("NEXT2FLASH_DIR", "/home/user/ssf2/Next2Flash")

sys.path.insert(0, os.path.join(N2F, "app", "lib"))
try:
    import as3_decompiler as P
    from as3_decompiler import abc_editor, abc_patcher, swf_patcher, abc_parser
    from as3_decompiler.method_decompiler import MethodDecompiler
except Exception as e:  # pragma: no cover
    sys.exit(f"cannot import the Python oracle from {N2F}: {e}")

ABCFile = P.ABCFile


def hx(b):
    return bytes(b).hex()


# ---------------------------------------------------------------- builders

EMPTY_ABC = struct.pack("<HH", 16, 46) + bytes([0] * 12)  # 7 pools + methods/meta/inst/scripts/bodies = 12 zero counts? see below


def empty_abc():
    b = bytearray(struct.pack("<HH", 16, 46))
    # ints, uints, doubles, strings, ns, nssets, multinames
    b += bytes([0] * 7)
    # methods, metadata, instances(+classes), scripts, bodies
    b += bytes([0] * 5)
    return bytes(b)


class Asm(abc_editor.Assembler):
    def label_positions(self):
        pos = 0
        out = {}
        for item in self._items:
            if item[0] == "label":
                out[item[1]] = pos
                continue
            _, op, ops, fmt = item
            size = 1
            for d, v in ops:
                size += 3 if isinstance(v, str) else abc_patcher._operand_byte_size(d, v)
            pos += size
        return out


class Builder:
    def __init__(self, seed=0):
        self.ed = abc_editor.ABCEditor(empty_abc())
        self.abc = self.ed.abc
        self.rng = random.Random(seed)

    def mn(self, name, ns=""):
        return self.ed.ensure_qname_by_name(ns, name)

    def S(self, s):
        return self.ed.ensure_string(s)

    def method(self, name, ptypes=(), ret=0, code=None, pnames=None, locals_=10, exceptions=None, flags=0, opt=None, traits=None):
        return self.ed.add_method(
            return_type=ret, param_types=list(ptypes), name=name, flags=flags,
            optional_values=opt, param_names=pnames, code=code, max_stack=8,
            local_count=locals_, init_scope_depth=1, max_scope_depth=4,
            exceptions=exceptions or [], traits=traits or [])

    def add_class(self, pkg, name, sup="Object", traits=(), ctraits=(), iface=False, ifaces=(), iinit_code=None, flags=1):
        ed = self.ed
        name_mn = self.mn(name, pkg)
        sup_mn = 0 if iface else (self.mn(sup.split(".")[-1], ".".join(sup.split(".")[:-1])) if sup else 0)
        iinit = self.method("", code=iinit_code)
        cinit = self.method("")
        inst = abc_parser.InstanceInfo()
        inst.name_idx = name_mn
        inst.super_idx = sup_mn
        inst.flags = (4 if iface else 0) | flags
        inst.interfaces = [self.mn(i.split(".")[-1], ".".join(i.split(".")[:-1])) for i in ifaces]
        inst.iinit = iinit
        inst.traits = list(traits)
        ci = abc_parser.ClassInfo()
        ci.cinit = cinit
        ci.traits = list(ctraits)
        self.abc.instances.append(inst)
        self.abc.classes.append(ci)
        idx = len(self.abc.classes) - 1
        sinit = self.method("")
        si = abc_parser.ScriptInfo()
        si.sinit = sinit
        si.traits = [ed.make_class_trait(name_mn, idx)]
        self.abc.scripts.append(si)
        return idx

    def data(self):
        return self.ed.serialize()


# ------------------------------------------------------ random method generator

STRS = ["hello", "", "a\nb", 'say "hi"', "tab\there", "back\\slash", "é中", "x", "trace me", "0", "1.5"]
PROPS = ["alpha", "beta", "gamma", "delta", "count", "name"]
FUNCS = ["foo", "bar", "baz", "compute", "trace", "parseInt"]


class Gen:
    def __init__(self, b, rng):
        self.b = b
        self.r = rng
        self.lab = 0
        self.a = None
        self.nparams = 0
        self.excs = []
        self.inner = []

    def L(self):
        self.lab += 1
        return f"L{self.lab}"

    def reg(self):
        return self.r.randint(1, 8)

    def expr(self, d):
        r, a, b = self.r, self.a, self.b
        c = r.random()
        if d <= 0 or c < 0.28:
            k = r.randint(0, 13)
            if k == 0:
                a.emit("pushbyte", r.randint(0, 127))
            elif k == 1:
                a.emit("pushbyte", r.randint(128, 255))
            elif k == 2:
                a.emit("pushshort", r.choice([200, 1000, 40000, 536870911, 0x20000000 + 5]))
            elif k == 3:
                a.emit("pushint", b.ed.ensure_int(r.choice([0, 1, -1, 255, 256, 65535, 100000, -70000, 2147483647])))
            elif k == 4:
                a.emit("pushuint", b.ed.ensure_uint(r.choice([0, 255, 256, 65536, 4294967295])))
            elif k == 5:
                a.emit("pushstring", b.S(r.choice(STRS)))
            elif k == 6:
                a.emit("pushdouble", b.ed.ensure_double(r.choice([0.0, 1.0, 2.5, 255.0, 256.0, 4096.0, 1e20, 0.1, -3.75, 123456789.125, 1e-7, 4294967296.0])))
            elif k == 7:
                a.emit(r.choice(["pushtrue", "pushfalse", "pushnull", "pushundefined", "pushnan"]))
            elif k == 8:
                a.emit(f"getlocal_{r.randint(0, 3)}")
            elif k == 9:
                a.emit("getlocal", self.reg())
            elif k == 10:
                a.emit("getlocal_0")
                a.emit("getproperty", b.mn(r.choice(PROPS)))
            elif k == 11:
                a.emit("getlex", b.mn(r.choice(["Math", "Main", "Vector", "Array", "String"])))
            elif k == 12:
                a.emit("getlocal_0")
                a.emit("getslot", r.randint(1, 3))
            else:
                a.emit("pushbyte", r.randint(0, 9))
            return
        if c < 0.52:
            self.expr(d - 1)
            self.expr(d - 1)
            op = r.choice(["add", "subtract", "multiply", "divide", "modulo", "lshift", "rshift", "urshift", "bitand", "bitor", "bitxor", "add_i", "subtract_i", "multiply_i"])
            a.emit(op)
            if r.random() < 0.2:
                a.emit(r.choice(["convert_i", "convert_u", "convert_d", "coerce_a"]))
        elif c < 0.64:
            self.expr(d - 1)
            self.expr(d - 1)
            a.emit(r.choice(["equals", "strictequals", "lessthan", "lessequals", "greaterthan", "greaterequals", "in", "instanceof"]))
        elif c < 0.70:
            self.expr(d - 1)
            a.emit(r.choice(["not", "negate", "increment", "decrement", "typeof", "bitnot", "convert_s", "convert_b", "coerce_s", "coerce_a", "negate_i", "increment_i", "decrement_i"]))
        elif c < 0.78:
            nm = r.choice(FUNCS)
            a.emit("findpropstrict", b.mn(nm))
            n = r.randint(0, 2)
            for _ in range(n):
                self.expr(d - 1)
            a.emit("callproperty", b.mn(nm), n)
        elif c < 0.82:
            self.expr(d - 1)
            n = r.randint(0, 2)
            for _ in range(n):
                self.expr(d - 1)
            a.emit("callproperty", b.mn(r.choice(FUNCS)), n)
        elif c < 0.86:
            n = r.randint(0, 3)
            for _ in range(n):
                self.expr(d - 1)
            a.emit("newarray", n)
        elif c < 0.89:
            n = r.randint(1, 3)
            for i in range(n):
                a.emit("pushstring", b.S(f"k{i}"))
                self.expr(d - 1)
            a.emit("newobject", n)
        elif c < 0.93:
            lf, le = self.L(), self.L()
            self.expr(d - 1)
            a.emit(r.choice(["iffalse", "iftrue"]), lf)
            self.expr(d - 1)
            a.emit("jump", le)
            a.label(lf)
            self.expr(d - 1)
            a.label(le)
        elif c < 0.97:
            l = self.L()
            self.expr(d - 1)
            a.emit("dup")
            a.emit(r.choice(["iffalse", "iftrue"]), l)
            a.emit("pop")
            self.expr(d - 1)
            a.label(l)
        elif c < 0.985:
            a.emit("findpropstrict", b.mn("Foo"))
            n = r.randint(0, 2)
            for _ in range(n):
                self.expr(d - 1)
            a.emit("constructprop", b.mn("Foo"), n)
        else:
            self.expr(d - 1)
            a.emit("coerce", b.mn(r.choice(["Sprite", "int", "String"])))

    def cond(self, d):
        r, a = self.r, self.a
        if r.random() < 0.5:
            self.expr(d)
            self.expr(d)
            a.emit(r.choice(["equals", "strictequals", "lessthan", "lessequals", "greaterthan", "greaterequals"]))
        else:
            self.expr(d)

    def block(self, d, n):
        for _ in range(n):
            self.stmt(d)

    def stmt(self, d):
        r, a, b = self.r, self.a, self.b
        c = r.random()
        if d <= 0 or c < 0.30:
            k = r.randint(0, 6)
            if k == 0 or k == 1:
                self.expr(2)
                if r.random() < 0.4:
                    a.emit(r.choice(["coerce_s", "convert_i", "coerce_d", "coerce_a", "convert_u", "convert_b"]))
                a.emit(r.choice(["setlocal", "setlocal"]), self.reg())
            elif k == 2:
                a.emit("getlocal_0")
                self.expr(2)
                a.emit("setproperty", b.mn(r.choice(PROPS)))
            elif k == 3:
                a.emit("findpropstrict", b.mn("trace"))
                self.expr(2)
                a.emit("callpropvoid", b.mn("trace"), 1)
            elif k == 4:
                reg = self.reg()
                a.emit(r.choice(["inclocal", "declocal", "inclocal_i"]), reg)
            elif k == 5:
                reg = r.randint(1, 3)
                if r.random() < 0.5:
                    a.emit(f"getlocal_{reg}")
                    a.emit("dup")
                    a.emit(r.choice(["increment", "decrement"]))
                    a.emit(f"setlocal_{reg}")
                else:
                    a.emit(f"getlocal_{reg}")
                    a.emit(r.choice(["increment", "decrement"]))
                    a.emit("dup")
                    a.emit(f"setlocal_{reg}")
                a.emit("pop")
            else:
                a.emit("getlocal_0")
                self.expr(1)
                a.emit("callpropvoid", b.mn(r.choice(FUNCS)), 1)
            return
        if c < 0.46:
            lelse, lend = self.L(), self.L()
            self.cond(1)
            a.emit(r.choice(["iffalse", "iftrue" if False else "iffalse"]), lelse)
            self.block(d - 1, r.randint(1, 2))
            if r.random() < 0.5:
                a.emit("jump", lend)
                a.label(lelse)
                self.block(d - 1, r.randint(1, 2))
                a.label(lend)
            else:
                a.label(lelse)
        elif c < 0.56:
            lc, lb = self.L(), self.L()
            a.emit("jump", lc)
            a.label(lb)
            a.emit("label")
            self.block(d - 1, r.randint(1, 2))
            a.label(lc)
            self.cond(1)
            a.emit("iftrue", lb)
        elif c < 0.62:
            lb = self.L()
            a.label(lb)
            a.emit("label")
            self.block(d - 1, r.randint(1, 2))
            self.cond(1)
            a.emit("iftrue", lb)
        elif c < 0.72:
            reg = self.reg()
            lc, lb = self.L(), self.L()
            a.emit("pushbyte", 0)
            a.emit("setlocal", reg)
            a.emit("jump", lc)
            a.label(lb)
            a.emit("label")
            self.block(d - 1, r.randint(1, 2))
            a.emit("getlocal", reg)
            a.emit("increment")
            a.emit("convert_i")
            a.emit("setlocal", reg)
            a.label(lc)
            a.emit("getlocal", reg)
            a.emit("pushbyte", r.randint(2, 20))
            a.emit(r.choice(["iflt", "ifle", "ifnlt"]), lb)
        elif c < 0.78:
            obj, idx, v = 6, 7, 8
            lc, lb = self.L(), self.L()
            self.expr(1)
            a.emit("setlocal", obj)
            a.emit("pushbyte", 0)
            a.emit("setlocal", idx)
            a.emit("jump", lc)
            a.label(lb)
            a.emit("label")
            a.emit("getlocal", obj)
            a.emit("getlocal", idx)
            a.emit(r.choice(["nextvalue", "nextname"]))
            a.emit("setlocal", v)
            self.block(d - 1, 1)
            a.label(lc)
            a.emit("hasnext2", obj, idx)
            a.emit("iftrue", lb)
        elif c < 0.86:
            n = r.randint(2, 3)
            t = 8
            lend, lchain, lsw, ldef = self.L(), self.L(), self.L(), self.L()
            cases = [self.L() for _ in range(n)]
            a.emit("jump", lchain)
            for cl in cases:
                a.label(cl)
                a.emit("label")
                self.block(d - 1, 1)
                if r.random() < 0.8:
                    a.emit("jump", lend)
            a.label(ldef)
            a.emit("label")
            self.block(d - 1, 1)
            a.emit("jump", lend)
            a.label(lchain)
            a.emit("getlocal", self.reg())
            a.emit("setlocal", t)
            nexts = [self.L() for _ in range(n)]
            for i in range(n):
                a.emit("pushbyte", i + 1)
                a.emit("getlocal", t)
                a.emit("ifstrictne", nexts[i])
                a.emit("pushbyte", i)
                a.emit("jump", lsw)
                a.label(nexts[i])
            a.emit("pushbyte", n)
            a.label(lsw)
            a.emit("kill", t)
            a.emit("lookupswitch", ldef, cases + [ldef])
            a.label(lend)
        elif c < 0.93:
            ltry, ltryend, lcatch, lafter = self.L(), self.L(), self.L(), self.L()
            a.label(ltry)
            self.block(d - 1, r.randint(1, 2))
            a.label(ltryend)
            a.emit("jump", lafter)
            a.label(lcatch)
            a.emit("label")
            a.emit("pop")
            self.block(d - 1, 1)
            a.label(lafter)
            self.excs.append((ltry, ltryend, lcatch, r.choice([0, b.mn("Error"), b.mn("TypeError")]), b.mn("e")))
        elif c < 0.96:
            nm = r.choice(FUNCS)
            a.emit("findpropstrict", b.mn(nm))
            self.expr(1)
            a.emit("callpropvoid", b.mn(nm), 1)
        else:
            # closure
            mi = self.make_closure(d - 1)
            a.emit("newfunction", mi)
            a.emit("setlocal", self.reg())

    def make_closure(self, d):
        g = Gen(self.b, self.r)
        g.nparams = self.r.randint(0, 2)
        code, excs = g.build_body(d, closure=True)
        mi = self.b.method("", ptypes=[0] * g.nparams, code=code[0], exceptions=code[1], pnames=[f"c{i}" for i in range(g.nparams)] if g.nparams else None)
        return mi

    def build_body(self, depth, closure=False):
        a = self.a = Asm()
        a.emit("getlocal_0")
        a.emit("pushscope")
        self.block(depth, self.r.randint(2, 5))
        if self.r.random() < 0.6:
            self.expr(2)
            a.emit("returnvalue")
        else:
            a.emit("returnvoid")
        a.label("__end")
        a.emit("returnvoid") if False else None
        code = a.assemble()
        pos = a.label_positions()
        exc = [self.b.ed.make_exception(pos[f], pos[t], pos[c], et, vn) for f, t, c, et, vn in self.excs]
        return (code, exc), exc


def fuzz_class(seed, nmeth=14, depth=3):
    b = Builder(seed)
    rng = random.Random(seed * 7919 + 1)
    traits = []
    obj = b.mn("Object")
    for i in range(nmeth):
        g = Gen(b, rng)
        g.nparams = rng.randint(0, 3)
        (code, exc), _ = g.build_body(depth)
        pnames = [f"p{j}" for j in range(g.nparams)] if rng.random() < 0.7 else None
        mi = b.method(f"m{i}", ptypes=[b.mn(rng.choice(["int", "String", "Object", "Number"])) if rng.random() < 0.5 else 0 for _ in range(g.nparams)],
                      ret=b.mn("void") if rng.random() < 0.3 else 0, code=code, exceptions=exc, pnames=pnames)
        traits.append(b.ed.make_method_trait(b.mn(f"m{i}"), mi))
    for j, nm in enumerate(PROPS[:3]):
        traits.append(b.ed.make_slot_trait(b.mn(nm), b.mn("int"), slot_id=j + 1))
    idx = b.add_class("", "Fuzz", traits=traits)
    return b, idx


def hand_class():
    """Hand-written classes: imports, interface, statics, getters/setters, optional params, rest."""
    b = Builder(1)
    ed = b.ed
    # interface
    im = b.method("run", ptypes=[b.mn("int")], ret=b.mn("void"), pnames=["speed"])
    i_run = ed.make_method_trait(b.mn("run", ""), im)
    b.add_class("com.test", "IRunner", iface=True, traits=[i_run])
    # main class
    a = Asm()
    a.emit("getlocal_0")
    a.emit("pushscope")
    a.emit("getlocal_0")
    a.emit("constructsuper", 0)
    a.emit("getlocal_0")
    a.emit("pushstring", b.S("start"))
    a.emit("setproperty", b.mn("label", ""))
    a.emit("findpropstrict", b.mn("trace"))
    a.emit("pushstring", b.S("hello world"))
    a.emit("callpropvoid", b.mn("trace"), 1)
    a.emit("returnvoid")
    ctor = b.method("", ptypes=[b.mn("String")], code=a.assemble(), pnames=["title"],
                    opt=[(0x01, b.S("dflt"))], flags=8)
    getter_code = Asm()
    getter_code.emit("getlocal_0")
    getter_code.emit("pushscope")
    getter_code.emit("getlocal_0")
    getter_code.emit("getproperty", b.mn("label", ""))
    getter_code.emit("returnvalue")
    gm = b.method("title", ret=b.mn("String"), code=getter_code.assemble())
    sm = Asm()
    sm.emit("getlocal_0")
    sm.emit("pushscope")
    sm.emit("getlocal_0")
    sm.emit("getlocal_1")
    sm.emit("setproperty", b.mn("label", ""))
    sm.emit("returnvoid")
    sm_i = b.method("title", ptypes=[b.mn("String")], ret=b.mn("void"), code=sm.assemble(), pnames=["v"])
    rest = b.method("sum", ptypes=[], ret=b.mn("Number"), flags=4, code=bytes([0xD0, 0x30, 0x24, 0x00, 0x48]))
    stat = Asm()
    stat.emit("getlocal_0")
    stat.emit("pushscope")
    stat.emit("findpropstrict", b.mn("trace"))
    stat.emit("pushdouble", ed.ensure_double(3.0))
    stat.emit("callpropvoid", b.mn("trace"), 1)
    stat.emit("returnvoid")
    sm2 = b.method("boot", ret=b.mn("void"), code=stat.assemble())
    traits = [
        ed.make_slot_trait(b.mn("label", ""), b.mn("String"), slot_id=1, vindex=b.S("none"), vkind=1),
        ed.make_slot_trait(b.mn("hits", ""), b.mn("int"), slot_id=2, vindex=ed.ensure_int(300), vkind=3),
        ed.make_getter_trait(b.mn("title"), gm),
        ed.make_setter_trait(b.mn("title"), sm_i),
        ed.make_method_trait(b.mn("sum"), rest, attr=2),
    ]
    ctraits = [
        ed.make_slot_trait(b.mn("MAX", ""), b.mn("int"), slot_id=1, vindex=ed.ensure_int(1000), vkind=3, is_const=True),
        ed.make_slot_trait(b.mn("ratio", ""), b.mn("Number"), slot_id=2, vindex=ed.ensure_double(2.0), vkind=6),
        ed.make_method_trait(b.mn("boot"), sm2),
    ]
    idx = b.add_class("com.test", "Main", sup="flash.display.Sprite", ifaces=["com.test.IRunner"], traits=traits, ctraits=ctraits,
                      iinit_code=None)
    b.abc.instances[idx].iinit = ctor
    b.ed.add_metadata("SWF", [("width", "100")])
    return b


def variant(strs, extra=False):
    """Original/compiled pair for transplant: same layout, edited constants, optional new method + slot."""
    b = Builder(5)
    ed = b.ed
    def mcode(s, n):
        a = Asm()
        a.emit("getlocal_0")
        a.emit("pushscope")
        a.emit("findpropstrict", b.mn("trace"))
        a.emit("pushstring", b.S(s))
        a.emit("callpropvoid", b.mn("trace"), 1)
        a.emit("findpropstrict", b.mn("trace"))
        a.emit("pushint", ed.ensure_int(n))
        a.emit("callpropvoid", b.mn("trace"), 1)
        a.emit("returnvoid")
        return a.assemble()
    m0 = b.method("a", code=mcode(strs[0], 1000), ret=b.mn("void"))
    m1 = b.method("b", code=mcode(strs[1], 2000), ret=b.mn("void"))
    traits = [ed.make_method_trait(b.mn("a"), m0), ed.make_method_trait(b.mn("b"), m1),
              ed.make_slot_trait(b.mn("x"), b.mn("int"), slot_id=1)]
    if extra:
        m2 = b.method("c", code=mcode("brand new", 3), ret=b.mn("void"))
        traits.append(ed.make_method_trait(b.mn("c"), m2))
        traits.append(ed.make_slot_trait(b.mn("y"), b.mn("String"), slot_id=2))
    b.add_class("p", "T", traits=traits)
    return b.data()


# ---------------------------------------------------------------- recorders

def record_abc(name, data, methods=True):
    abc = ABCFile(data)
    dec = P.AS3Decompiler(abc)
    md = MethodDecompiler(abc)
    case = {"name": name, "abc": hx(data), "roundtrip": abc_patcher.serialize_abc(abc) == data, "classes": [], "methods": {}, "scripts": []}
    case["list"] = dec.list_classes()
    for ci in range(len(abc.instances)):
        try:
            case["classes"].append({"index": ci, "source": dec.decompile_class(ci)})
        except Exception as e:
            case["classes"].append({"index": ci, "error": type(e).__name__})
    # per-method decompile + disassembly (class context from traits)
    ctx = {}
    for ci, (inst, cls) in enumerate(zip(abc.instances, abc.classes)):
        cname = abc.mn_name(inst.name_idx)
        ctx[inst.iinit] = (ci, False, cname)
        ctx[cls.cinit] = (ci, True, cname)
        for t in inst.traits:
            if t.kind in (1, 2, 3, 5):
                ctx[t.method_idx] = (ci, False, cname)
        for t in cls.traits:
            if t.kind in (1, 2, 3, 5):
                ctx[t.method_idx] = (ci, True, cname)
    for mi in (sorted(abc.method_bodies) if methods else []):
        ci, st, cname = ctx.get(mi, (-1, False, ""))
        try:
            text = md.decompile(mi, "    ", ci, st, cname)
        except Exception as e:
            continue
        if "decompile error" in text:
            continue
        case["methods"][str(mi)] = {"ctx": [ci, st, cname], "src": text,
                                    "dis": abc_editor.disassemble(abc, abc.method_bodies[mi].code)}
    for si in range(len(abc.scripts)):
        try:
            case["scripts"].append({"index": si, "source": dec.decompile_script(si)})
        except Exception as e:
            case["scripts"].append({"index": si, "error": type(e).__name__})
    case["scripts_list"] = dec.list_scripts()
    return case


def swf_with_abc(abc_data, name="frame1", compressed=False):
    def tag(t, body):
        if len(body) < 63:
            return struct.pack("<H", (t << 6) | len(body)) + body
        return struct.pack("<HI", (t << 6) | 63, len(body)) + body
    body = bytes([0x78, 0x00, 0x05, 0x5F, 0x00, 0x00, 0x0F, 0xA0, 0x00])  # RECT 550x400
    body += struct.pack("<HH", 0x1800, 1)
    body += tag(9, bytes([1, 2, 3]))  # SetBackgroundColor
    body += tag(82, struct.pack("<I", 1) + name.encode() + b"\0" + abc_data)
    body += tag(1, b"")
    body += tag(0, b"")
    total = 8 + len(body)
    if compressed:
        return b"CWS" + bytes([10]) + struct.pack("<I", total) + zlib.compress(body)
    return b"FWS" + bytes([10]) + struct.pack("<I", total) + body


def py_patch(swf_path, mapping):
    """The bridge's cmd_patch loop, returning logical tags."""
    swf = swf_patcher.read_swf_full(swf_path)
    tags = swf["tags"]
    replaced = blocks = 0
    for i, (tt, body) in enumerate(tags):
        if tt not in (82, 72):
            continue
        if tt == 82:
            if len(body) < 5:
                continue
            nul = body.find(b"\x00", 4)
            if nul < 0:
                continue
            prefix, abc_data = body[:nul + 1], body[nul + 1:]
        else:
            prefix, abc_data = b"", body
        ed = abc_editor.ABCEditor(abc_data)
        st = ed.abc.strings
        hits = 0
        for idx in range(1, len(st)):
            nv = mapping.get(st[idx])
            if nv is not None and nv != st[idx]:
                st[idx] = nv
                hits += 1
        if not hits:
            continue
        tags[i] = (tt, prefix + abc_patcher.serialize_abc(ed.abc))
        replaced += hits
        blocks += 1
    return replaced, blocks, [(t, hx(b)) for t, b in tags]


def main():
    check = "--check" in sys.argv
    out = {"cases": [], "transplants": [], "swfpatch": [], "diffs": [], "floats": [], "texts": []}
    # real fixtures
    if os.path.isdir(FIXDIR):
        for fn in sorted(os.listdir(FIXDIR)):
            if fn.endswith(".swf"):
                _, blocks = P.read_abc_blocks(os.path.join(FIXDIR, fn))
                for i, (nm, data) in enumerate(blocks):
                    out["cases"].append(record_abc(f"fixture:{fn}:{i}", data))
    out["cases"].append(record_abc("hand", hand_class().data()))
    for seed in range(1, 25):
        b, _ = fuzz_class(seed)
        out["cases"].append(record_abc(f"fuzz{seed}", b.data(), methods=seed <= 4))

    # transplant
    orig = variant(["one", "two"])
    comp_s = variant(["ONE", "two"])
    comp_x = variant(["one", "two"], extra=True)
    comp_xs = variant(["uno", "dos"], extra=True)
    for label, comp, changed, edits in [
        ("strings-full", comp_s, None, None),
        ("strings-changed", comp_s, {"method:a"}, None),
        ("strings-known", comp_s, {"method:a"}, {"one": "ONE"}),
        ("extra-full", comp_x, None, None),
        ("extra-changed", comp_xs, {"method:a", "method:c"}, None),
        ("both-full", comp_xs, None, {"one": "uno", "two": "dos"}),
    ]:
        try:
            res = abc_patcher.transplant_class(orig, comp, "p.T", changed_methods=set(changed) if changed is not None else None,
                                               source_string_edits=edits)
            out["transplants"].append({"name": label, "orig": hx(orig), "compiled": hx(comp), "class": "p.T",
                                       "changed": sorted(changed) if changed is not None else None,
                                       "edits": [[k, v] for k, v in edits.items()] if edits else None, "result": hx(res)})
        except Exception as e:
            out["transplants"].append({"name": label, "orig": hx(orig), "compiled": hx(comp), "class": "p.T",
                                       "changed": sorted(changed) if changed is not None else None,
                                       "edits": [[k, v] for k, v in edits.items()] if edits else None, "error": True})

    # string patch through a SWF container
    import tempfile
    b, _ = fuzz_class(3)
    data = b.data()
    for comp in (False, True):
        swf = swf_with_abc(data, compressed=comp)
        with tempfile.NamedTemporaryFile(suffix=".swf", delete=False) as f:
            f.write(swf)
            path = f.name
        mapping = {"hello": "HELLO", "x": "y", "nothing-here": "z"}
        rep, blk, tags = py_patch(path, mapping)
        os.unlink(path)
        out["swfpatch"].append({"swf": hx(swf), "mapping": mapping, "replaced": rep, "blocks": blk, "tags": tags})

    # difflib + source helpers
    rng = random.Random(99)
    for _ in range(120):
        n = rng.randint(0, 12)
        m = rng.randint(0, 12)
        alpha = "abcdef"
        a = [rng.choice(alpha) for _ in range(n)]
        bb = [rng.choice(alpha) for _ in range(m)]
        import difflib
        ops = difflib.SequenceMatcher(None, a, bb, autojunk=False).get_opcodes()
        out["diffs"].append({"a": a, "b": bb, "ops": [list(o) for o in ops]})
    srcs = [c["classes"][0]["source"] for c in out["cases"] if c["name"] in ("hand", "fuzz3", "fuzz7") and c["classes"] and "source" in c["classes"][0]]
    for s in srcs:
        e = s.replace('"hello world"', '"bye world"').replace('"start"', '"go"').replace("return;", "return;")
        e = e.replace('"hello"', '"HELLO"')
        cn = "Main" if "class Main" in s else "Fuzz"
        out["texts"].append({
            "orig": s, "edited": e, "class": cn,
            "methods": abc_patcher.extract_method_texts(s, cn),
            "changed": sorted(abc_patcher._detect_changed_methods(s, e, cn)),
            "string_edits": [[k, v] for k, v in abc_patcher._extract_string_edits_from_source(s, e).items()],
        })

    for v in [0.0, 1.0, 2.5, 255.0, 1e20, 0.1, -3.75, 123456789.125, 1e-7, 4294967296.0, 1e15, 1e16, 123456789012345678.0, 5e-324, 1.7976931348623157e308, 0.30000000000000004, 100.0, 1e22, 12345.678901234567, -0.0003]:
        out["floats"].append({"bits": struct.unpack("<Q", struct.pack("<d", v))[0], "repr": repr(v), "g15": f"{v:.15g}"})

    text = json.dumps(out, separators=(',', ':'), sort_keys=True, ensure_ascii=True) + "\n"
    if check:
        cur = open(GOLDEN).read() if os.path.exists(GOLDEN) else ""
        if cur != text:
            print("as3_cases.json differs from a rebuild")
            sys.exit(1)
        print("as3_cases.json up to date")
        return
    with open(GOLDEN, "w") as f:
        f.write(text)
    nm = sum(len(c["methods"]) for c in out["cases"])
    print(f"wrote {GOLDEN}: {len(out['cases'])} abc cases, {nm} methods, {len(out['transplants'])} transplants")


if __name__ == "__main__":
    main()
