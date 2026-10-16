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
LOCK = os.path.join(REPO, "flare", "sources", "Cargo.lock")


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
    # The workspace also contains toonzqt (egui/wgpu) and flarelib, which
    # need crates --offline may not have cached; they are covered by the
    # full workspace run, not by this suite. Cargo resolves the whole
    # workspace even for -p, so temporarily strip those members (restored in
    # finally) and test only the offline-capable crates.
    with open(MANIFEST, encoding="utf-8") as f:
        orig = f.read()
    # cargo rewrites Cargo.lock to match the stripped member list below; snapshot
    # it too so the suite leaves the working tree clean.
    lock_orig = None
    if os.path.isfile(LOCK):
        with open(LOCK, "rb") as f:
            lock_orig = f.read()
    lines = []
    skip_patch = False
    for line in orig.splitlines(keepends=True):
        s = line.strip()
        if s.startswith("[patch."):
            skip_patch = True
            continue
        if skip_patch:
            if s.startswith("["):
                skip_patch = False
            else:
                continue
        if s.startswith('"toonzqt"'):
            continue
        if line.strip().startswith("members ="):
            line = line.replace(', "toonzqt"', "").replace(
                ', "flarelib"', "")
        lines.append(line)
    stripped = "".join(lines)
    with open(MANIFEST, "w", encoding="utf-8") as f:
        f.write(stripped)
    try:
        r = subprocess.run([cargo, "test", "-p", "flare_formats", "-p", "flare_remote",
                            "--offline", "--manifest-path", MANIFEST],
                           cwd=REPO, capture_output=True, text=True)
    finally:
        with open(MANIFEST, "w", encoding="utf-8") as f:
            f.write(orig)
        if lock_orig is not None:
            with open(LOCK, "wb") as f:
                f.write(lock_orig)
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