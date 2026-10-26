# Flare Rewrite Backlog (C/C++ -> Rust)

> Update 2026-10-26 (rw3): 9 more files ported (namebuilder, perlinnoise [partial], 5 tnzext potentials, tiio_jpg_exif, tgeometry [partial]); the counts below are the 2026-10-24 snapshot (77 ported) and are now 86 of 2335. Not viable this round: igs_radial_blur/igs_rotate_blur (QTransform), tvariant/tunit/tcli (class-state heavy), pli/tags, sound tsio_* (TSoundTrack), ikengine (glue over ported kernels). Candidates left: igs_fog (924, pure), tcurves/tcurveutil, flash/placement.cpp.

Generated read-only from branch `Rust-Rewrite` (HEAD 95dd5e2). Not committed. Source tree: `flare/sources`.

## Method

- Scope: all git-tracked `.c .cc .cpp .cxx .h .hpp .hh` under `flare/sources`, excluding `flare_legacy/` (6 files, note only). No thirdparty directory exists under `flare/sources`.
- Rust-ported (excluded): 77 files whose stem has a same-name `.rs` anywhere under `flare/sources` (77 of 2335 tracked). Working tree is clean: 0 tracked files are deleted.
- Remaining: 2252 files, 914347 LOC (`wc -l` physical lines).
- Qt: `#include <Q*>`/`"Q*"`/`<Qt*>` (direct, the table column) plus Q-type usage (`QString`, `Q_OBJECT`, ...) and transitive closure over project `#include`s. GL: same with `GL/`, `GL*`, `glew`, `glu`, `glext`, `tgl.h`, `QOpenGL`, plus `gl*(` / `GL*` type usage. Qt-free and GL-free are both required for batching.
- Framework-bound: a class deriving from a `T*` or `Q*` base with `virtual` in the file (8 Qt-free files; skipped from batches).
- Include resolution: suffix match, then same directory, then unique basename; 5 ambiguous includes left unresolved.
- Difficulty ordering: batch total LOC ascending (no semantic scoring). Batches are formed inside one directory, 4-6 files each (one 7-file batch where a directory has exactly 7 eligible files).

## 1. Per-directory inventory (remaining files)

