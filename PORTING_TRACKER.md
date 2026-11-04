# Porting Tracker: Rust-Rewrite / Master → Main

## Goal
Port OpenToonz, Moho, and Flare features missing from the `main` branch (pure-Rust rewrite)
into the new crate architecture.

## Missing file/project types (Main currently: .flare, .fla, .xfl, .swf)

### OpenToonz (Rust-Rewrite has C++ in common/timage, common/tlevel, common/tropcm, common/toonz)
- [ ] .tnz (Toonz scene) — scene file with level references, xsheet
- [ ] .tlv (Toonz level) — raster level with frames
- [ ] .pli (Toonz vector level) — vector level
- [ ] .tzu (Toonz palette) — palette file
- [ ] .tzp (Toonz project) — project settings
- [ ] .tzc (Toonz camera) — camera settings
- [ ] Xsheet/timeline format differences

### Moho (needs research — possibly in Master or external)
- [ ] .moho / .mh (Moho project)
- [ ] Moho layer types (bone, switch, etc.)

### Other formats
- [ ] .gif, .png sequences (frame export)
- [ ] .avi, .mov, .mp4 (video import/export) — Main has flare-video crate
- [ ] .psd (Photoshop) — Main has photocraft-psd dep
- [ ] .svg (vector import)

## Missing UI features (Main vs Rust-Rewrite/Master)
- [ ] UI layout profiles (save/restore panel layouts)
- [ ] Themes (Dark, Light, Clay, Synthwave, etc.)
- [ ] OpenToonz-style xsheet view
- [ ] Onion skin
- [ ] Camera tool / camera layer
- [ ] Bone/rigging tools
- [ ] Paint bucket / ink bottle (advanced)
- [ ] Gadget tools
- [ ] Tracker
- [ ] Plastic tool

## Missing image processing (Main vs Rust ports)
- [x] Erode/dilate (ported to Rust in Rust-Rewrite: terodilate)
- [ ] Antialias (ported: tantialias)
- [x] Blur (tblur)
- [ ] Despeckle (tdespeckle)
- [ ] Distance transform (ported: tdistancetransform)
- [ ] Convolution (ported: tconvolve)
- [x] Color correction (gamma, levels done; curves open) -- crates/foundation/flare-core/src/image_ops.rs
- [ ] Resample (tresample)

## Missing OpenToonz features
- [ ] Scan & cleanup
- [ ] Inbetween
- [ ] Auto lip-sync
- [ ] Plastic deformation
- [ ] Particle effects
- [ ] Fx (effects stack)
- [ ] Render queue / batch rendering
- [ ] Palette management (advanced)

## Next steps
1. Set up Java + Ruffle submodule to build main locally
2. Diff feature sets systematically
3. Port missing features crate-by-crate
4. Each port needs tests in the new crate structure
## Key feature commits to port (from Rust-Rewrite / Master)

### UI Layout Profiles & Themes (Master)
- 70d34606f Separate UI Colour Themes from UI Layout Profiles
- 1d4585882 layouts: every profile exposes all UI options; preserve profiles; redesign Animate
- a3a12c5ed layouts: every profile exposes all UI options; preserve profiles; redesign Animate
- e556c0919 Compositor Todo and Theme WIP
- ba50b2b6c Fix include paths and update theme directory retrieval

### Flash/Animate Export (Master)
- 69e26b296 Add FlashExportCommand to flare app sources (File > Export > Flash/Animate FLA)

### AS3/ABC (Rust-Rewrite)
- 1d74ec1da AS3/ABC core in Rust: parser, decompiler, patcher, FFI, golden tests
- 1f7d9b7c4 AS3: call the Rust core directly; remove the Python sidecar

### Rust ports of image processing (Rust-Rewrite)
- b1a323825 Retire stdfx/igs_line_blur.cpp
- 2174ae3eb Retire FDTShapes, FAction, FDTSprite, FDTSounds
- e9d207c22 Retire FDTFonts, FDTText, FDTButtons
- 2fc737060 Retire tpixelutils and tcolorfunctions
- 4ccb2ab36 Retire igs_maxmin, igs_maxmin_lens_matrix, igs_maxmin_slrender
- 30c5385a0 Retire image/tzp/avl.c and port toonzrle decode kernels
- 7dce022f4 avl: align pool block, Avl_Dummy cfg
- 24bf610eb Retire SWF stream writer .cpp files
- 66a24bef2 port-w2 review nits

