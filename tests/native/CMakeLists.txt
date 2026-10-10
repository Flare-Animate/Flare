# Tests for the native format readers in tnzcore.
#
# These link the real modules and call the real entry points. A test that
# re-implements the function under test is worse than no test: it passes when
# the shipped code is broken, and it goes stale silently.
#
# The readers are reached through their exported API. Where a reader is only
# reachable through the DLL's import table, the harness resolves it at run time
# so the test still exercises the shipped binary.
#
# Each test binary takes its fixture directory as argv[1].

cmake_minimum_required(VERSION 3.16)
project(flare_native_tests CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_AUTOMOC OFF)

find_package(Qt5Core REQUIRED)
find_package(Qt5Gui REQUIRED)

# Overridable, so the tests work in a tree laid out differently from the
# defaults rather than being silently broken.
set(FLASH_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../.." CACHE PATH
    "Flare source root")
set(BUILD "${FLASH_ROOT}/build_local" CACHE PATH
    "Flare build directory (must contain a built tnzcore)")
set(ZLIB_DIR "${FLASH_ROOT}/thirdparty/zlib-1.2.8" CACHE PATH
    "zlib source root")
set(MINIZIP "${ZLIB_DIR}/contrib/minizip" CACHE PATH
    "minizip source root")
set(ZLIB_IMPORT_LIB "${FLASH_ROOT}/vcpkg/installed/x64-windows/lib/zlib.lib"
    CACHE FILEPATH "zlib import library")

# Where CMake actually puts a target's import library differs between build
# layouts: an in-source tree writes to <build>/<target>/RelWithDebInfo, while the
# older build_local tree nests it under <build>/sources/<target>/RelWithDebInfo.
# Probe both instead of hardcoding one, so pointing BUILD at either tree works.
function(_flare_find_target_lib OUT_DIR OUT_LIB)
    foreach(_sub "sources/tnzcore" "tnzcore")
        set(_dir "${BUILD}/${_sub}/RelWithDebInfo")
        if(EXISTS "${_dir}/tnzcore.lib")
            set(${OUT_DIR} "${_dir}" PARENT_SCOPE)
            set(${OUT_LIB} "${_dir}/tnzcore.lib" PARENT_SCOPE)
            return()
        endif()
    endforeach()
    set(${OUT_DIR} "" PARENT_SCOPE)
    set(${OUT_LIB} "" PARENT_SCOPE)
endfunction()

# Fail early and clearly rather than deep inside a link error.
_flare_find_target_lib(TNZCORE_LIB_DIR TNZCORE_LIB)
if(NOT TNZCORE_LIB)
    message(FATAL_ERROR
        "tnzcore.lib not found under ${BUILD} (looked in "
        "sources/tnzcore/RelWithDebInfo and tnzcore/RelWithDebInfo). "
        "Build it first:\n"
        "  cmake --build ${BUILD} --config RelWithDebInfo --target tnzcore")
endif()
message(STATUS "native tests: using tnzcore from ${TNZCORE_LIB_DIR}")

# tnzbase sits beside tnzcore in either layout; find it rather than deriving it by
# walking up from tnzcore, which only works for the nested build_local layout.
function(_flare_find_sibling_lib OUT_LIB)
    set(_name "${ARGV1}")
    foreach(_sub "sources/${_name}" "${_name}")
        set(_dir "${BUILD}/${_sub}/RelWithDebInfo")
        if(EXISTS "${_dir}/${_name}.lib")
            set(${OUT_LIB} "${_dir}/${_name}.lib" PARENT_SCOPE)
            return()
        endif()
    endforeach()
    set(${OUT_LIB} "" PARENT_SCOPE)
endfunction()

_flare_find_sibling_lib(TNZBASE_LIB tnzbase)
if(NOT TNZBASE_LIB)
    message(FATAL_ERROR
        "tnzbase.lib not found under ${BUILD} (looked in "
        "sources/tnzbase/RelWithDebInfo and tnzbase/RelWithDebInfo). "
        "Build it first:\n"
        "  cmake --build ${BUILD} --config RelWithDebInfo --target tnzbase")
