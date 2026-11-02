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
