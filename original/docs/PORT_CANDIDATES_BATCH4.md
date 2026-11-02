# Port candidates, batch 4 (C++ -> Rust)

Paths relative to `flare/sources/`. Filter: no sibling `.rs`, no Qt, no TRaster/TRasterPT/TRasterFx, 30-400 lines. Ordered easiest first (low risk before med, then by size). Rejected families from RUST_REWRITE.md (igs_motion_wind*, igs_maxmin*, igs_fog, igs_color_blend, igs_resource_*, noise_in_camera) excluded.

| # | Path | Lines | Purpose | Includes (summary) | Risk |
|---|---|---|---|---|---|
| 1 | common/xscopedlock.cpp | 30 | RAII lock wrapper over pimpl | own header, tthread.h | low |
| 2 | flarelib/ikskeleton.cpp | 32 | IK skeleton compute / set purpose | own header only | low |
| 3 | flarelib/iknode.cpp | 33 | IK node purpose, rotatePoint | own header only | low |
| 4 | common/tstream/tstreamexception.cpp | 37 | Stream exception message formatting | tstreamexception.h, tfilepath.h, tconvert.h, tstream.h | low |
| 5 | flarelib/logger.cpp | 39 | Singleton log buffer | flare/logger.h | low |
| 6 | flarelib/observer.cpp | 44 | TNotifier attach/detach singleton | flare/observer.h | low |
| 7 | flarelib/filepathproperties.cpp | 44 | File path properties save/load | flare/filepathproperties.h, tstream.h | low |
| 8 | flarelib/sandor_fxs/CallParam.cpp | 60 | argv parsing for Sandor call params | math.h, stdlib.h, SDef.h | low |
| 9 | flarelib/namebuilder.cpp | 69 | Level name generator (wstring) | flare/namebuilder.h, tconvert.h | low |
| 10 | tnzext/Types.cpp | 78 | OddInt value wrapper | ext/Types.h | low |
| 11 | flarelib/sandor_fxs/CallCircle.cpp | 91 | Circle sample list, qsort compare | math.h, stdlib.h, search.h | low |
| 12 | flarelib/sandor_fxs/CIL.cpp | 132 | Range-string parsing (isRange/begin/end) | string.h, math.h, SDef.h, CIL.h, algorithm | low |
| 13 | flare/predict3d.cpp | 124 | 3D point prediction | memory, predict3d.h, metnum.h | low |
| 14 | flare/metnum.cpp | 323 | Dense matrix alloc/free and numeric helpers | math.h, float.h, stdlib.h, metnum.h | low |
| 15 | common/tparam/tdoubleparamrelayproperty.cpp | 93 | Relay property over a double param | own header only | med |
| 16 | common/tapptools/tparamundo.cpp | 30 | Param undo manager singleton and onChange | tparamundo.h, tundo.h | med |
| 17 | flarelib/ikengine.cpp | 127 | IK joint tree and Jacobian solve | ikengine.h, ikjacobian.h | med |
| 18 | common/tstream/tpersistset.cpp | 57 | Persistent object set save/load | tstream.h, typeinfo, tpersistset.h | med |
| 19 | common/tparam/tparamcontainer.cpp | 115 | Param container pimpl and observer | tparamcontainer.h, tparamset.h, tparam.h | med |
| 20 | common/tparam/tnotanimatableparam.cpp | 264 | Non-animatable param (value and stream) | tnotanimatableparam.h, tstream.h | med |
| 21 | flarelib/tpinnedrangeset.cpp | 185 | Pinned frame range lookup and set | flare/tpinnedrangeset.h, tstream.h | med |
| 22 | flarelib/sandor_fxs/PatternPosition.cpp | 216 | Random pattern positions, set checks | memory, stdlib, stdio, math, string, SError.h, SDef.h | med |
| 23 | flarelib/sandor_fxs/SDirection.cpp | 391 | Direction-selection builder over a mask | memory, stdlib, algorithm, SDirection.h, SError.h | med |
| 24 | flarelib/sandor_fxs/BlurMatrix.cpp | 247 | Blur kernel matrix build and copy | windows.h (remove), time.h, vector, cmath | med |
| 25 | flarelib/sandor_fxs/EraseContour.cpp | 182 | Contour erase selection (CMAP32) | stdlib.h, search.h, math.h, EraseContour.h | med |
| 26 | flarefarm/tfarmclient/casmfileinfo.cpp | 119 | CASM frame range parsing from filename | casmfileinfo.h, stdio, string, sys/param.h | med |
| 27 | sound/raw/tsio_raw.cpp | 107 | Raw sound track reader/writer | tsio_raw.h, tsound_t.h, tsystem.h, tfilepath_io.h | med |
| 28 | image/compatibility/tnz4_cpp.cpp | 35 | IMAGE alloc/free helpers (posix IO) | tnz4.h, memory.h, stdio, unistd.h | med |
| 29 | common/tsound/tsound.cpp | 355 | TSoundTrack buffers and class code | tsound.h, tsound_t.h, tconvert.h | med |
| 30 | tnztools/modifiers/modifiersimplify.cpp | 75 | Track simplify modifier (step) | tools/modifiers/modifiersimplify.h, algorithm | med |
| 31 | common/expressions/tparser.cpp | 341 | Token-pattern expression parser | tparser.h, tgrammar.h, ttokenizer.h, tutil.h, iostream | med |
| 32 | common/expressions/texpression.cpp | 194 | Expression evaluation over grammar and parser | tgrammar.h, tparser.h, tunit.h | med |
| 33 | sound/wav/tsio_wav.cpp | 382 | WAV reader/writer | memory, tmachine.h, tsystem.h, tfilepath_io.h, tsioutils.h | med |
| 34 | common/tcore/tsmartpointer.cpp | 78 | Smart pointer thread-safety helper | tsmartpointer.h, tthreadmessage.h | med |
| 35 | common/tvrender/tpaletteutil.cpp | 130 | Palette merge helpers | tpaletteutil.h, tcolorstyles.h | med |
| 36 | common/flash/FDTSprite.cpp | 74 | Flash display-tree sprite writer | FSWFStream.h, FDTSprite.h | med |
| 37 | common/tcore/tthread_x.cpp | 279 | pthread mutex/condition wrapper (unix) | tthread.h, pthread.h | med |
| 38 | common/tvectorimage/cornerdetector.cpp | 292 | Corner detection on 3D point lists | tgeometry.h, cornerdetector.h | med |
| 39 | common/psdlib/psdutils.cpp | 213 | PSD row RLE unpack and block skip (FILE*) | zlib.h, psdutils.h | med |
| 40 | stdfx/naru_maxflow.cpp | 188 | Max-flow graph augment/adopt (whole graph) | naru_graph.h | **DONE** (batch 5; see PORTING.md) |
| 41 | flarelib/sandor_fxs/BlurMatrix.cpp | 247 | Blur kernel matrix build and copy | windows.h (remove), time.h, vector, cmath | **DONE** (batch 7) |

