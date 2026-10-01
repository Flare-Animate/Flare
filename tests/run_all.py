"""One command to run every test in the repository.

Deliberately narrow: it runs the suites that exist and reports honestly when
one cannot run, rather than skipping silently or pretending the rest covers for
it.

    python tests/run_all.py   (from the repository root)

Exits non-zero if anything fails or if a suite that should be runnable is not.
"""
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, ".."))

# (label, argv, cwd)
SUITES = [
    ("flash/bridge (pytest)", [sys.executable, "-m", "pytest",
                               "tools/flash/tests", "tools/sync/tests", "-q"], REPO),
    ("moho menu wiring", [sys.executable, "tests/moho/test_moho_menu.py"], REPO),
    ("native readers (C++)", [sys.executable, "run_tests.py", "--no-build"],
     os.path.join(HERE, "native")),
]


def main():
    if not os.environ.get("QT_BIN"):
        print("   note: QT_BIN is not set, so the C++ tests may not find Qt's "
              "plugins\n"
              "         and report that no bitmaps decoded. Set it to "
              "<qt>/5.x/<kit>/bin.")

    results = []
    for label, argv, cwd in SUITES:
        print(f"\n=== {label} ===")
        r = subprocess.run(argv, cwd=cwd, capture_output=True, text=True)
        out = (r.stdout + r.stderr).rstrip()
        # Show the last few meaningful lines rather than everything.
        lines = [l for l in out.splitlines() if l.strip()]
        for line in lines[-6:]:
            print(f"   {line}")
        results.append((label, r.returncode, out))

    print()
    failed = [(l, rc) for l, rc, _ in results if rc != 0]
    for label, rc, out in results:
        print(f"   {'PASS' if rc == 0 else 'FAIL'}  {label}")
    print()
    if failed:
        print(f"FAILED: {len(failed)} of {len(results)} suites")
        return 1
    print(f"PASSED: all {len(results)} suites")
    return 0


if __name__ == "__main__":
    sys.exit(main())