endif()
message(STATUS "native tests: using tnzbase from ${TNZBASE_LIB}")

# Same question for flareqt, needed by flareupdater_tests.
function(_flare_find_flareqt_lib OUT_LIB)
    foreach(_sub "sources/flareqt" "flareqt")
        set(_dir "${BUILD}/${_sub}/RelWithDebInfo")
        if(EXISTS "${_dir}/flareqt.lib")
            set(${OUT_LIB} "${_dir}/flareqt.lib" PARENT_SCOPE)
            return()
        endif()
    endforeach()
    set(${OUT_LIB} "" PARENT_SCOPE)
endfunction()

_flare_find_flareqt_lib(FLAREQT_LIB)
if(NOT FLAREQT_LIB)
    set(FLAREQT_LIB "")
    message(STATUS "native tests: flareqt.lib not found under ${BUILD}; "
                   "flareupdater_tests will not link")
endif()

add_executable(flash_reader_tests
    flash_reader_tests.cpp
)
target_include_directories(flash_reader_tests PRIVATE
    "${FLASH_ROOT}/flare/sources/common/flash"
    "${FLASH_ROOT}/flare/sources/common/moho"
    "${FLASH_ROOT}/flare/sources/include"
    "${ZLIB_DIR}"
    "${MINIZIP}"
)
# Optional: compile the Flash sources into the test directly, for checking edits
# before tnzcore.dll has been rebuilt. Local definitions win over the import lib.
set(FLASH_LOCAL_SRC OFF CACHE BOOL "Compile common/flash sources into flash_reader_tests")
if(FLASH_LOCAL_SRC)
    target_sources(flash_reader_tests PRIVATE
        "${FLASH_ROOT}/flare/sources/common/flash/SWFAssets.cpp"
        "${FLASH_ROOT}/flare/sources/common/flash/SWFShape.cpp")
    target_compile_definitions(flash_reader_tests PRIVATE TFLASH_EXPORTS)
endif()
target_link_libraries(flash_reader_tests PRIVATE
    Qt5::Core
    Qt5::Gui
    "${ZLIB_IMPORT_LIB}"
    "${TNZCORE_LIB}"
    "${TNZBASE_LIB}"
    kernel32.lib
)

add_executable(xfl_shape_tests
    xfl_shape_tests.cpp
)
target_include_directories(xfl_shape_tests PRIVATE
    "${FLASH_ROOT}/flare/sources/common/flash"
    "${FLASH_ROOT}/flare/sources/include"
    "${ZLIB_DIR}"
    "${MINIZIP}"
)
target_link_libraries(xfl_shape_tests PRIVATE
    Qt5::Core
    Qt5::Gui
    "${ZLIB_IMPORT_LIB}"
    "${TNZCORE_LIB}"
    "${TNZBASE_LIB}"
    kernel32.lib
)

add_executable(shape_census
    shape_census.cpp
)
target_include_directories(shape_census PRIVATE
    "${FLASH_ROOT}/flare/sources/common/flash"
    "${FLASH_ROOT}/flare/sources/include"
    "${ZLIB_DIR}"
    "${MINIZIP}"
)
target_link_libraries(shape_census PRIVATE
    Qt5::Core
    Qt5::Gui
    "${ZLIB_IMPORT_LIB}"
    "${TNZCORE_LIB}"
    "${TNZBASE_LIB}"
    kernel32.lib
)

# A probe rather than a test: it takes arbitrary paths and prints what each
# shipped reader made of them, so a gap in coverage shows up as a zero rather
# than as a pass. Needs the moho include path too, since it reports both.
add_executable(probe_samples
    probe_samples.cpp
)
target_include_directories(probe_samples PRIVATE
    "${FLASH_ROOT}/flare/sources/common/flash"
    "${FLASH_ROOT}/flare/sources/common/moho"
    "${FLASH_ROOT}/flare/sources/include"
    "${ZLIB_DIR}"
    "${MINIZIP}"
)
target_link_libraries(probe_samples PRIVATE
    Qt5::Core
    Qt5::Gui
    "${ZLIB_IMPORT_LIB}"
    "${TNZCORE_LIB}"
    "${TNZBASE_LIB}"
    kernel32.lib
)