### OpenToonz integration (Rust-Rewrite)
- 82bfba409 Restore Flare CMake/sources/CI clobbered by upstream sync
- 79276e5ae Add Rust legacy-FLA CFBF reader with flare_cfbf_* C ABI
- fb4ca0233 WIP: Fix Build
- 360a559d3 Detect portablestuff next to executable (AppImage)
- fb8e3099d Set AA_UseDesktopOpenGL before QApplication on Windows

### Rust ports of trop (Rust-Rewrite, this session)
- bbox, tconvert, tantialias, brush, distancetransform, convolve, fracmove
- trop_utils (isOpaque, swapRB, gammaCorrect, setChannel, shrink, makeStereo, copyAndSwapRB, copy_same_size)
- trgbmscale (partial, u8)

## Findings from external projects

### minnamate (clean-room Adobe Animate in Rust)
- Status: early, vibecoded. Features: shapes, symbols, tweens, masks, guides, layer folders, parenting, filters, blend modes, sound, frame scripts, publishing to SWF/HTML5/GIF/PNG/JPEG/video
- Not yet: bones, 3D, Asset Warp, input text, multiple documents, custom workspaces
- Useful for: clean-room reference for Animate feature parity

### Next2Flash (SWF round-trip editor, MIT)
- AS3/ABC decompiler (crown jewel): abc_parser, method_decompiler, opcodes, swf_reader, swf_patcher
- swf_to_n2d.py, compile_n2d.py (~400KB SWF<->XML conversion) — still upstream-only
- bitmap_converter, char_id_allocator, cycle_detector, conversion_service, compilation_pipeline
- Already integrated in Rust-Rewrite via As3Bridge and ac_swf*.rs

### Flare-RustBase (foundation for Flare's Rust rewrite, Apache-2.0/MIT)
- This IS the base of the main branch (crates/, apps/, egui)
- Features already in main: symbols, layers, keyframes, tweens, onion skin, scenes, tools, library, actions, scripting (JS/Python/Node/Ruby), import/export (XFL, SWF, SVG, PSD, XDTS), effects, motion blur, trace bitmap, particles, sound, lip sync, masks/guides, bones, warp, width, camera, 3D, gradients, filters, blend modes, merge shapes, drawing objects, align/info panels, rulers/grid, library profiles
- Missing from main vs Flare-RustBase: (need diff)

## Stubbed re-additions (return NotYetImplemented; crates/editing/flare-tools/src/not_yet.rs)
- Tracker tool: original/flare/sources/tnztools/trackertool.cpp
- Plastic tool: original/flare/sources/tnztools/plastictool.cpp
- Hook tool: original/flare/sources/tnztools/hooktool.cpp
- Cutter tool: original/flare/sources/tnztools/cuttertool.cpp
- Stop motion: original/flare/sources/stopmotion/stopmotion.cpp
- Scan cleanup: original/flare/sources/tcleanupper/tcleanupper.cpp
- Render farm: original/flare/sources/flarefarm/tfarm/tfarmtask.cpp

## Android (from original/flare/android) — done, first version (2026-11-04)
- The old app was Qt for Android (a Gradle project loading Qt libraries). Its replacement is not
  a port of Qt: `apps/flare-android` is a `NativeActivity` cdylib (`org.flareanimate.flare`,
  minSdk 28 / targetSdk 34) around the NEW desktop shell itself — `flare-desktop`'s `ui` module
  (now a library) runs on the activity's event loop through `run_on_activity`. One
  implementation, not a mobile rewrite.
- Start/open logic (`apps/flare-android/src/open.rs`) is platform-independent and tested.
- Saved layout, preferences and recovery files work through the app's data directory
  (`flare-platform::dirs::set_app_root`), where the phone has no platform convention.
- Dialogs that need a native box (pickers, message boxes) answer as cancelled on Android:
  `flare-platform`'s dialogs are split into a native backend and a phone backend; rfd (no
  Android backend) is dependency-gated off the target.
- CI (`.github/workflows/workflow_android.yml`) builds `libflare_android.so` for
  aarch64-linux-android with the NDK (CC/AR/linker env set, which unblocked blake3);
  `continue-on-error` until its first green run. An APK: `cargo apk` on the crate's
  `AndroidManifest.xml`.
- Still dimmed on Android (recorded in docs/SURFACES.md): opening/saving from phone storage
  (SAF picker is the next step), Test Movie in Ruffle, sound on a device, touch pressure.

