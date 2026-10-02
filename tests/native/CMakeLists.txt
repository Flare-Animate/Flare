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

# Fail early and clearly rather than deep inside a link error.
if(NOT EXISTS "${BUILD}/sources/tnzcore/RelWithDebInfo/tnzcore.lib")
    message(FATAL_ERROR
        "tnzcore.lib not found under ${BUILD}. Build it first:\n"
        "  cmake --build ${BUILD} --config RelWithDebInfo --target tnzcore")
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
target_link_libraries(flash_reader_tests PRIVATE
    Qt5::Core
    Qt5::Gui
    "${ZLIB_IMPORT_LIB}"
    "${BUILD}/sources/tnzcore/RelWithDebInfo/tnzcore.lib"
    "${BUILD}/sources/tnzbase/RelWithDebInfo/tnzbase.lib"
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
    "${BUILD}/sources/tnzcore/RelWithDebInfo/tnzcore.lib"
    "${BUILD}/sources/tnzbase/RelWithDebInfo/tnzbase.lib"
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
    "${BUILD}/sources/tnzcore/RelWithDebInfo/tnzcore.lib"
    "${BUILD}/sources/tnzbase/RelWithDebInfo/tnzbase.lib"
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
    "${BUILD}/sources/tnzcore/RelWithDebInfo/tnzcore.lib"
    "${BUILD}/sources/tnzbase/RelWithDebInfo/tnzbase.lib"
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
    "${BUILD}/sources/tnzcore/RelWithDebInfo/tnzcore.lib"
    "${BUILD}/sources/tnzbase/RelWithDebInfo/tnzbase.lib"
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
    "${BUILD}/sources/tnzcore/RelWithDebInfo/tnzcore.lib"
    "${BUILD}/sources/tnzbase/RelWithDebInfo/tnzbase.lib"
    kernel32.lib
)
