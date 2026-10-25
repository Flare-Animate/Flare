"""Golden drift gate: re-derive every differential golden from the archived C++.

A golden is only a test if it cannot be edited by hand into agreement. Each generator
supports `--check`, which rebuilds the reference from git history and exits non-zero on
any byte of drift, so this suite is what makes the goldens mean something.

Both motion-wind goldens are checked: the 64-bit one that replays on LP64 and the
32-bit one that replays wherever `unsigned long` is 32 bits (Windows, i686 Linux).

Skips (exit 0, loudly) when no host C++ compiler is available. That is an environment
gap, not a defect: the port's own tests do not depend on a compiler, and failing here
would turn a missing toolchain into a red suite.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, ".."))

# Which host recorded each golden. Libm- and zlib-dependent goldens are only byte-stable
# on the platform that recorded them, so each is checked there: `linux` = glibc + the
# zlib shim, `macos` = Apple libm/clang. `--set` selects one; the default checks all.
LINUX_RECORDED = ("potentials", "swf writer", "swf shapes", "maxmin", "motion blur", "line blur", "gaussian blur")

CHECKS = [
    ("swf writer (FSWFStream, FPrimitive, FObj, FSound, FCT, FDTBitmaps)", ["tests/native/gen_swfwriter_cases.py", "--check"]),
    ("swf shapes/actions/sprite/sounds (FDTShapes, FAction, FDTSprite, FDTSounds)", ["tests/native/gen_swfshapes_cases.py", "--check"]),
    ("attenuation", ["tests/native/gen_igs_attenuation_cases.py", "--check"]),
    ("noise (64-bit word)", ["tests/native/gen_noise_cases.py", "--rng-bits", "64", "--check"]),
    ("noise in camera (32-bit word)", ["tests/native/gen_noise_cases.py", "--rng-bits", "32", "--check"]),
    (
        "motion wind (64-bit word)",
        ["tests/native/gen_igs_motion_wind_cases.py", "--rng-bits", "64", "--check"],
    ),
    (
        "motion wind (32-bit word)",
        ["tests/native/gen_igs_motion_wind_cases.py", "--rng-bits", "32", "--check"],
    ),
    ("maxmin", ["tests/native/gen_igs_maxmin_cases.py", "--check"]),
    ("gaussian blur", ["tests/native/gen_igs_gaussian_blur_cases.py", "--check"]),
    ("motion blur", ["tests/native/gen_igs_motion_blur_cases.py", "--check"]),
    ("line blur", ["tests/native/gen_igs_line_blur_cases.py", "--check"]),
    (
        "motion wind scanline walk (64-bit word)",
        ["tests/native/gen_igs_motion_wind_change_cases.py", "--rng-bits", "64", "--check"],
    ),
    (
        "motion wind scanline walk (32-bit word)",
        ["tests/native/gen_igs_motion_wind_change_cases.py", "--rng-bits", "32", "--check"],
    ),
    ("namebuilder", ["tests/native/gen_namebuilder_cases.py", "--check"]),
    ("perlinnoise", ["tests/native/gen_perlinnoise_cases.py", "--check"]),
    ("potentials (tnzext, libm exp)", ["tests/native/gen_potentials_cases.py", "--check"]),
]


def recorded_on(label: str) -> str:
    return "linux" if label.startswith(LINUX_RECORDED) and not label.startswith("motion wind") else "macos"


def main() -> int:
    which = "all"
    if "--set" in sys.argv:
        which = sys.argv[sys.argv.index("--set") + 1]
        if which not in ("all", "linux", "macos"):
            print("usage: run_stdfx_goldens.py [--set all|linux|macos]")
            return 2
    cxx = os.environ.get("CXX") or shutil.which("g++") or shutil.which("clang++")
    if not cxx:
        print("SKIP: no host C++ compiler (set CXX); golden drift not checked")
        return 0
    # Verify the compiler can actually find standard headers.
    import tempfile
    probe_dir = tempfile.mkdtemp(prefix="cxxprobe-")
    probe = os.path.join(probe_dir, "probe.cpp")
    with open(probe, "w") as fh:
        fh.write("#include <cstdint>\n#include <vector>\n#include <cmath>\nint main(){return 0;}\n")
    try:
        subprocess.run([cxx, "-std=c++17", "-c", probe, "-o", os.path.join(probe_dir, "probe.o")],
                       check=True, capture_output=True)
    except (subprocess.CalledProcessError, FileNotFoundError):
        shutil.rmtree(probe_dir, ignore_errors=True)
        print("SKIP: host C++ compiler cannot find standard headers; golden drift not checked")
        return 0
    shutil.rmtree(probe_dir, ignore_errors=True)

    failed = []
    for label, argv in CHECKS:
        if which != "all" and recorded_on(label) != which:
            continue
        print(f"--- {label}", flush=True)
        result = subprocess.run([sys.executable, *argv], cwd=REPO)
        if result.returncode != 0:
            failed.append(label)

    if failed:
        print(f"FAILED: drifted golden(s): {', '.join(failed)}")
        return 1
    print("all goldens reproduce from the archived C++")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
