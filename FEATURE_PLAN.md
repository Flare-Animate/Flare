# Flare Feature Implementation Plan

## Branch Strategy
- `master` - stable, deployable
- `feature/moho-support` - Moho project import
- `feature/flippaclip-ui` - Flippaclip UI profile
- `feature/next2flash-unification` - Next2Flash integration
- `feature/flash-game-engine` - Flash game creation
- `feature/rust-backend` - Gradual Rust rewrite
- `feature/android-build` - Android support
- `feature/installer-exe` - Windows installer
- `feature/flash-universal-support` - All Flash/Adobe formats
- `feature/p2p-remote` - P2P remote control
- `feature/readme-pro` - Professional README

## External Repos to Reference
1. **jpexs-decompiler** - SWF/ActionScript decompilation, ABC parsing
2. **ruffle-rs/ruffle** - SWF runtime, ActionScript VM, Flash API
3. **DaVikingCode/Citrus-Engine** - Flash game framework (AS3)
4. **benjamin-stern/Adobe-Flash--Simple-Game-Engine** - Simple game engine patterns
5. **ienaga/swf2js** - SWF to JS transpilation
6. **BowlerHatLLC/vscode-as3mxml** - AS3/MXML language support

## Phase 1: Foundation (Week 1-2)
- [ ] Set up feature branches
- [ ] Audit current Flash/SWF support gaps
- [ ] Create test suite for Flash formats
- [ ] Set up Rust FFI infrastructure

## Phase 2: Core Features (Week 2-4)
- [ ] Moho project support (parse .moho files)
- [ ] Flippaclip UI profile (layout + tool presets)
- [ ] Next2Flash full unification (AS3 bridge + runtime)
- [ ] Flash game engine (Citrus + Simple Game Engine patterns)

## Phase 3: Platform & Distribution (Week 3-5)
- [ ] Android build (Gradle + JNI/NDK)
- [ ] Windows installer (NSIS/Inno Setup)
- [ ] Universal Flash/Adobe format support

## Phase 4: Advanced (Week 4-6)
- [ ] P2P remote control (WebRTC/libp2p)
- [ ] Rust backend modules
- [ ] Professional README + docs

## Current State
- master: e95b3376f (clean, at origin/master)
- flash-xfl-cfbf-sync: a9e7c5bf2 (Flash import work)
- merge/upstream-sync: f6bcd1a28 (upstream merge work)

## Next Steps
1. Merge flash-xfl-cfbf-sync into master (preserve Flash import work)
2. Create feature branches from master
3. Begin Phase 1 work