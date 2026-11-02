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
- [ ] Erode/dilate (ported to Rust in Rust-Rewrite: terodilate)
- [ ] Antialias (ported: tantialias)
- [ ] Blur (tblur)
- [ ] Despeckle (tdespeckle)
- [ ] Distance transform (ported: tdistancetransform)
- [ ] Convolution (ported: tconvolve)
- [ ] Color correction (gamma, levels, curves)
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