| Dir | Files | LOC | Qt incl (direct) | GL incl (direct) | Qt-bound (transitive) | GL-bound (transitive) | Qt+GL-free (batchable pool) | Free LOC | Free <150 | Free 150-400 | Free >400 | Framework-bound |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| colorfx | 13 | 12390 | 3 | 3 | 10 | 8 | 2 | 107 | 2 | 0 | 0 | 0 |
| common/expressions | 4 | 2064 | 2 | 0 | 2 | 0 | 2 | 535 | 0 | 2 | 0 | 0 |
| common/flash | 42 | 17039 | 10 | 0 | 11 | 0 | 31 | 12036 | 8 | 12 | 11 | 0 |
| common/moho | 3 | 854 | 2 | 0 | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| common/psdlib | 2 | 1631 | 0 | 0 | 2 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| common/tapptools | 3 | 1950 | 1 | 0 | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| common/tcache | 1 | 2025 | 1 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| common/tcolor | 4 | 978 | 0 | 0 | 0 | 0 | 4 | 978 | 3 | 0 | 1 | 0 |
| common | 7 | 1935 | 3 | 1 | 6 | 1 | 1 | 534 | 0 | 0 | 1 | 0 |
| common/tcore | 12 | 3049 | 5 | 0 | 10 | 0 | 2 | 431 | 1 | 0 | 1 | 0 |
| common/tfx | 14 | 8435 | 4 | 1 | 14 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| common/tgame | 1 | 809 | 1 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| common/tgeometry | 3 | 1863 | 0 | 0 | 0 | 0 | 3 | 1863 | 0 | 0 | 3 | 0 |
| common/tgl | 3 | 1087 | 2 | 2 | 3 | 3 | 0 | 0 | 0 | 0 | 0 | 0 |
| common/tiio | 11 | 5716 | 1 | 0 | 6 | 1 | 4 | 1284 | 3 | 0 | 1 | 0 |
| common/timage | 2 | 117 | 0 | 0 | 2 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| common/timage_io | 2 | 1253 | 1 | 0 | 2 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| common/tipc | 4 | 1192 | 4 | 0 | 4 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| common/tmeshimage | 1 | 357 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| common/tmetaimage | 1 | 189 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| common/tparam | 14 | 4075 | 1 | 0 | 11 | 0 | 3 | 174 | 3 | 0 | 0 | 0 |
| common/traster | 1 | 469 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| common/trasterimage | 3 | 2446 | 1 | 0 | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| common/trop | 29 | 24926 | 0 | 0 | 26 | 0 | 3 | 647 | 1 | 2 | 0 | 0 |
| common/tsound | 5 | 5196 | 1 | 0 | 5 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| common/tstream | 3 | 1396 | 0 | 0 | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| common/tsystem | 9 | 3561 | 4 | 0 | 9 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| common/ttest | 1 | 517 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| common/ttoonzimage | 1 | 248 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| common/tunit | 1 | 681 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| common/tvectorimage | 16 | 19174 | 3 | 5 | 12 | 7 | 4 | 152 | 4 | 0 | 0 | 0 |
| common/tvrender | 22 | 15121 | 6 | 7 | 22 | 15 | 0 | 0 | 0 | 0 | 0 | 0 |
| common/twain | 29 | 6473 | 0 | 0 | 0 | 0 | 29 | 6473 | 24 | 2 | 3 | 0 |
| common/txsheet | 1 | 16 | 0 | 0 | 0 | 0 | 1 | 16 | 1 | 0 | 0 | 0 |
| flare | 411 | 215170 | 308 | 10 | 404 | 205 | 7 | 1921 | 5 | 0 | 2 | 0 |
| flarefarm | 65 | 22759 | 8 | 0 | 52 | 1 | 13 | 4149 | 9 | 1 | 3 | 0 |
| flarelib | 200 | 101222 | 69 | 12 | 177 | 25 | 23 | 1233 | 22 | 1 | 0 | 0 |
| flareqt | 116 | 81731 | 87 | 5 | 108 | 11 | 8 | 818 | 5 | 3 | 0 | 0 |
| flash | 21 | 5834 | 14 | 0 | 21 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| image | 101 | 44369 | 35 | 2 | 75 | 8 | 20 | 6409 | 12 | 4 | 4 | 0 |
| include | 599 | 107607 | 180 | 16 | 379 | 27 | 215 | 41696 | 157 | 46 | 12 | 8 |
| mousedragfilter | 1 | 6 | 0 | 0 | 0 | 0 | 1 | 6 | 1 | 0 | 0 | 0 |
| remote | 3 | 277 | 1 | 0 | 1 | 0 | 2 | 198 | 2 | 0 | 0 | 0 |
| sound | 11 | 1470 | 2 | 0 | 11 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| stdfx | 255 | 73058 | 41 | 28 | 201 | 35 | 54 | 16714 | 38 | 7 | 9 | 0 |
| stopmotion | 14 | 9635 | 14 | 0 | 14 | 3 | 0 | 0 | 0 | 0 | 0 | 0 |
| t32bitsrv | 7 | 1892 | 4 | 0 | 7 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| tcleanupper | 1 | 858 | 1 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| tcomposer | 3 | 1324 | 1 | 0 | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| tconverter | 1 | 594 | 1 | 0 | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| tnzbase | 16 | 5090 | 0 | 0 | 14 | 0 | 2 | 39 | 2 | 0 | 0 | 0 |
| tnzext | 36 | 10444 | 3 | 8 | 30 | 9 | 6 | 739 | 5 | 0 | 1 | 0 |
| tnztools | 123 | 81775 | 63 | 49 | 122 | 100 | 1 | 75 | 1 | 0 | 0 | 0 |
| **TOTAL** | 2252 | 914347 | 888 | 149 | 1798 | 461 | 441 | 99227 | 309 | 80 | 52 | 8 |

Note: `Qt-bound` and `GL-bound` also count files with Qt/GL usage without an include, and files that include a Qt-bound project header. Qt-free = Qt-bound false and GL-bound false.

## 2. Batches (ordered easiest first)

90 batches, 380 files, 86738 LOC. Paths relative to `flare/sources`. Batch LOC = sum of physical lines.

