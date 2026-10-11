"""Run the Rust crates' unit tests (flare_formats + flare_remote).

The workspace manifest is flare/sources/Cargo.toml. Skips cleanly (exit 0 with a
notice) when cargo is absent, so a machine without a Rust toolchain does not
report a false failure -- but it never reports success it did not measure.
"""
import os
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, ".."))
MANIFEST = os.path.join(REPO, "flare", "sources", "Cargo.toml")


def find_cargo():
    for name in ("cargo", "cargo.exe"):
        p = shutil.which(name)
        if p:
            return p
    home = os.environ.get("USERPROFILE") or os.path.expanduser("~")
    p = os.path.join(home, ".cargo", "bin", "cargo.exe")
    return p if os.path.isfile(p) else None


def main():
    cargo = find_cargo()
    if not cargo:
        print("SKIP: cargo not found; Rust tests not run (not counted as a pass)")
        return 0
    r = subprocess.run([cargo, "test", "--workspace", "--offline",
                        "--manifest-path", MANIFEST],
                       cwd=REPO, capture_output=True, text=True)
    out = (r.stdout or "") + (r.stderr or "")
    fails = [l for l in out.splitlines() if "test result: FAILED" in l or l.startswith("error")]
    passed = sum(int(l.split()[3]) for l in out.splitlines()
                 if "test result: ok" in l)
    for l in out.splitlines():
        if "test result:" in l:
            print(l)
    if r.returncode != 0:
        print(out[-4000:])
        print("FAIL: cargo test exit %d" % r.returncode)
        return 1
    print("ok: %d Rust tests passed" % passed)
    return 0


if __name__ == "__main__":
    sys.exit(main())