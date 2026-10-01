"""Negative control: prove the shape tests can actually fail.

Injects each of the bugs that were fixed, one at a time, and confirms the suite
notices. A test that cannot fail is not a test, and the only way to know whether
these can fail is to break the code on purpose and check.

This exists because the first attempt at this found something uncomfortable: four
of the bugs it was written to detect were *not* caught, because every test case
used coordinates far enough from the threshold that the two comparison
predicates agreed. The near-threshold cases now in xfl_shape_tests.cpp are there
because of that.

Patterns match on a normalised form, and the diff for each is printed, so a
substitution that does not do what its name claims is visible rather than silent
-- which is how four of the first eight attempts turned into no-ops.

It builds tnzcore, so it needs the application build. It leaves the source tree
exactly as it found it, including on failure.

    python tests/native/mutation_check.py
"""
import difflib
import io
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
BUILD = os.environ.get("FLARE_BUILD",
                       os.path.join(REPO, "build_local"))
SHAPE = os.path.join(REPO, "flare", "sources", "common", "flash",
                     "XFLShape.cpp")
EXE = os.path.join(HERE, "build", "RelWithDebInfo", "xfl_shape_tests.exe")

# Qt and the Flare runtime both have to be reachable, or the test process cannot
# start at all -- silently, with no output and a nonzero exit.
QT_BIN = os.environ.get("QT_BIN", "")

ENV = dict(os.environ)
_parts = [QT_BIN, os.path.join(BUILD, "RelWithDebInfo"),
          os.path.join(REPO, "LocalTest"), ENV.get("PATH", "")]
ENV["PATH"] = os.pathsep.join([p for p in _parts if p])
if QT_BIN:
    ENV["QT_PLUGIN_PATH"] = os.path.join(QT_BIN, "..", "plugins")


def norm(text):
    return re.sub(r"[ \t]+", " ", text.replace("\r\n", "\n"))


# (name, regex, replacement). Each reintroduces a bug that was fixed, and each
# is one of the ways this format is commonly misread.
MUTATIONS = [
    ("restatement test uses qFuzzyCompare, which is relative",
     r"!cur\.points\.isEmpty\(\) && qAbs\(at\.x\(\) - dest\.x\(\)\) < kPointEpsilon &&\s*qAbs\(at\.y\(\) - dest\.y\(\)\) < kPointEpsilon",
     "!cur.points.isEmpty() && qFuzzyCompare(at.x(), dest.x()) &&\n"
     "                qFuzzyCompare(at.y(), dest.y())"),

    ("a restated moveTo records a point again",
     r"if \(!restates\) cur\.points\.append\(at\);",
     "cur.points.append(at);"),

    ("a number in an opcode slot is read as a lineTo",
     r"if \(t\.op == Op::Number\) \{",
     "if (false) {"),

    # '/' is a lineTo, byte-identical in meaning to '|'. Treating it as a
    # subpath break is the single most common misreading of this format, and it
    # produces a differently-shaped path rather than an error -- which is why the
    # tokenizer normalises it here. An earlier version of this control matched
    # the restatement predicate instead, so two controls hit one site and
    # nothing exercised this one.
    ("'/' normalised to a subpath break instead of a lineTo",
     r"case '!':\n(\s*)case '\|':\n\s*case '/':\n",
     "case '!':\n\\1case '|':\n"),

    ("the comparison epsilon is finer than the coordinate quantum",
     r"constexpr double kPointEpsilon = 1e-3;",
     "constexpr double kPointEpsilon = 1e-9;"),

    ("a quadratic's control point is emitted as its endpoint",
     r"d \+= QLatin1Char\('Q'\) \+ num\(ctrl\.x\(\)\)",
     "d += QLatin1Char('Q') + num(e.x())"),

    ("coordinates not divided from twips to pixels",
     r"raw\.append\(Raw\{'n', toks\.at\(ti \+ k\)\.value / kTwipsPerPixel\}\);",
     "raw.append(Raw{'n', toks.at(ti + k).value});"),

    # A straight segment consumes one point and a quadratic two. Recording the
    # wrong kind desynchronises the point stream from the segment list.
    ("a straight segment recorded as a quadratic",
     r"cur\.segments\.append\(Contour::Kind::Line\);",
     "cur.segments.append(Contour::Kind::Quad);"),
]