| ID | Dir | Files | LOC | Paths |
|---|---|---:|---:|---|
| B01 | include/tcg | 4 | 12 | `include/tcg/tcg_iterator_ops.h`, `include/tcg/tcg_list.h`, `include/tcg/tcg_macros.h`, `include/tcg/tcg_mesh.h` |
| B02 | include/tcg | 4 | 12 | `include/tcg/tcg_mesh_bgl.h`, `include/tcg/tcg_numeric_ops.h`, `include/tcg/tcg_observer_notifier.h`, `include/tcg/tcg_pixel.h` |
| B03 | include/tcg | 4 | 12 | `include/tcg/tcg_pixel_ops.h`, `include/tcg/tcg_point.h`, `include/tcg/tcg_point_ops.h`, `include/tcg/tcg_poly_ops.h` |
| B04 | include/tcg | 4 | 12 | `include/tcg/tcg_pool.h`, `include/tcg/tcg_ptr.h`, `include/tcg/tcg_sequence_ops.h`, `include/tcg/tcg_traits.h` |
| B05 | include/tcg | 5 | 15 | `include/tcg/tcg_alignment.h`, `include/tcg/tcg_base.h`, `include/tcg/tcg_consts.h`, `include/tcg/tcg_containers_reader.h`, `include/tcg/tcg_controlled_access.h` |
| B06 | include/tcg | 5 | 15 | `include/tcg/tcg_cyclic.h`, `include/tcg/tcg_edge.h`, `include/tcg/tcg_face.h`, `include/tcg/tcg_hash.h`, `include/tcg/tcg_image_ops.h` |
| B07 | stdfx | 5 | 39 | `stdfx/igs_resource_msg_from_err.cpp`, `stdfx/igs_resource_sleep_win.cpp`, `stdfx/igs_os_type.h`, `stdfx/igs_resource_msg_from_err.h`, `stdfx/iwa_cie_d65.h` |
| B08 | include | 4 | 58 | `include/tfarmplatforms.h`, `include/tquicktime.h`, `include/colorfx.h`, `include/tnzimage.h` |
| B09 | flarelib/sandor_fxs | 5 | 60 | `flarelib/sandor_fxs/Params.cpp`, `flarelib/sandor_fxs/InputParam.cpp`, `flarelib/sandor_fxs/SError.cpp`, `flarelib/sandor_fxs/STColSelPic.cpp`, `flarelib/sandor_fxs/calligraph.h` |
| B10 | flareqt | 4 | 61 | `flareqt/trepetitionguard.cpp`, `flareqt/plugin_port_interface.h`, `flareqt/plugin_tile_interface.h`, `flareqt/plugin_fxnode_interface.h` |
| B11 | include/flare4.6 | 4 | 65 | `include/flare4.6/toonzparam.h`, `include/flare4.6/trange_data.h`, `include/flare4.6/casm_msg.h`, `include/flare4.6/aliases.h` |
| B12 | common/twain | 5 | 73 | `common/twain/ttwain_winX.c`, `common/twain/ttwain_stateX.c`, `common/twain/ttwain_utilX.c`, `common/twain/ttwain_error.h`, `common/twain/ttwain_utilPD.h` |
| B13 | common/twain | 4 | 73 | `common/twain/ttwain_win.h`, `common/twain/ttwain_utilP.h`, `common/twain/ttwain_statePD.h`, `common/twain/ttwain_winPD.h` |
| B14 | include | 4 | 85 | `include/tnzsound.h`, `include/transparencycheck.h`, `include/tmachine.h`, `include/tconst.h` |
| B15 | common/twain | 4 | 89 | `common/twain/ttwain_conversion.h`, `common/twain/ttwain_utilM.c`, `common/twain/ttwain_win.c`, `common/twain/ttwain_state.h` |
| B16 | flarelib | 5 | 99 | `flarelib/scriptbinding.h`, `flarelib/cleanuppalette.h`, `flarelib/cleanupcommon.h`, `flarelib/targetcolors.cpp`, `flarelib/ikskeleton.cpp` |
| B17 | include | 4 | 106 | `include/tregionoutline.h`, `include/historytypes.h`, `include/tparamundo.h`, `include/tsweepboundary.h` |
| B18 | include/flare | 5 | 111 | `include/flare/flipbooksettings.h`, `include/flare/controlpointobserver.h`, `include/flare/currentimage.h`, `include/flare/cellpositionratio.h`, `include/flare/txshleveltypes.h` |
| B19 | include/tlin | 4 | 111 | `include/tlin/tlin.h`, `include/tlin/tlin_vector.h`, `include/tlin/tlin_basicops.h`, `include/tlin/tlin_cblas_wrap.h` |
| B20 | stdfx | 5 | 116 | `stdfx/igs_resource_sleep_win.h`, `stdfx/igs_os_type_win.h`, `stdfx/igs_motion_blur.h`, `stdfx/igs_resource_thread.cpp`, `stdfx/igs_resource_multithread.h` |
| B21 | stdfx | 4 | 121 | `stdfx/iwa_fresnel.h`, `stdfx/iwa_xyz.h`, `stdfx/igs_gaussian_blur.h`, `stdfx/igs_resource_sleep_unix.cpp` |
| B22 | include/flareqt | 4 | 137 | `include/flareqt/fxtypes.h`, `include/flareqt/trepetitionguard.h`, `include/flareqt/viewcommandids.h`, `include/flareqt/selectioncommandids.h` |
| B23 | stdfx | 4 | 140 | `stdfx/igs_resource_sleep_unix.h`, `stdfx/igs_maxmin_slrender.h`, `stdfx/igs_os_type_unix.h`, `stdfx/igs_line_blur.h` |
| B24 | tnzext | 4 | 142 | `tnzext/StrokeDeformationAdapter.cpp`, `tnzext/DraggerStatus.cpp`, `tnzext/Readme.cpp`, `tnzext/Potential.cpp` |
| B25 | image/tzp | 4 | 145 | `image/tzp/filetzup.h`, `image/tzp/toonztags.h`, `image/tzp/tiff_dir.c`, `image/tzp/tcm.h` |
| B26 | common/tvectorimage | 4 | 152 | `common/tvectorimage/tcomputeregions.h`, `common/tvectorimage/tstrokeoutline.cpp`, `common/tvectorimage/outlineApproximation.h`, `common/tvectorimage/tl2lautocloser.h` |
| B27 | include | 4 | 158 | `include/tdebugmessage.h`, `include/tstreamexception.h`, `include/tinbetween.h`, `include/tatomicvar.h` |
| B28 | include/tcg | 4 | 166 | `include/tcg/tcg_vertex.h`, `include/tcg/tcg_misc.h`, `include/tcg/point.h`, `include/tcg/image_ops.h` |
| B29 | common/twain | 4 | 170 | `common/twain/ttwain_capability.h`, `common/twain/ttwain_stateM.c`, `common/twain/ttwain_utilW.c`, `common/twain/ttwain_conversion.c` |
| B30 | stdfx | 4 | 187 | `stdfx/igs_maxmin_lens_matrix.h`, `stdfx/igs_warp.h`, `stdfx/igs_resource_msg_from_err_unix.h`, `stdfx/igs_fog.h` |
| B31 | include/flare | 4 | 190 | `include/flare/logger.h`, `include/flare/tlog.h`, `include/flare/dpiscale.h`, `include/flare/filepathproperties.h` |
| B32 | include | 4 | 200 | `include/tpersistset.h`, `include/tcg_wrap.h`, `include/trasterfxrenderdata.h`, `include/tnztypes.h` |
| B33 | flarelib/sandor_fxs | 5 | 211 | `flarelib/sandor_fxs/patternmap.h`, `flarelib/sandor_fxs/CallParam.h`, `flarelib/sandor_fxs/InputParam.h`, `flarelib/sandor_fxs/CallParam.cpp`, `flarelib/sandor_fxs/Params.h` |
| B34 | stdfx | 4 | 211 | `stdfx/igs_resource_thread_unix.cpp`, `stdfx/igs_rotate_blur.h`, `stdfx/igs_radial_blur.h`, `stdfx/igs_resource_thread.h` |
| B35 | include/flare | 4 | 227 | `include/flare/studiopalettecmd.h`, `include/flare/txsheetcolumnchange.h`, `include/flare/imagelocation.h`, `include/flare/namebuilder.h` |
| B36 | stdfx | 4 | 236 | `stdfx/igs_resource_msg_from_err_win.h`, `stdfx/igs_resource_multithread.cpp`, `stdfx/igs_hls_noise_in_camera.h`, `stdfx/igs_hsv_noise_in_camera.h` |
| B37 | include | 4 | 247 | `include/tbrush.h`, `include/cellposition.h`, `include/tparamchange.h`, `include/tstringid.h` |
| B38 | stdfx | 4 | 265 | `stdfx/igs_motion_wind.h`, `stdfx/iwa_noise1234.h`, `stdfx/igs_resource_thread_win.cpp`, `stdfx/igs_maxmin.h` |
| B39 | include/ext | 5 | 269 | `include/ext/plasticvisualsettings.h`, `include/ext/OverallDesigner.h`, `include/ext/Designer.h`, `include/ext/NotSymmetricBezierPotential.h`, `include/ext/LinearPotential.h` |
| B40 | include/flare | 4 | 286 | `include/flare/tcolumnfxset.h`, `include/flare/ikengine.h`, `include/flare/targetcolors.h`, `include/flare/ikskeleton.h` |
| B41 | include/tw | 5 | 289 | `include/tw/textlistener.h`, `include/tw/event.h`, `include/tw/colors.h`, `include/tw/stringtable.h`, `include/tw/message.h` |
| B42 | common/flash | 5 | 294 | `common/flash/F3SDK.h`, `common/flash/FDTSprite.h`, `common/flash/FFixed.h`, `common/flash/FDTSprite.cpp`, `common/flash/ZipArchive.h` |
| B43 | include | 4 | 295 | `include/tparser.h`, `include/tversion.h`, `include/tdoublekeyframe.h`, `include/tpropertytype.h` |
| B44 | include/flare4.6 | 4 | 324 | `include/flare4.6/machine.h`, `include/flare4.6/affine.h`, `include/flare4.6/tcm.h`, `include/flare4.6/tmacroX.h` |
| B45 | include/tools | 4 | 326 | `include/tools/RGBpicker.h`, `include/tools/toolcommandids.h`, `include/tools/vectoralignment.h`, `include/tools/cursors.h` |
| B46 | include | 4 | 333 | `include/tpixelcm.h`, `include/tdata.h`, `include/tfilepath_io.h`, `include/tellipticbrush.h` |
| B47 | common/twain | 4 | 335 | `common/twain/ttwain_global_def.h`, `common/twain/ttwain_stateW.c`, `common/twain/ttwain_winW.c`, `common/twain/ttwainP.h` |
| B48 | flarelib | 4 | 335 | `flarelib/logger.cpp`, `flarelib/observer.cpp`, `flarelib/levelproperties.cpp`, `flarelib/ikengine.cpp` |
| B49 | include/flare | 4 | 349 | `include/flare/toonzfolders.h`, `include/flare/captureparameters.h`, `include/flare/fxdag.h`, `include/flare/columnfan.h` |
| B50 | include/tcg | 4 | 354 | `include/tcg/vertex.h`, `include/tcg/ptr.h`, `include/tcg/edge.h`, `include/tcg/macros.h` |
| B51 | include | 4 | 368 | `include/tparamuiconcept.h`, `include/tbezier.h`, `include/timageinfo.h`, `include/ttokenizer.h` |
| B52 | include/tcg | 4 | 430 | `include/tcg/face.h`, `include/tcg/alignment.h`, `include/tcg/sequence_ops.h`, `include/tcg/containers_reader.h` |
| B53 | include | 4 | 434 | `include/texpression.h`, `include/tpluginmanager.h`, `include/tlogger.h`, `include/tspectrumparam.h` |
| B54 | include/ext | 4 | 451 | `include/ext/plastichandle.h`, `include/ext/Potential.h`, `include/ext/meshutils.h`, `include/ext/StrokeParametricDeformer.h` |
| B55 | stdfx | 4 | 468 | `stdfx/iwa_simplexnoise.h`, `stdfx/igs_ifx_common.h`, `stdfx/igs_maxmin.cpp`, `stdfx/igs_resource_msg_from_err_win.cpp` |
| B56 | include/flare | 4 | 469 | `include/flare/skeleton.h`, `include/flare/doubleparamcmd.h`, `include/flare/levelset.h`, `include/flare/strokegenerator.h` |
| B57 | include/tcg | 4 | 498 | `include/tcg/base.h`, `include/tcg/traits.h`, `include/tcg/iterator_ops.h`, `include/tcg/mesh_bgl.h` |
| B58 | flarelib/sandor_fxs | 4 | 528 | `flarelib/sandor_fxs/SError.h`, `flarelib/sandor_fxs/YOMBInputParam.h`, `flarelib/sandor_fxs/SDef.h`, `flarelib/sandor_fxs/YOMBInputParam.cpp` |
| B59 | include | 4 | 548 | `include/tcolorfunctions.h`, `include/tstopwatch.h`, `include/tiio_jpg_exif.h`, `include/tutil.h` |
| B60 | common/twain | 4 | 617 | `common/twain/ttwain.h`, `common/twain/ttwain_util.h`, `common/twain/ttwain_error.c`, `common/twain/ttwain_winM.c` |
| B61 | include/flare | 4 | 635 | `include/flare/tcamera.h`, `include/flare/tstageobjectid.h`, `include/flare/cleanupparaneters.h`, `include/flare/tproject.h` |
| B62 | include | 4 | 642 | `include/tpixelgr.h`, `include/tcurveutil.h`, `include/tdoubleparam.h`, `include/tconstwrapper.h` |
| B63 | include/flare4.6 | 4 | 696 | `include/flare4.6/readcasm.h`, `include/flare4.6/casm.h`, `include/flare4.6/toonz.h`, `include/flare4.6/tmacro.h` |
| B64 | common/flash | 5 | 754 | `common/flash/FSWFStream.h`, `common/flash/FObj.h`, `common/flash/FSound.h`, `common/flash/FDTFonts.h`, `common/flash/FDTBitmaps.h` |
| B65 | include/tlin | 4 | 754 | `include/tlin/tlin_matrix.h`, `include/tlin/tlin_sparsemat.h`, `include/tlin/tlin_superlu_wrap.h`, `include/tlin/cblas.h` |
| B66 | flareqt | 4 | 757 | `flareqt/plugin_utilities.h`, `flareqt/toonz_plugin.h`, `flareqt/toonz_params.h`, `flareqt/toonz_hostif.h` |
| B67 | include | 4 | 783 | `include/tcommon.h`, `include/tspectrum.h`, `include/tparam.h`, `include/tunit.h` |
| B68 | include/tcg | 4 | 791 | `include/tcg/pixel.h`, `include/tcg/numeric_ops.h`, `include/tcg/controlled_access.h`, `include/tcg/observer_notifier.h` |
| B69 | stdfx | 4 | 837 | `stdfx/igs_resource_msg_from_err_unix.cpp`, `stdfx/igs_maxmin_slrender.cpp`, `stdfx/texturefxP.h`, `stdfx/igs_maxmin_multithread.h` |
| B70 | include | 4 | 926 | `include/tstrokeoutline.h`, `include/tvectorrenderdata.h`, `include/tgrammar.h`, `include/tcolumnset.h` |
| B71 | include/flare4.6 | 4 | 945 | `include/flare4.6/udit.h`, `include/flare4.6/toonzproc.h`, `include/flare4.6/avl.h`, `include/flare4.6/tsound.h` |
| B72 | common/tcolor | 4 | 978 | `common/tcolor/tspectrum.cpp`, `common/tcolor/tcolorfunctions.cpp`, `common/tcolor/tpixel.cpp`, `common/tcolor/tpixelutils.cpp` |
| B73 | common/flash | 4 | 1044 | `common/flash/FPrimitive.h`, `common/flash/FObj.cpp`, `common/flash/FPrimitive.cpp`, `common/flash/FDTBitmaps.cpp` |
| B74 | common/flash | 5 | 1068 | `common/flash/FDTButtons.h`, `common/flash/FDTText.h`, `common/flash/FDTSounds.h`, `common/flash/XFLReader.h`, `common/flash/FSound.cpp` |
| B75 | include/tcg | 4 | 1080 | `include/tcg/mesh.h`, `include/tcg/hash.h`, `include/tcg/poly_ops.h`, `include/tcg/pool.h` |
| B76 | include | 4 | 1133 | `include/toutputproperties.h`, `include/tzerofinder.h`, `include/tinterval.h`, `include/tstrokedeformations.h` |
| B77 | include/flare | 4 | 1364 | `include/flare/hook.h`, `include/flare/levelproperties.h`, `include/flare/preferencesitemids.h`, `include/flare/mypainthelpers.hpp` |
| B78 | stdfx | 4 | 1514 | `stdfx/igs_maxmin_getput.h`, `stdfx/igs_maxmin_lens_matrix.cpp`, `stdfx/igs_motion_wind.cpp`, `stdfx/igs_hls_noise_in_camera.cpp` |
| B79 | common/flash | 4 | 1644 | `common/flash/FCT.h`, `common/flash/FDTButtons.cpp`, `common/flash/FDTText.cpp`, `common/flash/FDTSounds.cpp` |
| B80 | flare | 7 | 1921 | `flare/curveio.h`, `flare/studiopalettecmd.h`, `flare/scanlist.h`, `flare/moviegenerator.h`, `flare/filmstripcommand.h`, `flare/menubarcommandids.h`, `flare/bravomark.h` |
| B81 | common/flash | 4 | 1996 | `common/flash/FDTFonts.cpp`, `common/flash/FSWFStream.cpp`, `common/flash/FCT.cpp`, `common/flash/Macromedia.h` |
| B82 | include/tcg | 4 | 2141 | `include/tcg/cyclic.h`, `include/tcg/pixel_ops.h`, `include/tcg/point_ops.h`, `include/tcg/list.h` |
| B83 | stdfx | 4 | 2268 | `stdfx/igs_hsv_noise_in_camera.cpp`, `stdfx/iwa_noise1234.cpp`, `stdfx/igs_gaussian_blur.cpp`, `stdfx/iwa_simplexnoise.cpp` |
| B84 | include | 4 | 3313 | `include/tpixel.h`, `include/tcurves.h`, `include/tpixelutils.h`, `include/tgeometry.h` |
| B85 | flarefarm/tfarmclient | 7 | 3650 | `flarefarm/tfarmclient/util.h`, `flarefarm/tfarmclient/application.h`, `flarefarm/tfarmclient/util.cpp`, `flarefarm/tfarmclient/tasks.h`, `flarefarm/tfarmclient/pixmaps.h`, `flarefarm/tfarmclient/filebrowser_icons.h`, `flarefarm/tfarmclient/pixmapFiletype.h` |
| B86 | image/tzp | 4 | 3854 | `image/tzp/tmacro.h`, `image/tzp/avl.h`, `image/tzp/toonzrle.cpp`, `image/tzp/avl.c` |
| B87 | common/twain | 4 | 5116 | `common/twain/ttwain_capability.c`, `common/twain/ttwain_state.c`, `common/twain/ttwain_util.c`, `common/twain/twain.h` |
| B88 | common/flash | 4 | 5236 | `common/flash/FAction.cpp`, `common/flash/FDTShapes.h`, `common/flash/FAction.h`, `common/flash/FDTShapes.cpp` |
| B89 | stdfx | 4 | 10312 | `stdfx/igs_motion_blur.cpp`, `stdfx/igs_fog.cpp`, `stdfx/iwa_rainbow_intensity.h`, `stdfx/igs_line_blur.cpp` |
| B90 | include/flare4.6 | 4 | 15457 | `include/flare4.6/pixel.h`, `include/flare4.6/casm_node.h`, `include/flare4.6/raster.h`, `include/flare4.6/s.rop.c` |

