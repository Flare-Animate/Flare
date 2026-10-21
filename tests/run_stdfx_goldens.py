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

CHECKS = [
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
    (
        "motion wind scanline walk (64-bit word)",
        ["tests/native/gen_igs_motion_wind_change_cases.py", "--rng-bits", "64", "--check"],
    ),
    (
        "motion wind scanline walk (32-bit word)",
        ["tests/native/gen_igs_motion_wind_change_cases.py", "--rng-bits", "32", "--check"],
    ),
]


def main() -> int:
    cxx = os.environ.get("CXX") or shutil.which("clang++") or shutil.which("g++")
    if not cxx:
        print("SKIP: no host C++ compiler (set CXX); golden drift not checked")
        return 0

    failed = []
    for label, argv in CHECKS:
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