# Decode one DefineShape body and print the result, so the SWF shape decoder can
# be checked against a real file without a test harness in the way.
add_executable(swfshape_probe
    swfshape_probe.cpp
)
target_include_directories(swfshape_probe PRIVATE
    "${FLASH_ROOT}/flare/sources/common/flash"
    "${FLASH_ROOT}/flare/sources/include"
    "${ZLIB_DIR}"
    "${MINIZIP}"
)
target_link_libraries(swfshape_probe PRIVATE
    Qt5::Core
    Qt5::Gui
    "${ZLIB_IMPORT_LIB}"
    "${TNZCORE_LIB}"
    "${TNZBASE_LIB}"
    kernel32.lib
)

add_executable(swfshape_tests
    swfshape_tests.cpp
)
# Prebuilt tnzcore can be stale: -DFLASH_LOCAL_SRC=ON tests the checked-out decoder.
option(FLASH_LOCAL_SRC "compile SWFShape.cpp into swfshape_tests" OFF)
if(FLASH_LOCAL_SRC)
  target_compile_definitions(swfshape_tests PRIVATE TFLASH_EXPORTS)
  target_link_options(swfshape_tests PRIVATE /FORCE:MULTIPLE)
  target_sources(swfshape_tests PRIVATE
      "${FLASH_ROOT}/flare/sources/common/flash/SWFShape.cpp")
endif()
target_include_directories(swfshape_tests PRIVATE
    "${FLASH_ROOT}/flare/sources/common/flash"
    "${FLASH_ROOT}/flare/sources/include"
    "${ZLIB_DIR}"
    "${MINIZIP}"
)
target_link_libraries(swfshape_tests PRIVATE
    Qt5::Core
    Qt5::Gui
    "${ZLIB_IMPORT_LIB}"
    "${TNZCORE_LIB}"
    "${TNZBASE_LIB}"
    kernel32.lib
)

add_executable(swfshape_real_tests
    swfshape_real_tests.cpp
)
# Both extractSwfShapes callers failed to link with LNK1120 when TNZCORE_LIB was a
# prebuilt DLL predating FlashAssets::extractSwfShapes -- the same staleness that
# FLASH_LOCAL_SRC already works around for swfshape_tests. Compiling the checked-out
# reader in gives them the same escape hatch, so the two shape-extraction tests can
# be run against the source without first rebuilding tnzcore.
if(FLASH_LOCAL_SRC)
  target_compile_definitions(swfshape_real_tests PRIVATE TFLASH_EXPORTS)
  target_link_options(swfshape_real_tests PRIVATE /FORCE:MULTIPLE)
  target_sources(swfshape_real_tests PRIVATE
      "${FLASH_ROOT}/flare/sources/common/flash/SWFAssets.cpp"
      "${FLASH_ROOT}/flare/sources/common/flash/SWFShape.cpp")
endif()
target_include_directories(swfshape_real_tests PRIVATE
    "${FLASH_ROOT}/flare/sources/common/flash"
    "${FLASH_ROOT}/flare/sources/include"
    "${ZLIB_DIR}"
    "${MINIZIP}"
)
target_link_libraries(swfshape_real_tests PRIVATE
    Qt5::Core
    Qt5::Gui
    "${ZLIB_IMPORT_LIB}"
    "${TNZCORE_LIB}"
    "${TNZBASE_LIB}"
    kernel32.lib
)

add_executable(swfshape_extract_tests
    swfshape_extract_tests.cpp
)
# See the note on swfshape_real_tests: this target hit the same LNK1120 against a
# stale prebuilt tnzcore.
if(FLASH_LOCAL_SRC)
  target_compile_definitions(swfshape_extract_tests PRIVATE TFLASH_EXPORTS)
  target_link_options(swfshape_extract_tests PRIVATE /FORCE:MULTIPLE)
  target_sources(swfshape_extract_tests PRIVATE
      "${FLASH_ROOT}/flare/sources/common/flash/SWFAssets.cpp"
      "${FLASH_ROOT}/flare/sources/common/flash/SWFShape.cpp")
