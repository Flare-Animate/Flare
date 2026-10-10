"""Build and run the native reader tests.

Wraps the CMake configure/build/run sequence so the tests can be run with one
command, and so a missing tnzcore build produces an instruction rather than a
link error.

usage:
    python run_tests.py [--build-dir DIR] [--no-build] [--filter NAME]
"""
import argparse
import os
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
FLARE_BUILD = os.environ.get("FLARE_BUILD", os.path.join(REPO, "build_local"))
DEFAULT_BUILD = os.path.join(HERE, "build")


def find_lib(build, name):
    """Locate a target's import library in either supported build layout.

    A tree configured from flare/sources -- which is what CI does -- writes to
    <build>/<target>/RelWithDebInfo. The older build_local tree nests it under
    <build>/sources/<target>/RelWithDebInfo. Probing both means pointing
    --flare-build at either tree works, instead of reporting "tnzcore has not
    been built" for a build that is sitting right there. tests/native/CMakeLists.txt
    already probes both for the same reason.
    """
    for sub in (name, os.path.join("sources", name)):
        candidate = os.path.join(build, sub, "RelWithDebInfo", name + ".lib")
        if os.path.isfile(candidate):
            return candidate
    return None


# (binary, which fixture set it takes)
TARGETS = [
    ("flash_reader_tests", "flash"),
    ("xfl_shape_tests", "none"),
    ("swfshape_tests", "flash"),
    ("swfshape_extract_tests", "flash"),
    ("moho_reader_tests", "moho"),
    ("flareupdater_tests", "none"),
    ("flareupdater_net_tests", "none"),
    ("remote_protocol_tests", "none"),
]


def run(cmd, **kw):
    return subprocess.run(cmd, capture_output=True, text=True, **kw)


def die(msg):
    print(f"   {msg}")
    sys.exit(1)


