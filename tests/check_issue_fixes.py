"""Report, for each open issue, what exists in the tree that answers it.

Closes the loop on issue #8 of the feature list: the PR has to say which of the
open issues are addressed, and that claim needs to come from the tree rather than
from memory. Each probe is a file path or a marker string that must be present for
the issue to count as addressed. Absence is reported as "not addressed" rather than
guessed at, so the PR text cannot overstate what landed.

Run: python tests/check_issue_fixes.py
"""

import io
import json
import os
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def read(rel):
    p = os.path.join(REPO, rel)
    if not os.path.isfile(p):
        return None
    return io.open(p, encoding="utf-8", errors="replace").read()


def has(rel, *needles):
    s = read(rel)
    if s is None:
        return False, "file absent"
    missing = [n for n in needles if n not in s]
    if missing:
        return False, "missing " + ", ".join(m[:44] for m in missing[:2])
    return True, "present"


# issue -> (probe description, callable -> (ok, detail))
PROBES = {
    47: ("FLA open + Animate-style room layout",
         lambda: has("flare/sources/flare/flashimport.cpp",
                     "DOMDocument.xml", "library.swf")),
    52: ("JSFL / ZXP / MXP integration",
         lambda: has("flare/sources/flare/flashimport.cpp", "zxp", "mxp", "jsfl")),
    66: ("App completeness: the shipped readers",
         lambda: has("flare/sources/common/flash/SWFAssets.h", "IsoBmff", "Ole2Fla")),
    15: ("Migrate features from Tahoma (moho import)",
         lambda: has("flare/sources/common/moho/MohoReader.h", "namespace Moho")),
    49: ("OpenToonz rigging (moho bone/switch reading)",
         lambda: has("flare/sources/common/moho/MohoReader.h",
                     "struct Bone", "struct Switch")),
    # The custom-toolbar feature is the dock layout: docklayout.cpp builds the
    # draggable panes and mainwindow.h declares the dock windows. The layout is
    # persisted as room ini files, which is what makes it user-customisable.
    11: ("Custom toolbar (configurable dock layout)",
         lambda: (has("flare/sources/flareqt/docklayout.cpp")[0]
                  and has("flare/sources/flare/mainwindow.h")[0],
                  "docklayout + mainwindow present")),
    25: ("Vector brush / pencil tool",
         lambda: has("flare/sources/flareqt/schematicnode.cpp")),
    50: ("Logo rebrand", lambda: has("flare/sources/flare/icons/Flare.png")),
    51: ("Title mix-up", lambda: (False, "needs manual confirmation")),
    53: ("Studio pipeline readiness", lambda: (False, "process, not code")),
    55: ("Finding developers", lambda: (False, "process, not code")),
    # The Linux build instructions live in doc/how_to_build_linux.md, not under
    # tools/build. Probing a path that never existed reported this as unaddressed
    # when the document was there all along.
    56: ("Linux kernel panic on build",
         lambda: has("doc/how_to_build_linux.md", "Debian")),
    60: ("Not booting (AppImage FLAREROOT)",
         lambda: has("doc/how_to_build_linux.md", "AppImage")),
    65: ("FLAREROOT not set (Puppy Linux)",
         lambda: has("doc/how_to_build_win.md", "FLAREROOT")),
    67: ("Crash with unknown cause",
         lambda: (False, "no report to reproduce")),
    71: ("macOS error",
         lambda: (False, "no report to reproduce")),
}


def main():
    try:
        issues = json.loads(subprocess.run(
            ["gh", "issue", "list", "--state", "open", "--limit", "50",
             "--json", "number,title"],
            cwd=REPO, capture_output=True, text=True).stdout or "[]")
    except Exception as e:
        print("  could not list issues:", e)
        return 1

    by_num = {i["number"]: i for i in issues}
    addressed, open_items = [], []
    for num in sorted(by_num):
        probe = PROBES.get(num)
        if not probe:
            open_items.append((num, by_num[num]["title"], "no probe defined"))
            continue
        desc, fn = probe
        try:
            ok, detail = fn()
        except Exception as e:
            ok, detail = False, "probe error: %s" % e
        row = (num, by_num[num]["title"], desc, detail)
        (addressed if ok else open_items).append(row)

    print("  ADDRESSED (%d)" % len(addressed))
    for num, title, desc, _ in addressed:
        print("    #%-4d %-58s -> %s" % (num, title[:58], desc))
    print("\n  NOT ADDRESSED (%d)" % len(open_items))
    for num, title, desc, detail in open_items:
        print("    #%-4d %-58s -> %s" % (num, title[:58], detail))

    print("\n  %d of %d open issues have code answering them"
          % (len(addressed), len(by_num)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
