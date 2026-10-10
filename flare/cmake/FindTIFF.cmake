# looks for libtiff(4.0.3 modified)
find_path(
    TIFF_INCLUDE_DIR
    NAMES
        tiffio.h
    HINTS
        ${SDKROOT}
    PATH_SUFFIXES
        tiff-4.0.3/libtiff/
# if mono or another framework with a tif library
# is installed, ignore it.
if(BUILD_ENV_APPLE)
    NO_DEFAULT_PATH
    NO_CMAKE_ENVIRONMENTPATH
    NO_CMAKE_PATH
endif()
)

find_library(
    TIFF_LIBRARY
    NAMES
        libtiff.a
    HINTS
        ${SDKROOT}
    PATH_SUFFIXES
        tiff-4.0.3/libtiff/.libs
    NO_DEFAULT_PATH
)

# fall back to system libtiff (bundled 4.0.3 no longer built on Linux/macOS)
if(NOT TIFF_LIBRARY)
    find_library(TIFF_LIBRARY NAMES tiff libtiff)
    if(TIFF_LIBRARY)
        # system lib: drop bundled 4.0.3 header (TIFF_INT8_T mismatch), use system tiff.h
        unset(TIFF_INCLUDE_DIR CACHE)
        find_path(TIFF_INCLUDE_DIR NAMES tiffio.h NO_DEFAULT_PATH PATHS /usr/include /usr/include/x86_64-linux-gnu /usr/include/aarch64-linux-gnu /usr/local/include /opt/homebrew/include)
        if(NOT TIFF_INCLUDE_DIR)
            find_path(TIFF_INCLUDE_DIR NAMES tiffio.h)
        endif()
    endif()
endif()
if(NOT TIFF_INCLUDE_DIR)
    find_path(TIFF_INCLUDE_DIR NAMES tiffio.h)
endif()

message("***** libtiff Header path:" ${TIFF_INCLUDE_DIR})
message("***** libtiff Library path:" ${TIFF_LIBRARY})

set(TIFF_NAMES ${TIFF_NAMES} TIFF)

include(FindPackageHandleStandardArgs)
FIND_PACKAGE_HANDLE_STANDARD_ARGS(TIFF
    DEFAULT_MSG TIFF_LIBRARY TIFF_INCLUDE_DIR)

if(TIFF_FOUND)
    set(TIFF_LIBRARIES ${TIFF_LIBRARY})
endif()

mark_as_advanced(
    TIFF_LIBRARY
    TIFF_INCLUDE_DIR
)