def ensure_fixtures():
    """Regenerate the fixtures so a stale or partial tree cannot make a test
    pass or fail for the wrong reason."""
    fx = os.path.join(HERE, "..", "flash_fixtures")
    for script in ("generate_fixtures.py", "generate_trailer_fixtures.py"):
        p = os.path.join(fx, script)
        if not os.path.isfile(p):
            continue
        r = run([sys.executable, p], cwd=fx)
        if r.returncode:
            die(f"{script} failed:\n{r.stdout}{r.stderr}")
        print(f"   fixtures: {script}")

    # The Moho fixtures go to a temp directory, since they are generated rather
    # than committed.
    moho_fx = os.path.join(os.environ.get("TEMP", "/tmp"), "flare_moho_fixtures")
    if os.path.isdir(moho_fx):
        shutil.rmtree(moho_fx, ignore_errors=True)
    r = run([sys.executable, os.path.join(fx, "generate_moho_fixtures.py"),
             moho_fx])
    if r.returncode:
        die(f"generate_moho_fixtures.py failed:\n{r.stdout}{r.stderr}")
    print("   fixtures: generate_moho_fixtures.py")
    return os.path.join(HERE, "..", "flash_fixtures"), moho_fx


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--build-dir", default=DEFAULT_BUILD)
    ap.add_argument("--flare-build", default=FLARE_BUILD)
    ap.add_argument("--no-build", action="store_true",
                    help="skip the CMake configure and build")
    ap.add_argument("--filter", default="",
                    help="only run tests whose name contains this")
    ap.add_argument("--no-local-src", action="store_true",
                    help="link the tests against the built tnzcore instead of "
                         "compiling common/flash into them; catches export/ABI "
                         "mistakes that FLASH_LOCAL_SRC hides")
    args = ap.parse_args()

    flare_build = os.path.abspath(args.flare_build)
    tnzcore_lib = find_lib(flare_build, "tnzcore")
    flareqt_lib = find_lib(flare_build, "flareqt")
    if not tnzcore_lib:
        die(f"tnzcore has not been built (looked in "
            f"<build>/tnzcore/RelWithDebInfo and "
            f"<build>/sources/tnzcore/RelWithDebInfo).\n"
            f"  cmake --build {flare_build} --config RelWithDebInfo --target tnzcore")
    print(f"   tnzcore: {tnzcore_lib}")
    if flareqt_lib:
        print(f"   flareqt: {flareqt_lib}")
    else:
        # Not fatal for the format readers, but the updater tests link it, so
        # say so plainly rather than failing later with unresolved externals.
        print("   flareqt: not built -- flareupdater tests will be skipped")

    flash_fx, moho_fx = ensure_fixtures()

    if not args.no_build:
        build = os.path.abspath(args.build_dir)
        # Point the tests at this tree's build rather than the default.
        # -S is required: without it CMake configures whatever source directory
        # the caller's cwd happens to be, which is the repo root -- and the root
        # CMakeLists.txt configures the whole application rather than the tests.
        #
        # FLASH_LOCAL_SRC defaults ON here even though CMakeLists.txt keeps it OFF by
        # default. The tests link TNZCORE_LIB, which is whatever tnzcore.dll happens to
        # be lying in the build tree -- frequently a prebuilt DLL that predates the
        # Flash reader. That surfaced as LNK1120 "unresolved external
        # __imp_?extractSwfShapes@FlashAssets" in swfshape_extract_tests and
        # swfshape_real_tests: the tests were right and the DLL was stale, so the
        # suite reported a build failure for a defect that was not in the source.
        # Compiling common/flash into the test executables makes them depend only on
        # the checked-out source. Pass --no-local-src to test against the built DLL,
        # which is the arrangement that catches export/ABI mistakes.
        extra = [] if args.no_local_src else ["-DFLASH_LOCAL_SRC=ON"]
        r = run(["cmake", "-S", HERE, "-B", build,
                 "-DCMAKE_BUILD_TYPE=RelWithDebInfo",
                 f"-DBUILD={flare_build}",
                 f"-DFLASH_ROOT={REPO}"] + extra)
        if r.returncode:
            die("cmake configure failed:\n" + r.stdout + r.stderr)
        r = run(["cmake", "--build", build, "--config", "RelWithDebInfo", "-j", "8"])
        if r.returncode:
            die("cmake build failed:\n" + r.stdout + r.stderr)
        print("   built the test binaries\n")

    exe_dir = os.path.join(os.path.abspath(args.build_dir), "RelWithDebInfo")
    # Qt's plugins must be reachable or bitmap decoding silently yields nothing.
    # These binaries link tnzcore.dll, Qt5Core.dll and (transitively) OpenGL,
    # zlib and the Freeglut/LZ4/JPEG builds -- none of which sit beside the test
    # executable. Without these on PATH the process fails to start at all, with
    # no output and exit code 0xC0000135.
    env = dict(os.environ)
    dll_dirs = [os.path.join(flare_build, "RelWithDebInfo"),
                os.path.join(REPO, "LocalTest"),
                # flareqt pulls in the vcpkg-built third-party DLLs (OpenCV, tiff,
                # ...) transitively. Without these on PATH a test that links
                # flareqt dies at load time with 0xC0000135 and no output.
                os.path.join(REPO, "vcpkg", "installed", "x64-windows", "bin")]
    # QT_BIN unset was reported as "the C++ tests link Qt5Core, which is not beside
    # the test binary, so without it the process fails to start at all". Rather than
    # only honouring the env var, derive it: a Qt bin dir is any directory containing
    # Qt5Core.dll, searched along the usual install roots. This makes the suite
    # self-configuring on a machine where Qt is installed but not exported, which is
    # the common case on Windows.
    qt_bin = env.get("QT_BIN", "")
    if not qt_bin:
        candidates = []
        if sys.platform == "win32":
            qroot = os.environ.get("QTDIR", r"C:\Qt")
            candidates.append(os.path.join(qroot, "5.15.2", "msvc2019_64", "bin"))
            candidates.append(os.path.join(qroot, "5.15.2", "mingw73_64", "bin"))
            candidates.append(os.path.join(qroot, "5.15.2", "msvc2019", "bin"))
        else:
            candidates += ["/usr/lib/x86_64-linux-gnu", "/usr/lib64", "/usr/lib"]
        for c in candidates:
            if os.path.isfile(os.path.join(c, "Qt5Core.dll")) or \
               os.path.isfile(os.path.join(c, "libQt5Core.so.5")):
                qt_bin = c
                break
        if qt_bin:
            env["QT_BIN"] = qt_bin
    if qt_bin:
        dll_dirs.append(qt_bin)
        env["QT_PLUGIN_PATH"] = os.path.join(qt_bin, "..", "plugins")
    env["PATH"] = os.pathsep.join(
        [d for d in dll_dirs if os.path.isdir(d)] + [env.get("PATH", "")])

    # Where the built tnzcore is, for the suites that resolve it at run time
    # rather than linking against it.
    env["FLARE_TNZCORE"] = os.path.join(flare_build, "RelWithDebInfo",
                                        "tnzcore.dll")

    failed = 0
    for name, which in TARGETS:
        if args.filter and args.filter not in name:
            continue
        if name.startswith("flareupdater") and not flareqt_lib:
            print(f"   SKIP  {name} (flareqt.lib not built)")
            continue
        exe = os.path.join(exe_dir, name + ".exe")
        if not os.path.isfile(exe):
            die(f"{name} was not built (expected {exe})")
        fxd = flash_fx if which == "flash" else moho_fx
        if which == "none":
            # Self-contained: the geometry is written into the test, so no
            # fixture directory is needed.
            fxd = ""
        print(f"\n=== {name} ===")
        argv = [exe] + ([os.path.abspath(fxd)] if fxd else [])
        r = run(argv, env=env)
        sys.stderr.write(r.stderr)
        tail = [l for l in r.stderr.splitlines() if "checks," in l]
        print("   " + (tail[-1] if tail else f"exit {r.returncode}"))
        if r.returncode:
            failed += 1

    print()
    if failed:
        print(f"FAILED: {failed} test binary/binaries")
        return 1
    print("PASSED: all test binaries")
    return 0


if __name__ == "__main__":
    sys.exit(main())
