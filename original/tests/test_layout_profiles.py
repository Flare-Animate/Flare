"""Layout profiles: every profile exposes ALL UI options, and preserved profiles stay preserved.

  python tests/test_layout_profiles.py            # check
  python tests/test_layout_profiles.py --update   # re-record the preserved-profile manifest
                                                  # (only when you MEAN to change a preserved profile)

1. scripts/check_layout_profiles.py must report 100 % command coverage and no errors for
   every profile under stuff/profiles/layouts/rooms/.
2. The preserved profiles (OpenToonz, AnimateLegacy, FlipaClip, StudioGhibli) must not drift:
   their layouts.txt, room*.ini and menubars (with the generated "All Commands" block
   stripped) are hashed in tests/layout_profile_manifest.json.  Animate is the actively
   designed default and is deliberately not locked.
3. The stock profile set exists and the preference default is a profile that exists.
"""
import hashlib
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent
sys.path.insert(0, str(REPO / "scripts"))
import check_layout_profiles as C  # noqa: E402

MANIFEST = HERE / "layout_profile_manifest.json"
PRESERVED = ["AnimateLegacy", "FlipaClip", "OpenToonz", "StudioGhibli"]
STOCK = set(PRESERVED) | {"Animate"}
GEN = re.compile(r"[ \t]*" + re.escape(C.GEN_BEGIN) + r".*?" + re.escape(C.GEN_END) + r"\n?", re.S)


def fingerprint(pdir: Path) -> dict:
    out = {}
    for f in sorted(pdir.iterdir()):
        if f.suffix in (".ini", ".txt", ".xml"):
            data = f.read_bytes().decode("utf-8").replace("\r\n", "\n")
            data = GEN.sub("", data)
            out[f.name] = hashlib.sha256(data.encode("utf-8")).hexdigest()
    return out


def current() -> dict:
    return {p: fingerprint(C.ROOMS / p) for p in PRESERVED}


def main() -> int:
    if "--update" in sys.argv:
        MANIFEST.write_text(json.dumps(current(), indent=1, sort_keys=True) + "\n")
        print("manifest updated")
        return 0
    rc = C.main([])
    assert rc == 0, "scripts/check_layout_profiles.py reports gaps"
    names = {p.name for p in C.ROOMS.iterdir() if p.is_dir()}
    assert STOCK <= names, f"missing stock profile(s): {sorted(STOCK - names)}"
    assert "Default" not in names, "no room dir named Default"
    src = (REPO / "flare/sources/flarelib/preferences.cpp").read_text()
    m = re.search(r'"CurrentRoomChoice", QMetaType::QString, "(\w+)"', src)
    assert m and m.group(1) in names, "CurrentRoomChoice default must be an existing profile"
    want = json.loads(MANIFEST.read_text())
    have = current()
    for p in PRESERVED:
        assert have[p] == want.get(p), (
            f"preserved profile {p} changed: "
            f"{sorted(set(have[p].items()) ^ set(want.get(p, {}).items()))[:4]} "
            "(run with --update only if intended)")
    print("ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