endif()
target_include_directories(swfshape_extract_tests PRIVATE
    "${FLASH_ROOT}/flare/sources/common/flash"
    "${FLASH_ROOT}/flare/sources/include"
    "${ZLIB_DIR}"
    "${MINIZIP}"
)
target_link_libraries(swfshape_extract_tests PRIVATE
    Qt5::Core
    Qt5::Gui
    "${ZLIB_IMPORT_LIB}"
    "${TNZCORE_LIB}"
    "${TNZBASE_LIB}"
    kernel32.lib
)

add_executable(shape_export
    shape_export.cpp
)
target_include_directories(shape_export PRIVATE
    "${FLASH_ROOT}/flare/sources/common/flash"
    "${FLASH_ROOT}/flare/sources/include"
    "${ZLIB_DIR}"
    "${MINIZIP}"
)
target_link_libraries(shape_export PRIVATE
    Qt5::Core
    Qt5::Gui
    "${ZLIB_IMPORT_LIB}"
    "${TNZCORE_LIB}"
    "${TNZBASE_LIB}"
    kernel32.lib
)

add_executable(moho_reader_tests
    moho_reader_tests.cpp
)
target_include_directories(moho_reader_tests PRIVATE
    "${FLASH_ROOT}/flare/sources/common/flash"
    "${FLASH_ROOT}/flare/sources/common/moho"
    "${FLASH_ROOT}/flare/sources/include"
    "${ZLIB_DIR}"
    "${MINIZIP}"
)
target_link_libraries(moho_reader_tests PRIVATE
    Qt5::Core
    Qt5::Gui
    "${ZLIB_IMPORT_LIB}"
    "${TNZCORE_LIB}"
    "${TNZBASE_LIB}"
    kernel32.lib
)

# flareqt.lib is frequently a prebuilt DLL that predates FlareUpdater entirely:
# the two dates differed by five days, and no flareupdater*.obj existed in its build
# output, so both updater tests died with LNK1120 naming five and nine unresolved
# __imp_?FlareUpdater:: symbols. The source was present and correct; the import
# library was simply stale. Compiling flareupdater.cpp in gives the tests the same
# escape hatch FLASH_LOCAL_SRC gives the Flash readers, so they depend only on the
# checked-out source. --no-local-src still exercises the real export/ABI path.
set(FLAREUPDATER_LOCAL_SRC ON CACHE BOOL
    "compile flareupdater.cpp into the updater tests")
# FLAREQT_EXPORTS is essential. flareupdater.h sets DVAPI to DV_IMPORT_API unless it
# is defined, so without this the class is declared dllimport while being compiled into
# the test executable: MSVC warns C4273 on every method and then resolves every signal,
# staticMetaObject and vftable entry as __imp_* against a DLL that is not there.
#
# The header must be listed as a source, not just the .cpp. AUTOMOC only mocs a header
# that sits beside a same-basename source or that is itself listed as a target source;
# flareupdater.h is in flare/sources/include/flareqt while flareupdater.cpp is in
# flare/sources/flareqt, so no pair forms and the autogen set comes back empty
# ("No files found that require moc"). That produced LNK2001 on staticMetaObject,
# metaObject, qt_metacast and qt_metacall, plus LNK2019 on every signal.
function(_flareupdater_local src)
    target_sources(${src} PRIVATE
        "${FLASH_ROOT}/flare/sources/flareqt/flareupdater.cpp"
        "${FLASH_ROOT}/flare/sources/include/flareqt/flareupdater.h")
    target_compile_definitions(${src} PRIVATE FLAREQT_EXPORTS)
    set_target_properties(${src} PROPERTIES AUTOMOC ON)
endfunction()

add_executable(flareupdater_tests
    flareupdater_tests.cpp
)
if(FLAREUPDATER_LOCAL_SRC)
    _flareupdater_local(flareupdater_tests)
endif()
target_include_directories(flareupdater_tests PRIVATE
    "${FLASH_ROOT}/flare/sources/include"
)
target_link_libraries(flareupdater_tests PRIVATE
    Qt5::Core
    "${FLAREQT_LIB}"
    kernel32.lib
)

