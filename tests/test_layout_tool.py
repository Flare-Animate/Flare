"""Layout profiles: Default(Animate) is pref default, OpenToonz is separate, all profiles validate, bundle roundtrips."""
import os, sys, tempfile
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "tools"))
import layout_tool as L
R = L.ROOMS
profiles = sorted(os.listdir(R))
assert {"Default", "OpenToonz", "FlipaClip"} <= set(profiles), profiles
for p in profiles:
    e = L.validate(os.path.join(R, p))
    assert not e, (p, e)
rd = lambda *a: open(os.path.join(R, *a)).read()
assert rd("OpenToonz", "layouts.txt") != rd("Default", "layouts.txt")
with tempfile.TemporaryDirectory() as t:
    j = os.path.join(t, "b.json")
    L.export(os.path.join(R, "FlipaClip"), j)
    d = L.imp(j, t)
    assert not L.validate(d)
    assert open(os.path.join(d, "room1.ini")).read() == rd("FlipaClip", "room1.ini")
print("ok")