Files in directories with fewer than 4 eligible files cannot form a same-directory batch and are not batched (see section 4).

## 3. Defer (Qt / GL / GUI / framework-bound, and residuals)

Every remaining file not in a batch is listed here by directory with its reason. Rust target from Cargo workspace members in `flare/sources/Cargo.toml` (no `flare/sources/rust` directory exists; Rust ports sit beside sources).

| Dir | Deferred files | Qt-bound | GL-only | Framework-bound | Short residual | Reason | Rust target |
|---|---:|---:|---:|---:|---:|---|---|
| colorfx | 13 | 10 | 1 | 0 | 2 | Qt (direct or transitive; some also GL); GL only; dir has <4 eligible files | none (no Cargo member) |
| common/expressions | 4 | 2 | 0 | 0 | 2 | Qt (direct or transitive; some also GL); dir has <4 eligible files | ac-script |
| common/flash | 11 | 11 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | ac-xfl |
| common/moho | 3 | 3 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| common/psdlib | 2 | 2 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| common/tapptools | 3 | 3 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| common/tcache | 1 | 1 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| common | 7 | 6 | 0 | 0 | 1 | Qt (direct or transitive; some also GL); dir has <4 eligible files | none (no Cargo member) |
| common/tcore | 12 | 10 | 0 | 0 | 2 | Qt (direct or transitive; some also GL); dir has <4 eligible files | none (no Cargo member) |
| common/tfx | 14 | 14 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| common/tgame | 1 | 1 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| common/tgeometry | 3 | 0 | 0 | 0 | 3 | dir has <4 eligible files | none (no Cargo member) |
| common/tgl | 3 | 3 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| common/tiio | 11 | 6 | 1 | 0 | 4 | Qt (direct or transitive; some also GL); GL only; dir has <4 eligible files | none (no Cargo member) |
| common/timage | 2 | 2 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| common/timage_io | 2 | 2 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| common/tipc | 4 | 4 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| common/tmeshimage | 1 | 1 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| common/tmetaimage | 1 | 1 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| common/tparam | 14 | 11 | 0 | 0 | 3 | Qt (direct or transitive; some also GL); dir has <4 eligible files | none (no Cargo member) |
| common/traster | 1 | 1 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| common/trasterimage | 3 | 3 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| common/trop | 29 | 26 | 0 | 0 | 3 | Qt (direct or transitive; some also GL); dir has <4 eligible files | none (no Cargo member) |
| common/tsound | 5 | 5 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| common/tstream | 3 | 3 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| common/tsystem | 9 | 9 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | ac-package |
| common/ttest | 1 | 1 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| common/ttoonzimage | 1 | 1 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| common/tunit | 1 | 1 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| common/tvectorimage | 12 | 12 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| common/tvrender | 22 | 22 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | ac-render |
| common/txsheet | 1 | 0 | 0 | 0 | 1 | dir has <4 eligible files | none (no Cargo member) |
| flare | 404 | 404 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | ac-app |
| flarefarm | 58 | 52 | 0 | 0 | 6 | Qt (direct or transitive; some also GL); dir has <4 eligible files | none (no Cargo member) |
| flarelib | 177 | 177 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | ac-core |
| flareqt | 108 | 108 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | ac-cli |
| flash | 21 | 21 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | ac-xfl (not a workspace member) |
| image | 93 | 75 | 6 | 0 | 12 | Qt (direct or transitive; some also GL); GL only; dir has <4 eligible files | none (no Cargo member) |
| include | 398 | 379 | 5 | 8 | 6 | Qt (direct or transitive; some also GL); GL only; framework-bound; dir has <4 eligible files | none (no Cargo member) |
| mousedragfilter | 1 | 0 | 0 | 0 | 1 | dir has <4 eligible files | none (no Cargo member) |
| remote | 3 | 1 | 0 | 0 | 2 | Qt (direct or transitive; some also GL); dir has <4 eligible files | flare_remote |
| sound | 11 | 11 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | ac-audio |
| stdfx | 201 | 201 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| stopmotion | 14 | 14 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| t32bitsrv | 7 | 7 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| tcleanupper | 1 | 1 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| tcomposer | 3 | 3 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | minnamate-cli |
| tconverter | 1 | 1 | 0 | 0 | 0 | Qt (direct or transitive; some also GL) | none (no Cargo member) |
| tnzbase | 16 | 14 | 0 | 0 | 2 | Qt (direct or transitive; some also GL); dir has <4 eligible files | none (no Cargo member) |
| tnzext | 32 | 30 | 0 | 0 | 2 | Qt (direct or transitive; some also GL); dir has <4 eligible files | minnamate-editor |
| tnztools | 123 | 122 | 0 | 0 | 1 | Qt (direct or transitive; some also GL); dir has <4 eligible files | minnamate-core |

