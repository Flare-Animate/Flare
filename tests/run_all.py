"""One command to run the repository's test suites.

Runs everything that can run unattended. Three checks are deliberately not in the
default set:

  tests/native/differential_shape.py   needs an extracted FLA
  tests/native/verify_shapes_svg.py     needs an extracted FLA and shape_export
  tests/native/mutation_check.py        needs a built tnzcore, which it rebuilds

They are listed at the end of the output so this is not mistaken for "everything
passes, therefore everything has been checked".

    python tests/run_all.py   (from the repository root)

Exits non-zero if anything fails.
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
    ("flipaclip profile", [sys.executable, "tests/test_flipaclip_profile.py"], REPO),
    ("game export", [sys.executable, "tests/test_game_export.py"], REPO),
    # No --no-build: the runner builds the test binaries itself. Passing it made
    # this suite depend on someone having remembered to build them first, so it
    # reported a failure for a missing artefact rather than a real defect. The
    # Flare build tree it links against comes from $FLARE_BUILD, which
    # run_tests.py reads, so an out-of-the-way build is picked up without
    # editing anything.
    ("native readers (C++)", [sys.executable, "run_tests.py"],
     os.path.join(HERE, "native")),
    ("flash fixtures", [sys.executable, "verify_fixtures.py"],
     os.path.join(HERE, "flash_fixtures")),
]

# Not run here, and why. Each would fail on a clean checkout for a reason that
# has nothing to do with whether the code is correct.
NEEDS_INPUT = [
    ("tests/native/differential_shape.py", "an extracted .fla directory"),
    ("tests/native/verify_shapes_svg.py", "an extracted .fla directory, and the "
                                          "output of shape_export"),
    ("tests/native/mutation_check.py", "a built tnzcore, because it rebuilds to "
                                       "inject each bug in turn"),
    # Not a check -- it reports rather than asserts -- but it is the thing that
    # finds the untested format, so a run that never invokes it has not used the
    # tool that would have reported a gap. It needs real files to say anything.
    ("tests/native/probe_samples", "real Adobe/Flash/Moho files to report on; it "
                                   "asserts nothing, so it cannot be a suite"),
]


def native_runner_finds_qt():
    """Ask the native runner for the Qt root rather than duplicating the search.

    run_tests.py owns find_qt_root(); importing it keeps one search in one place,
    and a failure there means "cannot find", not "crashed".
    """
    try:
        import importlib.util
        here = os.path.dirname(os.path.abspath(__file__))
        spec = importlib.util.spec_from_file_location(
            "_native_runner", os.path.join(here, "native", "run_tests.py"))
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        return bool(mod.find_qt_root())
    except Exception:
        return False


def main():
    # run_tests.py now locates Qt itself and feeds it to both cmake and PATH, so
    # this note fired on machines where the native suite was in fact running fine
    # -- it reported 97 passing checks under exactly the conditions it warned about.
    # Warn only when Qt genuinely cannot be found anywhere.
    if not os.environ.get("QT_BIN") and not native_runner_finds_qt():
        print("   note: Qt was not found. The C++ tests link Qt5Core, which is"
              " not beside\n"
              "         the test binary, so without it those processes fail to"
              " start -- no\n"
              "         output, exit code 0xC0000135. Set QT_BIN to"
              " <qt>/5.x/<kit>/bin.")

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
    else:
        print(f"PASSED: all {len(results)} suites")
    print()
    print("not covered here (each needs input a clean checkout does not have):")
    for name, why in NEEDS_INPUT:
        print(f"   {name}  -- {why}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