Notes:
- Risk "med" means a project header or OS API dependency (zlib, pthread, FILE*, tundo/tparam framework, or Windows/posix headers) must be cleared first.
- Not verified by compilation; purposes come from file names and grep of signatures.
- Skipped on purpose: igs_motion_wind_pixel.cpp (rejected family), ino_blend_add.cpp (pulls rejected igs_color_blend), cpuextensions.cpp and flarefarm/tfarmclient/util.cpp (Windows headers), FSound.cpp (Macromedia.h, unclear), stringtable.cpp (tw/ UI headers).

## What actually blocks the remaining ~1075 files (survey, Oct 16 2026)

The candidates above look individually portable; surveyed across the directory, the
blockers are structural and fall into five buckets. Knowing which bucket a file is in
before starting it saves an hour each time.

1. **C++ base class with virtuals.** `flare/cellkeyframedata.cpp` derives from
   `DvMimeData`; `stdfx/ino_blend_*.cpp` (22 files) derive from `TBlendForeBackRasterFx`
   and are pure overrides of one method. A leaf override cannot move to Rust while its
   base is C++ — the vtable lives in the base. Porting the 22 blend wrappers means
   porting the fx plugin framework first.
2. **Templates.** `flarelib/namebuilder.cpp` (`FlexibleNameCreator` derives from
   `NameCreator`), `common/tcolor/tspectrum.cpp` (`TSpectrumT<>`),
   `flarelib/sandor_fxs/{PatternPosition,Params,STColSelPic,EraseContour}` (a template
   ctor over `CSTColSelPic<P>`). The header's public API *is* the template, so it cannot
   be forwarded through a C ABI.
3. **`DEFINE_CLASS_CODE` / static data members.** `common/tcore/tdata.cpp`,
   `common/txsheet/tcolumnset.cpp`, `flareqt/trepetitionguard.cpp`. The macro defines a
   class-code integer that C++ code reads by symbol; Rust can export a function, not a
   static data member. These need either a tiny shim or a wider refactor.
4. **Toonz `RASTER`.** `flarelib/sandor_fxs/patternmap.cpp`, `common/trasterimage/*`,
   `common/tvectorimage/*`, `common/tmeshimage/*`. `RASTER` is a large C++ class with
   its own ownership rules; any file taking or returning one is downstream of porting it.
5. **Qt / platform headers in the body.** `stdfx/igs_radial_blur.cpp`,
   `igs_rotate_blur.cpp` (`QPointF`, `QTransform`), the `igs_resource_*` thread files.

The genuinely leaf files were already taken by the earlier waves plus batches 5-7
(`igs_*` kernels, `naru_maxflow`, `igs_color_blend`, `BlurMatrix`, `tmathutil`,
`tcubicbezier`, `tcolorvalue`, `runsmap`, ...). The cheap next wins are therefore not
"more small files" but the *bases* the blockers point at: `TBlendForeBackRasterFx` (unlocks
22 files), `CSTColSelPic`/`CCIL` (already half-done: `CIL.rs` exists), `RASTER`, and the
`DEFINE_CLASS_CODE` convention.
