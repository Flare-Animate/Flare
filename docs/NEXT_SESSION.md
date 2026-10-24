# Open work for later sessions

Written 2026-10-24 so nothing is lost between sessions. Rules for agents: every sub-agent prompt starts with
"First invoke Skill anthropic-skills:caveman and anthropic-skills:strict"; cheapest viable model; independent
reviewer reads from git (never the working tree) before a push; push once per round after local build + CI.

## Needs the owner
- PR #151 (`Rust-Rewrite` -> `master`) has conflicts with `master`; merge `master` into `Rust-Rewrite` only with a go-ahead.
- Repo settings: Pages source = GitHub Actions; Actions > "Allow GitHub Actions to create and approve pull requests"
  (fixes `sync_upstream` and `cpp2rust-draft` PR opening); a security-scan workflow errors `JF_URL must be provided`.
- Code scanning alerts are unreadable with the session token (403, needs security_events); paste or grant to fix them.
- `nflare`: unidentified (only `sunflare` shaders match). Needs a repo URL to chop up like Next2Flash.
- Issues: close #112, #98 once Linux+macOS+Windows builds are green on the tip; #66/#71 need a macOS launch proof;
  #116/#114/#110/#117 need a regression test or guard before closing. Do not close feature requests.

## Known defects (from docs/TEST_REPORT_FLASH.md)
- AS3-only SWF import reports 0x0 px / 0 frames (header parse).
- PSD import only copies the file; no layers.
- Unzipped XFL folder cannot be chosen in the import dialog; typed filename in the file browser is ignored.
- Command line accepts only `.tnz`.
- About dialog loads `:Resources/tahoma2d_about.png` which does not exist (blank logo).
- Packaging: `.deb` rename in `ci-scripts/linux/flare-buildpkg.sh` is a guess; macOS bundle must provide `Flare.real`.
- Animate layout: empty black Flash Instance strip in room 2; existing users keep old personal layouts until
  Windows > Workspace > Reset Room Layout; shared `settings/commandbar.xml` is still the OpenToonz default.

## Rewrite (docs/REWRITE_BACKLOG.md, docs/PORTING.md)
- ~1100 C/C++ files remain; most are Qt/OpenGL bound. Qt-free logic files are nearly exhausted; next: Qt UI strategy
  (egui crates) rather than file-by-file ports. The owner's own agent ports `common/trop/*` and `common/tcore/*`.
- Next2Flash stages 2+: swf_to_n2d/compile_n2d (replace with native SWF/HTML5 export), shape_converter, fla_xfl
  vs ac_xfl, bitmap_converter, cycle_detector, mxmlc recompile workflow (docs/NEXT2FLASH_INTEGRATION.md).
- Ported-but-unverified: Windows/MSVC and macOS behaviour of recent ports (avl.rs 64-bit keys, tcolor inline
  forwarders, SWF writer headers); `igs_line_blur` assumes Release/NDEBUG.
- cpp2rust draft workflow works but only drafts tiny class-free files; use it for leaf files only.
- Guards in CI: check_cmake_sources.py, check_header_defs.py, check_preferences_ids.py, check_layout_profiles.py.