# Qt5::Network is genuinely required when FlareUpdater is compiled in rather than
# imported. checkForRelease issues a real HTTPS request, which needs
# QNetworkAccessManager; with the prebuilt flareqt.lib that arrived transitively, but
# compiled locally it does not -- the first attempt at this fix left the two targets
# on Qt5::Core alone and both then failed with C1083 "Cannot open include file:
# 'QNetworkAccessManager'". Applied here rather than at the top of the file, since
# target_link_libraries needs the target to exist first.
# Qt5::Network is genuinely required when FlareUpdater is compiled in rather than
# imported: checkForRelease issues a real HTTPS request through
# QNetworkAccessManager, which with a prebuilt flareqt.lib arrived transitively but
# compiled locally does not.
#
# Qt5_DIR was never set by this project -- lines 19-20 find Qt5Core and Qt5Gui by
# their own component names, which populates Qt5Core_DIR/Qt5Gui_DIR but leaves the
# umbrella Qt5_DIR cache entry NOTFOUND. So a bare find_package(Qt5 COMPONENTS Network)
# cannot resolve. CMAKE_PREFIX_PATH has to be seeded with the Qt root, which is
# inferred from where Qt5Core_DIR points (…/lib/cmake/Qt5Core -> three levels up).
if(FLAREUPDATER_LOCAL_SRC)
    if(Qt5Core_DIR)
        get_filename_component(_qt5core_libdir "${Qt5Core_DIR}" DIRECTORY)  # lib/cmake
        get_filename_component(_qt5core_lib    "${_qt5core_libdir}" DIRECTORY)
        get_filename_component(_qt5_root       "${_qt5core_lib}" DIRECTORY)
        if(EXISTS "${_qt5_root}/lib/cmake/Qt5Network")
            list(APPEND CMAKE_PREFIX_PATH "${_qt5_root}")
        endif()
    endif()
    find_package(Qt5 COMPONENTS Network QUIET)
    if(NOT TARGET Qt5::Network)
        message(FATAL_ERROR
            "FLAREUPDATER_LOCAL_SRC=ON needs Qt5::Network, and it could not be found "
            "(Qt5Core_DIR=${Qt5Core_DIR}). flareupdater.cpp includes "
            "<QNetworkAccessManager>. Pass -DCMAKE_PREFIX_PATH=<qt-root> on the same "
            "command line, or use -DFLAREUPDATER_LOCAL_SRC=OFF to link the prebuilt "
            "flareqt instead.")
    endif()
    target_link_libraries(flareupdater_tests PRIVATE Qt5::Network)
endif()

# The network half of FlareUpdater, against the real GitHub releases API.
# Skips cleanly when offline, so it is safe to run anywhere.
add_executable(flareupdater_net_tests
    flareupdater_net_tests.cpp
)
if(FLAREUPDATER_LOCAL_SRC)
    _flareupdater_local(flareupdater_net_tests)
endif()
target_include_directories(flareupdater_net_tests PRIVATE
    "${FLASH_ROOT}/flare/sources/include"
)
# No Qt5::Network here: the test drives FlareUpdater through its own signals and
# only needs QCoreApplication/QEventLoop from Qt5Core. Qt5::Network is linked by
# flareqt itself, so the import library is already present at run time.
target_link_libraries(flareupdater_net_tests PRIVATE
    Qt5::Core
    "${FLAREQT_LIB}"
    kernel32.lib
)
if(FLAREUPDATER_LOCAL_SRC AND TARGET Qt5::Network)
    target_link_libraries(flareupdater_net_tests PRIVATE Qt5::Network)
endif()

# P2P remote-control protocol (header-only, no deps).
add_executable(remote_protocol_tests
    remote_protocol_tests.cpp
)

# Moho import plan (header-only, no DLL).
add_executable(moho_plan_tests moho_plan_tests.cpp)
target_include_directories(moho_plan_tests PRIVATE
    "${FLASH_ROOT}/flare/sources/common/moho"
    "${FLASH_ROOT}/flare/sources/include")
target_link_libraries(moho_plan_tests PRIVATE Qt5::Core)
