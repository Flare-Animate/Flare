"""FlipaClip profile sanity: layouts.txt rooms exist, hierarchy indices match panes, default is Animate."""
import os, re, sys
R = os.path.join(os.path.dirname(__file__), "..", "stuff/profiles/layouts/rooms")
d = os.path.join(R, "FlipaClip")
rooms = open(os.path.join(d, "layouts.txt")).read().split()
assert rooms, "empty layouts.txt"
for r in rooms:
    t = open(os.path.join(d, r)).read()
    panes = set(re.findall(r"^pane_(\d+)[\\]name=", t, re.M))
    h = set(re.findall(r"\d+", re.search(r'hierarchy="([^"]*)"', t).group(1).replace("-1", "")))
    assert h <= panes and panes <= h | set(), (r, h, panes)
    assert "Timeline" in t and "name=FlipaClip" in t
assert os.path.exists(os.path.join(d, "menubar_template.xml"))
src = open(os.path.join(os.path.dirname(__file__), "..", "flare/sources/flarelib/preferences.cpp")).read()
assert '"CurrentRoomChoice", QMetaType::QString, "Animate"' in src
print("ok")
assert not os.path.exists(os.path.join(R, "Default")), "no room dir named Default"
assert os.path.isdir(os.path.join(R, "Animate")) and os.path.isdir(os.path.join(R, "OpenToonz")) and os.path.isdir(d)
print("ok2")
