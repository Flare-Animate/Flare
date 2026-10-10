# Known issues triage

- #71 macOS error: report is screenshots only, no reproducible detail. Needs text log + macOS version.
- #67 crash, #66 "App is incomplete": no repro/body. Needs crash report.
- #56 Linux build freeze: build saturates CPU on low-end machines. Use `cmake --build . -j2` to limit parallelism. Nightly AppImage SIGINT report is user ctrl+c, not a crash.
- #47 FLA open / Adobe Animate room layout: Flash import is WIP; tracked under Flash/FLA support roadmap.
