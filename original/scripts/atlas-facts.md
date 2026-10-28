# atlas-facts — the facts scripts/atlas_check.py enforces

Read by `scripts/atlas_check.py`. The atlas is checked against the CODE first (files,
symbols, constants resolved live); this file adds the few facts a live scan cannot infer.

## constants
# name  code-path (definition must read `name = <plain-decimal>` there)
kTwipsPerPixel  flare/sources/common/flash/XFLShape.cpp

## require
# load-bearing names whose disappearance from the atlas is worth a soft flag
flare_flash_frame_count
flare_cfbf_streams
flare_moho_free
unzip_any

## forbid
# names that must NOT appear — retired subsystems, removed APIs
# empty until the first rename

## exempt-paths
# path-ish tokens to skip in file resolution: externals, build vars, format labels
vcpkg
thirdparty
LocalTest
moho_manifest.json
.fla
.swf
.xfl
.moho
.tpl
.ini
