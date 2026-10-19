"""Re-derive the recorded goldens in cornerdetector.rs and casmfileinfo.rs.

The originals (cornerdetector.cpp, casmfileinfo.cpp) are deleted, so they are
rebuilt from git history (the commit before the retirement, REF below) with g++
and run on LCG-driven inputs; the output is spliced between the
`// BEGIN GOLDEN` / `// END GOLDEN` markers in the Rust test modules.
Cases that hit C++ undefined behaviour (trapped via _GLIBCXX_ASSERTIONS in a
forked child) are dropped by the generator.

Usage: python tests/native/gen_cornerdetector_casm_cases.py [--check]
Needs g++ and POSIX fork. Not part of the test run; the goldens are committed.
"""
import os, re, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
SRC = os.path.join(REPO, "flare", "sources")
REF = os.environ.get("PORT_C_REF", "7f5de0b^")


def git_show(path):
    return subprocess.check_output(["git", "-C", REPO, "show", f"{REF}:{path}"])


def run(cmd, **kw):
    return subprocess.check_output(cmd, **kw).decode()


def splice(rs, body, check):
    s = open(rs).read()
    pat = re.compile(r"(// BEGIN GOLDEN\n).*?(    // END GOLDEN)", re.S)
    new = pat.sub(lambda m: m.group(1) + body + m.group(2), s)
    if check:
        return new == s
    open(rs, "w").write(new)
    return True


def main():
    check = "--check" in sys.argv
    ok = True
    with tempfile.TemporaryDirectory() as t:
        # --- cornerdetector
        for n in ("cornerdetector.cpp", "cornerdetector.h"):
            open(os.path.join(t, n), "wb").write(git_show(f"flare/sources/common/tvectorimage/{n}"))
        exe = os.path.join(t, "gen")
        run(["g++", "-std=c++17", "-O1", "-DLINUX", "-D_GLIBCXX_ASSERTIONS", f"-I{SRC}/include", f"-I{t}",
             "-include", "vector", "-include", "cmath", "-include", "cstdio",
             os.path.join(HERE, "gen_cornerdetector_cases.cpp"), os.path.join(t, "cornerdetector.cpp"), "-o", exe])
        out = run([exe])
        # %.17g prints integers bare; Rust needs f64 literals.
        fl = lambda m: m.group(0) if re.search(r"[.eE]", m.group(0)) else m.group(0) + ".0"
        lines = []
        for l in out.splitlines():
            head, rest = l.split("], ", 1)
            head = re.sub(r"-?\d+(?:\.\d+)?(?:e[+-]?\d+)?", fl, head[len("        (&["):])
            rest = re.sub(r"^(\d+, \d+, \d+, )(\d+)(, &\[)", r"\1\2.0\3", rest)
            lines.append("        (&[" + head + "], " + rest)
        ok &= splice(os.path.join(SRC, "common/tvectorimage/cornerdetector.rs"), "\n".join(lines) + "\n", check)
        # --- casmfileinfo
        open(os.path.join(t, "orig.cpp"), "wb").write(git_show("flare/sources/flarefarm/tfarmclient/casmfileinfo.cpp"))
        open(os.path.join(t, "casmfileinfo.h"), "w").write(open(os.path.join(HERE, "casm_stub_casmfileinfo.h")).read())
        exe2 = os.path.join(t, "gen2")
        run(["g++", "-std=c++17", f"-I{t}", f'-DCASE_PATH="{t}/case.casm"',
             os.path.join(HERE, "gen_casm_cases.cpp"), "-o", exe2])
        ok &= splice(os.path.join(SRC, "flarefarm/tfarmclient/casmfileinfo.rs"), run([exe2]), check)
    if check and not ok:
        print("golden drifted")
        sys.exit(1)


if __name__ == "__main__":
    main()
