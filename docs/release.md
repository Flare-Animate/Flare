# Releasing

## Checklist for a version

1. `main` is green in CI; fmt, clippy and tests pass locally.
2. Move the `[Unreleased]` entries of [CHANGELOG.md](../CHANGELOG.md) under the new version and date.
3. Bump `version` in the root `Cargo.toml` (`[workspace.package]`) and run `cargo build --locked`
   once to refresh `Cargo.lock`.
4. Build the packages (below) and run the smoke test on each platform you ship.
5. Tag `vX.Y.Z` on `main`, push the tag, and attach the packages to a GitHub release.

## Packaging

- **macOS**: `scripts/package-macos.sh` builds a release binary and wraps it in `Flare.app`
  (Info.plist, icon set), then a zip in `dist/`. Signing and notarising need a developer account
  and are not scripted.
- **Linux**: `scripts/package-linux.sh` builds a tarball with the app, `flare-cli` and `flare-mcp`
  (needs the libraries listed in [CONTRIBUTING.md](../CONTRIBUTING.md)). An AppImage or distro
  package can wrap the same binaries. Not yet run on Linux; CI only builds and tests there.
- **Windows**: `scripts/package-windows.ps1` builds a zip with the same three programs. An
  installer is not scripted yet, and the script has not been run on Windows.
- **Web**: experimental, not part of a release.

## Smoke test (by hand, per platform)

The GUI is not driven by the automated tests, so before a release someone does this once:

1. Launch; the window, tool strip, stage, panels and timeline appear.
2. Draw with the pencil, the rectangle and the pen; undo and redo each.
3. Insert keyframes, add a motion tween, scrub and play the timeline.
4. Save as `.flare`, close, open it again; the drawing is intact.
5. Import a `.fla`; export a PNG and a GIF; publish a SWF.
6. Open two documents as tabs and switch between them.
7. Quit with unsaved changes; the app asks first. Relaunch after a forced quit; recovery is offered.