Deferred total: 1872 files (= 2252 remaining - 380 batched). Each deferred file falls in exactly one column: Qt-bound (includes Qt+GL), GL-only, framework-bound, or short residual.

### Framework-bound files (skipped from batches)

`include/flare/observer.h`, `include/tparamcontainer.h`, `include/tparamset.h`, `include/tpersist.h`, `include/tregion.h`, `include/tsmartpointer.h`, `include/tvariant.h`, `include/tw/dragdrop.h`

### Short residual files (dir has <4 Qt/GL-free files)

`colorfx/colorfxutils.h`, `colorfx/zigzagstyles.h`, `common/tvariant.cpp`, `common/expressions/texpression.cpp`, `common/expressions/tparser.cpp`, `common/tcore/tdebugmessage.cpp`, `common/tcore/tstopwatch.cpp`, `common/tgeometry/tcurveutil.cpp`, `common/tgeometry/tgeometry.cpp`, `common/tgeometry/tcurves.cpp`, `common/tiio/tiio_jpg_exif.cpp`, `common/tiio/bmp/filebmp.h`, `common/tiio/compatibility/tfile_io.h`, `common/tiio/compatibility/tfile_io.c`, `common/tparam/tparam.cpp`, `common/tparam/tdoubleparamfile.h`, `common/tparam/tparamcontainer.cpp`, `common/trop/pixelselectors.h`, `common/trop/optimize_for_lp64.h`, `common/trop/loop_macros.h`, `common/txsheet/tcolumnset.cpp`, `flarefarm/include/tserverproxy.h`, `flarefarm/include/tbaseserver.h`, `flarefarm/tfarm/tserverproxy.cpp`, `flarefarm/tfarm/tbaseserver.cpp`, `flarefarm/tfarmcontroller/tfarmcontroller_shutdown.cpp`, `flarefarm/tfarmserver/tfarmserver_shutdown.cpp`, `image/bmp/filebmp.h`, `image/compatibility/tfile_io.h`, `image/compatibility/inforegion.h`, `image/compatibility/tfile_io.c`, `image/exr/tinyexr_otmod.h`, `image/quantel/filequantel.h`, `image/quantel/filequantelP.h`, `image/tga/filetgaP.h`, `image/tga/filetga.h`, `image/tga/filetga.c`, `image/zcc/tfile.h`, `image/zcc/tfile.cpp`, `include/stdfx/shaderfx.h`, `include/tcg/boost/permuted_range.h`, `include/tcg/boost/range_utility.h`, `include/tcg/hpp/sequence_ops.hpp`, `include/tcg/hpp/image_ops.hpp`, `include/tcg/hpp/mesh.hpp`, `mousedragfilter/mousedragfilter.h`, `remote/remotetransport.h`, `remote/remoteprotocol.h`, `tnzbase/tscanner/TScannerIO/TScannerIO.h`, `tnzbase/tscanner/TScannerIO/TUSBScannerIO.h`, `tnzext/tlin/tlin_cblas_wrap.cpp`, `tnzext/tlin/tlin_superlu_wrap.cpp`, `tnztools/tests/vectoralignment_test.cpp`

## 4. Out of scope

- `flare/sources/flare_legacy/` (6 files: CMakeLists.txt, flashexport.cpp, mainwindow.cpp, ...): legacy, not in the backlog.
- Rust-ported (excluded, 77 files): same-stem `.rs` exists. Examples: `flarelib/ikjacobian.cpp` (retired in commit c9a5075).
