# Known issues triage

- #71 macOS error: report is screenshots only, no reproducible detail. Needs text log + macOS version.
- #67 crash, #66 "App is incomplete": no repro/body. Needs crash report.
- #56 Linux build freeze: build saturates CPU on low-end machines. Use `cmake --build . -j2` to limit parallelism. Nightly AppImage SIGINT report is user ctrl+c, not a crash.
- #47 FLA open / Adobe Animate room layout: Flash import is WIP; tracked under Flash/FLA support roadmap.
- #65 FLAREROOT not set (Puppy Linux): fixed in code. Read-only HOME falls back to the packaged stuff dir (common/tapptools/tenv.cpp), so FLAREROOT is never left unset.
- #60 AppImage won't start (EndeavourOS): FLAREROOT part fixed as #65. Remaining cause is the missing Qt wayland platform plugin in the AppImage; needs packaging change, not done here.
