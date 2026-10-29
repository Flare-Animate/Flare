# Releasing

## What 0.1.0 means

0.1.0 is a **usable early alpha of the core workflow**: a person can draw, animate on a
timeline, keep several documents open, save and reopen them, trade them with Animate, and export
the result, on macOS, Windows and Linux. It is not feature parity with Animate. It is released
when every box below is ticked.

**Scope: what has to work** (all built; each has automated tests unless marked by hand)

- [x] Drawing: the tool set, fills, strokes, gradients, text, undo and redo, copy and paste.
- [x] Animating: layers, folders, masks, motion guides, keyframes, linear motion tweens, onion
  skin, playback with sound.
- [x] A library of symbols and instances placed on the stage.
- [x] Projects: save, open, revert, recent files, several documents as tabs, autosave and crash
  recovery.
- [x] Animate exchange: import and export XFL and FLA, with what is not carried over reported.
- [x] Output: PNG, GIF and SWF.
- [x] The same documents from a command line (`flare-cli`) and an MCP server (`flare-mcp`).

**Quality: what has to be true** (checked by machines)

- [x] CI green on `main`: format, lints, tests on Linux, the browser build, the 500-line rule.
- [x] `cargo audit` reports no vulnerabilities.
- [x] The command line works end to end on real files (`scripts/tests/test_cli_smoke.py`).
- [x] Hostile and damaged input is refused, not a crash (property tests, fuzz-style tests).
- [x] A tagged build produces all six platform packages and verified checksums (run once, green).

**Quality: what has to be true** (checked by a person, see the smoke test below)

- [x] The packaged macOS app starts and stays running (launched for eight seconds on Apple
  Silicon: signature valid, no output, no panic). This is a startup check only, nothing was drawn.
- [ ] The smoke test passes by hand on macOS.
- [ ] The app starts and draws on one Windows and one Linux machine.

**Honesty: what has to be written down**

- [x] Release notes and `CHANGELOG.md` say what 0.1.0 does not do.
- [x] `SECURITY.md` says how to report a problem and what the dependency audit shows.

**Not in 0.1.0** (shown dimmed in the menus, or listed in the notes): shape tweens, more than
one scene, scripting and the Commands menu, editable keyboard shortcuts, editable text after it
is placed, the web build, Developer-ID signing and notarising, an auto-update.


Packaging follows the same layout as the other Craft projects: platform files in
[`packaging/`](../packaging), the build scripts in [`scripts/packaging/`](../scripts/packaging)
(Python, one per platform), icons in [`assets/`](../assets), and one workflow,
[`release.yml`](../.github/workflows/release.yml), that builds natively on each platform.

## What a release contains

For each of six targets (Windows, macOS and Linux, x64 and arm64):

| Platform | Desktop | Command line |
| -------- | ------- | ------------ |
| macOS | `flare-<v>-macos-<arch>.dmg` and `-update.zip` (the same ad-hoc signed `Flare.app`) | `flare-cli-<v>-macos-<arch>.tar.gz` |
| Windows | `.msi` (WiX 5, registers `.flare`), `-portable.zip`, and a bare `.exe` | `flare-cli-<v>-windows-<arch>.zip` |
| Linux | `.deb`, `.rpm`, `.AppImage` and a `.tar.gz` | `flare-cli-<v>-linux-<arch>.tar.gz` |

The command line archive holds `flare-cli` and `flare-mcp`. A `SHA256SUMS.txt` covers everything.
The web build is experimental and not released.

## Checklist for a version

1. `main` is green in CI; fmt, clippy and tests pass locally.
2. Move the `[Unreleased]` entries of [CHANGELOG.md](../CHANGELOG.md) under the new version and date.
3. Write `docs/releases/v<version>.md`: a short, user-facing summary. It becomes the release text and
   must exist before tagging.
4. Set `version` in the root `Cargo.toml` (`[workspace.package]`), run `cargo build --locked` once to
   refresh `Cargo.lock`. The version must be plain semver (the MSI limits the numbers).
5. Run the smoke test below on at least one platform.
6. With the maintainer's explicit permission (never before), tag `v<version>` on `main` and push the tag. The workflow validates that the tag matches the
   manifest, builds all six targets, checks the full asset set and checksums, and publishes only if
   every platform succeeded.

`package_release.py validate --tag v<version>` runs the first checks locally, and
`python -m unittest discover -s scripts/tests -p 'test_package_*.py'` tests the packaging logic.

## Building locally

macOS, from a release build (`cargo build --release -p flare-desktop`):

```
python3 scripts/packaging/package_macos.py --binary target/release/flare-desktop --output target/macos/Flare.app
python3 scripts/packaging/package_release.py package --version 0.1.0 --target aarch64-apple-darwin \
    --binary target/release/flare-desktop --output dist     # needs: pip install dmgbuild==1.6.7
```

`package_macos.py` also takes `--install` and `--register`. The bundle is signed ad hoc so it runs
on the machine that built it; signing with a Developer ID and notarising are not done yet, so a
downloaded copy needs a right-click, Open the first time.

Linux needs the libraries in [CONTRIBUTING.md](../CONTRIBUTING.md) plus `cargo-packager` 0.11.8 and
`cargo-generate-rpm` 0.21.0 (the workflow pins both). Windows needs WiX 5.

What has been run: the macOS bundle, DMG and updater zip were built and verified on Apple Silicon,
and the full release workflow (all six targets: MSI, `.deb`, `.rpm`, AppImage, DMG, portable
archives, checksums) ran green on GitHub without publishing. The workflow only runs on a `v*` tag, on
pull requests that touch packaging, or by hand from the Actions tab (no tag, nothing is published).

## Icon

`assets/flare-icon.png` is the 1024 px master, trimmed to a transparent rounded square with a thin
dark edge; `flare-icon.iconset`, `.icns` and `.ico` are made from it.

## Smoke test (by hand)

The GUI is not driven by the automated tests, so before a release someone does this once:

1. Launch; the window, tool strip, stage, panels and timeline appear.
2. Draw with the pencil, the rectangle and the pen; undo and redo each.
3. Insert keyframes, add a motion tween, scrub and play the timeline.
4. Save as `.flare`, close, open it again; the drawing is intact.
5. Import a `.fla`; export a PNG and a GIF; publish a SWF.
6. Open two documents as tabs and switch between them.
7. Quit with unsaved changes; the app asks first. Relaunch after a forced quit; recovery is offered.