def build():
    """Returns (ok, detail).

    A build failure must not be mistaken for a weak test: without this, a
    mutation that did not compile left the previous binary in place, the suite
    passed, and the result was recorded as MISSED -- indistinguishable from the
    suite not being able to detect that particular bug.
    """
    for cwd, args in ((BUILD, ["--target", "tnzcore", "-j", "8"]),
                      (os.path.join(HERE, "build"), ["-j", "8"])):
        r = subprocess.run(["cmake", "--build", cwd if False else cwd,
                            "--config", "RelWithDebInfo"] + args,
                           cwd=cwd, capture_output=True, text=True)
        if r.returncode:
            tail = [l for l in (r.stdout + r.stderr).splitlines()
                    if "error" in l.lower()][:3]
            return False, "; ".join(tail) or "build failed"
    return True, ""


def run():
    r = subprocess.run([EXE], capture_output=True, text=True, env=ENV)
    return r.returncode, r.stderr


def clip(text, n=84):
    return text.strip()[:n]


def main():
    if not os.path.isfile(EXE):
        print(f"   {EXE} not built. Run run_tests.py first, or build "
              f"tests/native.")
        return 1
    if not QT_BIN:
        print("   note: QT_BIN is not set, so the test process may not start at "
              "all.\n         Set it to <qt>/5.x/<kit>/bin.")

    print("baseline: the suite must pass on unmodified sources")
    ok, detail = build()
    if not ok:
        print(f"  BASELINE DOES NOT BUILD: {detail}")
        return 1
    rc, out = run()
    if rc != 0:
        print("  BASELINE FAILS -- results below would be meaningless")
        print(out[-2000:])
        return 1
    print("  [ok  ] baseline passes")

    results = []
    for name, pattern, repl in MUTATIONS:
        original = io.open(SHAPE, encoding="utf-8", newline="").read()
        ntext = norm(original)
        new, count = re.subn(pattern, repl, ntext)
        if count != 1:
            results.append((name, "SKIPPED", f"pattern matched {count} times"))
            print(f"  [skip] {name}: pattern matched {count} times")
            continue

        diff = [l for l in difflib.unified_diff(ntext.splitlines(),
                                                 new.splitlines(),
                                                 lineterm="", n=0)
                if l.startswith(("+", "-")) and not l.startswith(("+++", "---"))]
        if not diff:
            results.append((name, "SKIPPED", "substitution was a no-op"))
            print(f"  [skip] {name}: substitution was a no-op")
            continue

        io.open(SHAPE, "w", encoding="utf-8", newline="").write(new)
        try:
            ok, detail = build()
            if not ok:
                # Report the real cause. Without this, a mutation that fails to
                # compile reads exactly like a mutation the suite cannot see.
                results.append((name, "NO-BUILD", detail))
                print(f"  [skip] {name}: the mutation did not compile -- {detail[:70]}")
                continue
            rc, out = run()
            caught = rc != 0
            m = re.search(r"\[FAIL\] (.+)", out)
            results.append((name, "CAUGHT" if caught else "MISSED",
                            clip(m.group(1)) if m else ""))
            print(f"  [{'ok  ' if caught else 'MISS'}] {name}")
            # diff holds only +/- lines, but a substitution can add lines
            # without removing any (as with the '/' control), so index it
            # defensively rather than assuming the first entry is a removal.
            for line in diff[:2]:
                print(f"          inject:    {clip(line, 86)}")
            if m:
                print(f"          caught by: {clip(m.group(1), 86)}")
        finally:
            io.open(SHAPE, "w", encoding="utf-8", newline="").write(original)

    print()
    ok, _ = build()
    if not ok:
        print("restored: the baseline does not build -- something is wrong")
        return 1
    rc, _ = run()
    print(f"restored: baseline {'passes' if rc == 0 else 'FAILS'}")

    bad = [r for r in results if r[1] != "CAUGHT"]
    print()
    if bad:
        print(f"{len(bad)} of {len(results)} mutations were not conclusively "
              f"caught:")
        for r in bad:
            print(f"   [{r[1]}] {r[0]}  ({r[2]})")
        return 1
    print(f"PASSED: all {len(results)} mutations were caught by the suite")
    return 0


if __name__ == "__main__":
    sys.exit(main())
