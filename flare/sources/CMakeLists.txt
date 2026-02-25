cmake_minimum_required(VERSION 3.10)

set(CMAKE_BUILD_TYPE_INIT Release)

# Set C++17 globally 
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
message(STATUS "C++ standard: ${CMAKE_CXX_STANDARD}")

project(Flare LANGUAGES C CXX)

#-----------------------------------------------------------------------------
# Platform Specific Defaults

# list of var-names
set(_init_vars)

#-----------------------------------------------------------------------------
# Select build target
set(BUILD_TARGET_WIN       OFF)
set(BUILD_TARGET_APPLE     OFF)
set(BUILD_TARGET_UNIX      OFF)
if(WIN32)
    set(BUILD_TARGET_WIN    ON)
    message(STATUS "Target system is WIN")
elseif(APPLE)
    set(BUILD_TARGET_APPLE  ON)
    message(STATUS "Target system is APPLE")
elseif(UNIX)
    set(BUILD_TARGET_UNIX   ON)
    message(STATUS "Target system is UNIX")
else()
    set(BUILD_TARGET_UNIX   ON)
    message(WARNING "Target system unknown, set UNIX by default")
    message(STATUS "Target system is UNIX (actually unknown)")
endif()

#-----------------------------------------------------------------------------
# Select build environment

set(BUILD_ENV_MSVC     OFF)
set(BUILD_ENV_APPLE    OFF)
set(BUILD_ENV_UNIXLIKE OFF)
if(MSVC)
    set(BUILD_ENV_MSVC ON)
    if(NOT BUILD_TARGET_WIN)
        message(WARNING "Build environment is MSVC, but target system is not WIN")
    endif()
    message(STATUS "Build environment is MSVC")
elseif(CMAKE_HOST_APPLE)
    set(BUILD_ENV_APPLE ON)
    if(NOT BUILD_TARGET_APPLE)
        message(WARNING "Build environment is APPLE, but target system is not APPLE")
    endif()
    message(STATUS "Build environment is APPLE")
elseif(CMAKE_HOST_UNIX OR CMAKE_COMPILER_IS_GNUCC)
    set(BUILD_ENV_UNIXLIKE ON)
    if(NOT BUILD_TARGET_UNIX AND NOT BUILD_TARGET_WIN)
        message(WARNING "Build environment is UNIXLIKE, but target system is not UNIX or WIN")
    endif()
    message(STATUS "Build environment is UNIXLIKE")
else()
    message(ERROR "Unsupported build environment, only MSVC, APPLE and UNIXLIKE are supported")
endif()


# initialize to ON
macro(option_defaults_init)
    foreach(_var ${ARGV})
        set(${_var} ON)
        list(APPEND _init_vars "${_var}")
    endforeach()
    unset(_var)
endmacro()

# remove from namespace
macro(option_defaults_clear)
    foreach(_var ${_init_vars})
        unset(${_var})
    endforeach()
    unset(_var)
    unset(_init_vars)
endmacro()

# values to initialize WITH_****
option_defaults_init(
    _init_SYSTEM_LZO
)

# customize...
if(BUILD_ENV_MSVC)
    set(_init_SYSTEM_LZO                     OFF)
    set(_init_SYSTEM_SUPERLU                 OFF)
elseif(BUILD_ENV_APPLE)
    set(_init_SYSTEM_LZO                     OFF)
    set(_init_SYSTEM_SUPERLU                 OFF)
elseif(BUILD_ENV_UNIXLIKE)
    set(_init_SYSTEM_LZO                     ON)
    set(_init_SYSTEM_SUPERLU                 ON)
endif()


#-----------------------------------------------------------------------------
# Build Options

option(WITH_SYSTEM_LZO "Use the system LZO library instead of 'thirdparty'" ${_init_SYSTEM_LZO})
option(WITH_SYSTEM_SUPERLU "Use the system SuperLU library instead of 'thirdparty'" ${_init_SYSTEM_SUPERLU})
option(WITH_CANON "Build with Canon DSLR support - Requires Canon SDK" OFF)
option(WITH_TRANSLATION "Generate translation projects as well" ON)
option(WITH_WINTAB "(Windows only) Build with customized Qt with WinTab support. https://github.com/shun-iwasawa/qt5/releases/tag/v5.15.2_wintab" OFF)

# optionally enable MyPaint brush support; if the library or headers are unavailable we disable it
option(ENABLE_MYPaint "Enable MyPaint brush support" OFF)
# NOTE: default OFF because CI environments often lack a full libmypaint tree and
# the Windows build previously failed when header files were removed.  Users who
# want MyPaint can still enable this option manually if they provide a working
# thirdparty/libmypaint layout.


if(ENABLE_MYPaint)
    find_package(PkgConfig QUIET)
    if(PKG_CONFIG_FOUND)
        pkg_search_module(MYPAINT_LIB QUIET libmypaint libmypaint-1.3>=1.3)
    endif()
    if(NOT MYPAINT_LIB_FOUND)
        find_path(_mypaint_hdr NAMES mypaint-brush.h PATHS ${SDKROOT}/libmypaint/dist/${PLATFORM}/include/libmypaint)
        find_library(_mypaint_lib NAMES mypaint libmypaint PATHS ${SDKROOT}/libmypaint/dist/${PLATFORM})
        if(_mypaint_hdr AND _mypaint_lib)
            set(MYPAINT_LIB_INCLUDE_DIRS ${_mypaint_hdr})
            set(MYPAINT_LIB_LDFLAGS ${_mypaint_lib})
            set(MYPAINT_LIB_FOUND TRUE)
        endif()
    endif()
    if(MYPAINT_LIB_FOUND)
        message(STATUS "libmypaint support enabled")
        add_definitions(-DHAVE_MYPaint)
    else()
        message(WARNING "libmypaint not found; disabling MyPaint support")
        set(ENABLE_MYPaint OFF CACHE BOOL "" FORCE)
    endif()
endif()

# if the option has been disabled, make sure the helper macro is not defined
if(NOT ENABLE_MYPaint)
    remove_definitions(-DHAVE_MYPaint)
    # clear any cached library paths so they aren't accidentally reused
    set(MYPAINT_LIB_LDFLAGS "" CACHE STRING "" FORCE)
    set(MYPAINT_LIB_INCLUDE_DIRS "" CACHE STRING "" FORCE)
endif()

# avoid using again
option_defaults_clear()

# end option(...)

#-----------------------------------------------------------------------------
# Platform Architecture

if(BUILD_ENV_APPLE)
    if(NOT PLATFORM)
        set(PLATFORM 64)
    endif()
else() # Windows and Linux case
    if(CMAKE_SIZEOF_VOID_P EQUAL 8)
        set(PLATFORM 64)
        set(PLATFORM2 _64)
    elseif(CMAKE_SIZEOF_VOID_P EQUAL 4)
        set(PLATFORM 32)
        set(PLATFORM2)
    else()
        message("Warning : CMAKE_SIZEOF_VOID_P = ${CMAKE_SIZEOF_VOID_P}")
        set(PLATFORM 64)
        set(PLATFORM2 _64)
    endif()
endif()
message("PLATFORM:" ${PLATFORM})

#-----------------------------------------------------------------------------
# Third Party & System Libs

include(${CMAKE_CURRENT_SOURCE_DIR}/../cmake/util_compiler.cmake)

get_filename_component(SDKROOT ../../thirdparty/ ABSOLUTE)
message(STATUS "SDK Root:" ${SDKROOT})
list(APPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/../cmake")
if(BUILD_ENV_APPLE)
    set(THIRDPARTY_LIBS_HINTS "/usr/local/Cellar/" "/opt/include" ${SDKROOT})
elseif(BUILD_ENV_MSVC OR BUILD_ENV_UNIXLIKE)
    set(THIRDPARTY_LIBS_HINTS ${SDKROOT})
endif()

if(BUILD_ENV_MSVC)
    # configure the third party libs (only once) to avoid manual copies
    message(STATUS "configure the third party libs")
    if(EXISTS "${SDKROOT}/libpng-1.6.21/scripts/pnglibconf.h.prebuilt")
        file(COPY "${SDKROOT}/libpng-1.6.21/scripts/pnglibconf.h.prebuilt" DESTINATION "${SDKROOT}/libpng-1.6.21")
        file(RENAME "${SDKROOT}/libpng-1.6.21/pnglibconf.h.prebuilt" "${SDKROOT}/libpng-1.6.21/pnglibconf.h")
    endif()
    if(EXISTS "${SDKROOT}/tiff-4.0.3/libtiff/tif_config.vc.h")
        file(COPY_FILE "${SDKROOT}/tiff-4.0.3/libtiff/tif_config.vc.h" "${SDKROOT}/tiff-4.0.3/libtiff/tif_config.h")
    endif()
    if(EXISTS "${SDKROOT}/tiff-4.0.3/libtiff/tiffconf.vc.h")
        file(COPY_FILE "${SDKROOT}/tiff-4.0.3/libtiff/tiffconf.vc.h" "${SDKROOT}/tiff-4.0.3/libtiff/tiffconf.h")
    endif()
endif()

message(STATUS "Thirdparty Library Search path:" ${THIRDPARTY_LIBS_HINTS})

if(BUILD_ENV_MSVC)
    if(NOT DEFINED QT_PATH OR NOT EXISTS ${QT_PATH})
        set(QT_PATH "C:/Qt/5.15.2/msvc2019${PLATFORM2}" CACHE PATH "Qt installation directory")
        if(NOT EXISTS ${QT_PATH})
            message(FATAL_ERROR "QT_PATH not found: ${QT_PATH}. Specify QT_PATH properly.")
        endif()
    endif()
    set(QT_LIB_PATH ${QT_PATH})
    set(CMAKE_PREFIX_PATH "${CMAKE_PREFIX_PATH};${QT_PATH}/lib/cmake/")

    # sanitize any existing vcpkg applocal logs right now before projects are generated
    execute_process(COMMAND ${CMAKE_COMMAND} -E echo "Sanitizing vcpkg.applocal.log files (configure-time)...")
    execute_process(COMMAND ${CMAKE_COMMAND} -P "${CMAKE_SOURCE_DIR}/cmake/SanitizeVcpkgLogs.cmake")

    # If a local vcpkg is present, add its installed directories early so
    # find_package and find_dependency (OpenCV -> Protobuf etc.) can resolve
    # package configs during the initial configure step.
    if(EXISTS "${CMAKE_SOURCE_DIR}/vcpkg")
        if(CMAKE_SIZEOF_VOID_P EQUAL 8)
            set(_vcpkg_triplet "x64-windows")
        else()
            set(_vcpkg_triplet "x86-windows")
        endif()
        list(APPEND CMAKE_PREFIX_PATH "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}" "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/share")

        # Add vcpkg include dir so headers from vcpkg (e.g. OpenCV) are discoverable
        # even when system find_package doesn't locate them. This is a safe fallback
        # that prevents missing-header compile errors when vcpkg is present.
        include_directories(SYSTEM "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/include")

        # Also add the vcpkg lib directory to the linker search path so MSBuild
        # can resolve import-libs when its vcpkg integration points at a different
        # vcpkg root (prevents LNK1104: cannot open file 'freeglut.lib').
        link_directories("${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/lib")

        # Disable vcpkg applocal behavior - it can emit wildcard DLL entries
        # (e.g. libcrypto-*-x64.dll) which MSBuild treats as invalid paths (MSB3541).
        # We prefer to let developers manage runtime DLLs or use vcpkg's installed/bin
        # explicitly when packaging.
        set(VCPKG_APPLOCAL_DEPS OFF CACHE BOOL "Disable vcpkg applocal to avoid wildcard DLL paths" FORCE)

        # As an extra safeguard, remove any wildcard entries from existing
        # vcpkg.applocal.log files in the build tree (prevents MSBuild validation
        # errors if a previous generation left wildcard DLL names behind).
        if(EXISTS "${CMAKE_BINARY_DIR}")
            # sanitize all logs (including those at root) to avoid stale wildcards
            file(GLOB_RECURSE _vcpkg_applocal_logs "${CMAKE_BINARY_DIR}/**/vcpkg.applocal.log")
            foreach(_applog ${_vcpkg_applocal_logs})
                file(READ ${_applog} _applog_contents)
                # Remove any lines containing wildcard characters
                # previous regex was incorrect and failed to match some lines; use a simpler pattern
                string(REGEX REPLACE ".*\\*.*(\\r?\\n)?" "" _applog_sanitized "${_applog_contents}")
                if(NOT _applog_sanitized STREQUAL _applog_contents)
                    file(WRITE ${_applog} "${_applog_sanitized}")
                endif()
            endforeach()
        endif()

        # We also create a helper target so that any logs generated during the
        # build (e.g. by Qt deploy scripts) are sanitized before MSBuild tries to
        # use them.  The target is invoked early on every configuration build.
        add_custom_target(sanitize_vcpkg_logs ALL
            COMMAND ${CMAKE_COMMAND} -E echo "Sanitizing vcpkg.applocal.log files..."
            COMMAND ${CMAKE_COMMAND} -P "${CMAKE_SOURCE_DIR}/cmake/SanitizeVcpkgLogs.cmake"
            COMMENT "Removing wildcard DLL entries from applocal logs"
        )

    endif()
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} /std:c++17 /wd4251")
    set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} /std:c++17 /wd4251")
    add_definitions(
        -DVC_EXTRALEAN
        -DNOMINMAX
        -D_USE_MATH_DEFINES
    )
elseif(BUILD_ENV_APPLE)
    # Try to find Qt from Homebrew if not explicitly set
    if(NOT DEFINED QT_PATH)
        execute_process(
            COMMAND brew --prefix qt@5
            OUTPUT_VARIABLE HOMEBREW_QT_PREFIX
            OUTPUT_STRIP_TRAILING_WHITESPACE
            RESULT_VARIABLE BREW_RESULT
        )
        if(NOT BREW_RESULT EQUAL 0)
            message(FATAL_ERROR "Homebrew 'qt@5' not found. Install with 'brew install qt@5' or set QT_PATH manually.")
        endif()
        set(QT_PATH "${HOMEBREW_QT_PREFIX}/lib" CACHE PATH "Qt installation directory (from Homebrew)")
        message(STATUS "Auto-detected Qt path: ${QT_PATH}")
        set(QT_LIB_PATH "${QT_PATH}")
        list(APPEND CMAKE_PREFIX_PATH "${HOMEBREW_QT_PREFIX}/lib/cmake/Qt5")  # More precise for find_package(Qt5)
    # Conventional usage: if the QT_PATH is explicitly set
    else()
        set(QT_LIB_PATH "${QT_PATH}/")
        list(APPEND CMAKE_PREFIX_PATH "${QT_LIB_PATH}cmake/")
    endif()


    foreach(path ${CMAKE_PREFIX_PATH})
        message(STATUS "CMAKE_PREFIX_PATH: ${path}")
    endforeach()

    add_definitions(
        -DMACOSX
        -Di386
        -D__MACOS__
    )
    if(PLATFORM EQUAL 64)
        set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -m64 -stdlib=libc++ -fno-implicit-templates")
        set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -m64")
    elseif(PLATFORM EQUAL 32)
        set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -m32 -std=c++17 -stdlib=libc++ -fno-implicit-templates -D HAS_QUICKDRAW")
        set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -m32")
        set(CMAKE_LINKER_FLAGS "${CMAKE_LINKER_FLAGS} -m32")
    else()
        message(FATAL_ERROR "Invalid PLATFORM:" ${PLATFORM} ". 'PLATFORM' must be 32 or 64.")
    endif()
elseif(BUILD_ENV_UNIXLIKE)
    # Needed for correct Qt detection (handled by top-level 3.5+)
    # cmake_minimum_required(VERSION 2.8.12)  # Redundant and triggers <3.5 error
    set(PRELOAD_VARIABLE "LD_LIBRARY_PATH")
    if(CMAKE_SYSTEM_NAME MATCHES "Linux")
        add_definitions(-DLINUX)
    elseif(CMAKE_SYSTEM_NAME MATCHES "Haiku")
        add_definitions(-DHAIKU)
        set(PRELOAD_VARIABLE "LIBRARY_PATH")
    elseif(CMAKE_SYSTEM_NAME MATCHES "^.*BSD$|DragonFly")
        set(BUILD_TARGET_BSD ON)
        add_definitions(-D_BSD_SOURCE -DFREEBSD)
        include_directories("/usr/local/include/")
        message(STATUS "Configuring for BSD system")
    elseif(NOT BUILD_TARGET_WIN)
        message(WARNING "Support for generic Unix (Not Apple or Linux) isn't yet working!")
    endif()

    set(CMAKE_CXX_STANDARD 17)

    find_package(Qt5Widgets)

    set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS}")
    if (NOT (CMAKE_CXX_COMPILER_ID STREQUAL "Clang"))
        set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -lstdc++")
    endif()
endif()

add_definitions(
    -DQT_GUI_LIB
    -DQT_NETWORK_LIB
    -DQT_CORE_LIB
    -DQT_SHARED
)

# Find includes in corresponding build directories
set(CMAKE_INCLUDE_CURRENT_DIR ON)

if(BUILD_ENV_APPLE)
    set(CMAKE_MACOSX_RPATH ON)
    set(CMAKE_SKIP_BUILD_RPATH FALSE)
    set(CMAKE_BUILD_WITH_INSTALL_RPATH FALSE)
    set(CMAKE_INSTALL_RPATH ${QT_LIB_PATH})
    set(CMAKE_INSTALL_RPATH_USE_LINK_PATH TRUE)
    set(CMAKE_FIND_FRAMEWORK LAST)
    list(FIND CMAKE_PLATFORM_IMPLICIT_LINK_DIRECTORIES "${QT_LIB_PATH}" isSystemDir)
    if("${isSystemDir}" STREQUAL "-1")
        set(CMAKE_INSTALL_RPATH "${QT_LIB_PATH}")
    endif()
endif()

include_directories(
    include
)

# Find the QtWidgets library
find_package(Qt5 REQUIRED
    Core
    Gui
    Network
    OpenGL
    Svg
    Xml
    Widgets
    PrintSupport
    LinguistTools
    Multimedia
    MultimediaWidgets
    SerialPort
    UiTools
)

# QtScript is optional; detect separately so builds can proceed without it
find_package(Qt5Script QUIET)
if(Qt5Script_FOUND)
    set(WITH_QT_SCRIPT ON)
    add_definitions(-DWITH_QT_SCRIPT)
    message(STATUS "Qt5Script found - enabling script features")
else()
    set(WITH_QT_SCRIPT OFF)
    message(WARNING "Qt5Script not found - building without script support")
endif()

if(NOT Qt5Core_FOUND)
  message(FATAL_ERROR "Qt5Core not found check paths")
endif()
message(STATUS "Qt5 Modules: ${Qt5Core_VERSION}")

set(QT_MINIMUM_VERSION 5.5.0)
if(Qt5Core_VERSION VERSION_LESS QT_MINIMUM_VERSION)
    MESSAGE(FATAL_ERROR "Minimum supported Qt version: ${QT_MINIMUM_VERSION}.
    Installed version: ${Qt5Core_VERSION}")
endif()
unset(QT_MINIMUM_VERSION)

if(BUILD_ENV_MSVC)
    include_directories(
        SYSTEM
        ${SDKROOT}/glut/3.7.6/include
        ${SDKROOT}/zlib-1.2.8
        #${SDKROOT}/LibJPEG/jpeg-9
        ${SDKROOT}/libjpeg-turbo/include
    )
    add_definitions(
        -DGLUT_NO_LIB_PRAGMA
    )
endif()


if(PLATFORM EQUAL 64)
    if(BUILD_ENV_UNIXLIKE)
        find_package(OpenCV REQUIRED)
    else() # Windows and MacOS case
        # Try usual system/OpenCV first (quietly), then fallback to vcpkg-installed OpenCV when available
        # Disable automatic OpenCV config-file discovery when vcpkg's OpenCV causes
        # configure-time policy/dependency issues. Setting this allows building
        # without OpenCV support (optional features disabled).
        set(CMAKE_DISABLE_FIND_PACKAGE_OpenCV ON CACHE BOOL "Temporarily disable OpenCV find")
        # find_package(OpenCV 4.1 QUIET)  # disabled (use vcpkg/manual fallback instead)
        if(NOT OpenCV_FOUND)
            # Look for a vcpkg installation either via VCPKG_ROOT env or a local ./vcpkg folder
            if(DEFINED ENV{VCPKG_ROOT} OR EXISTS "${CMAKE_SOURCE_DIR}/vcpkg")
                if(DEFINED ENV{VCPKG_ROOT})
                    set(_vcpkg_root $ENV{VCPKG_ROOT})
                else()
                    set(_vcpkg_root "${CMAKE_SOURCE_DIR}/vcpkg")
                endif()
                if(CMAKE_SIZEOF_VOID_P EQUAL 8)
                    set(_vcpkg_triplet "x64-windows")
                else()
                    set(_vcpkg_triplet "x86-windows")
                endif()
                # Add vcpkg installed dirs to CMAKE_PREFIX_PATH so OpenCV and its dependencies are discoverable
                list(APPEND CMAKE_PREFIX_PATH "${_vcpkg_root}/installed/${_vcpkg_triplet}" "${_vcpkg_root}/installed/${_vcpkg_triplet}/share")
                # Also add the vcpkg lib directory to the linker search path so MSBuild can
                # resolve import-libs when its vcpkg integration points at a different root.
                link_directories("${_vcpkg_root}/installed/${_vcpkg_triplet}/lib")
                # Proactively set Protobuf_DIR so OpenCV's find_dependency can locate protobuf from vcpkg
                if(NOT DEFINED Protobuf_DIR)
                    set(Protobuf_DIR "${_vcpkg_root}/installed/${_vcpkg_triplet}/share/protobuf")
                endif()
                # Try OpenCV from vcpkg installed tree
                # find_package(OpenCV 4.1 CONFIG PATHS "${_vcpkg_root}/installed/${_vcpkg_triplet}/share/opencv4" QUIET)
                # (disabled) use manual vcpkg fallback below instead
            endif()
        endif()
        if(NOT OpenCV_FOUND)
            # Fallback: if vcpkg has OpenCV installed, use its include/lib paths
            if(EXISTS "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/lib/opencv_core4.lib")
                message(STATUS "Using vcpkg-installed OpenCV (fallback): ${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}")
                set(OpenCV_FOUND ON)
                set(OpenCV_INCLUDE_DIRS "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/include/opencv4")
                set(OpenCV_LIBS
                    "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/lib/opencv_core4.lib"
                    "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/lib/opencv_imgproc4.lib"
                    "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/lib/opencv_imgcodecs4.lib"
                    "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/lib/opencv_highgui4.lib"
                    "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/lib/opencv_videoio4.lib"
                    "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/lib/opencv_calib3d4.lib"
                    "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/lib/opencv_features2d4.lib"
                    "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/lib/opencv_flann4.lib"
                    "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/lib/opencv_video4.lib"
                    "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/lib/opencv_dnn4.lib"
                )
            else()
                message(WARNING "OpenCV not found — building without OpenCV support. Some features will be disabled.")
                set(OpenCV_FOUND OFF)
                set(OpenCV_LIBS "")
            endif()
        endif()
    endif()
    #include_directories(
    #    ${SDKROOT}/libjpeg-turbo64/include
    #)
    if(WITH_CANON)
        include_directories(
            ${SDKROOT}/canon/Header
        )
    endif()
endif()

if(OpenCV_FOUND)
    include_directories(SYSTEM ${OpenCV_INCLUDE_DIRS})
    message(STATUS "OpenCV include dirs: ${OpenCV_INCLUDE_DIRS}")
endif()

get_target_property(QtCore_location Qt5::Core LOCATION)
get_target_property(QtWidget_location Qt5::Widgets LOCATION)
message(STATUS "Qt Core Location:" ${QtCore_location})

macro(_find_toonz_library OUT_LIB_LIST IN_LIB_LIST)
    set(OUTLIST "")
    foreach(arg ${IN_LIB_LIST})
        unset(COREPATH CACHE)
        # Even if you try to find, CMake might not work properly, so we just generate and return the full path
        if(BUILD_ENV_APPLE)
            set(LIBPATH "\$<TARGET_FILE:${arg}>")
        else()
            set(LIBPATH ${arg})
        endif()
        set(OUTLIST ${OUTLIST} ${LIBPATH})
    endforeach()
    set(${OUT_LIB_LIST} ${OUTLIST})
endmacro()

set(TNZSTDFX_Location)
set(TFARM_Location)

set(QT_LIB)  # avoid warning

if(BUILD_ENV_MSVC)
    set(MSVC_LIB_VERSION 2015)

    # Ensure OpenGL target is available for FreeGLUT
    find_package(OpenGL REQUIRED)

    # Prefer system or vcpkg-provided GLUT (freeglut) when available
    find_package(FreeGLUT QUIET)
    if(TARGET FreeGLUT::freeglut)
        set(GLUT_LIB FreeGLUT::freeglut)
    else()
        find_package(GLUT QUIET)
        if(TARGET GLUT::GLUT)
            set(GLUT_LIB GLUT::GLUT)
        else()
            set(GLUT_LIB ${SDKROOT}/glut/3.7.6/lib/glut${PLATFORM}.lib)
        endif()
    endif()
    set(GL_LIB opengl32.lib)
    # Prefer vcpkg/system ZLIB when available, otherwise fall back to bundled thirdparty
    find_package(ZLIB QUIET)
    if(TARGET ZLIB::ZLIB)
        set(Z_LIB ZLIB::ZLIB)
    elseif(DEFINED ZLIB_LIBRARY AND DEFINED ZLIB_INCLUDE_DIR)
        set(Z_LIB ${ZLIB_LIBRARY})
    else()
        set(Z_LIB
            optimized ${SDKROOT}/zlib-1.2.8/lib/zlib-1.2.8_${MSVC_LIB_VERSION}${PLATFORM2}.lib
            debug ${SDKROOT}/zlib-1.2.8/lib/zlib-1.2.8_${MSVC_LIB_VERSION}${PLATFORM2}d.lib
        )
    endif()
    # Prefer system/vcpkg libjpeg; fall back to bundled thirdparty
    find_package(JPEG QUIET)
    if(TARGET JPEG::JPEG)
        set(JPEG_LIB JPEG::JPEG)
    elseif(DEFINED JPEG_LIBRARIES)
        set(JPEG_LIB ${JPEG_LIBRARIES})
    else()
        set(JPEG_LIB ${SDKROOT}/libjpeg-turbo/lib/jpeg-static_${MSVC_LIB_VERSION}${PLATFORM2}.lib)
    endif()
    # TurboJPEG fallback
    find_path(TURBOJPEG_INCLUDE_DIR turbojpeg.h PATHS ${CMAKE_SOURCE_DIR}/vcpkg/installed/x64-windows/include NO_DEFAULT_PATH)
    if(TURBOJPEG_INCLUDE_DIR)
        set(TURBOJPEG_LIB "${CMAKE_SOURCE_DIR}/vcpkg/installed/x64-windows/lib/turbojpeg.lib")
    else()
        set(TURBOJPEG_LIB ${SDKROOT}/libjpeg-turbo/lib/turbojpeg-static_${MSVC_LIB_VERSION}${PLATFORM2}.lib)
    endif()
    set(TIFF_INCLUDE_DIR ${SDKROOT}/tiff-4.0.3/libtiff)
    set(TIFF_LIB
        optimized ${SDKROOT}/tiff-4.0.3/lib/LibTIFF-4.0.3_${MSVC_LIB_VERSION}${PLATFORM2}.lib
        debug  ${SDKROOT}/tiff-4.0.3/lib/LibTIFF-4.0.3_${MSVC_LIB_VERSION}${PLATFORM2}d.lib
    )
    # Prefer vcpkg-installed libtiff when bundled import library/header are missing
    if(EXISTS "${CMAKE_SOURCE_DIR}/vcpkg")
        if(NOT DEFINED _vcpkg_triplet)
            if(CMAKE_SIZEOF_VOID_P EQUAL 8)
                set(_vcpkg_triplet "x64-windows")
            else()
                set(_vcpkg_triplet "x86-windows")
            endif()
        endif()
        find_file(_vcpkg_tiff_header NAMES tiffio.h PATHS "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/include" NO_DEFAULT_PATH)
        find_file(_vcpkg_tiff_lib NAMES tiff.lib TIFF.lib PATHS "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/lib" NO_DEFAULT_PATH)
        if(_vcpkg_tiff_header AND _vcpkg_tiff_lib)
            # Prefer vcpkg libtiff only if it exposes the 64-bit RGBA helpers used by
            # our TIFF reader. Some vcpkg builds omit TIFFReadRGBAStrip_64 /
            # TIFFReadRGBATile_64 and that causes unresolved externals at link time;
            # in that case fall back to the bundled thirdparty copy which provides
            # the aliases.
            file(READ "${_vcpkg_tiff_header}" _vcpkg_tiff_header_head LIMIT 8192)
            string(FIND "${_vcpkg_tiff_header_head}" "TIFFReadRGBAStrip_64" _vcpkg_tiff_has_64)
            if(NOT _vcpkg_tiff_has_64 EQUAL -1)
                set(TIFF_INCLUDE_DIR "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/include")
                set(TIFF_LIB "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/lib/tiff.lib")
                message(STATUS "Using vcpkg-installed libtiff: ${TIFF_LIB}")
            else()
                message(WARNING "vcpkg-installed libtiff lacks TIFFReadRGBA 64-bit helpers; using bundled thirdparty libtiff instead.")
            endif()
        endif()
    endif()
    set(PNG_LIB
        optimized ${SDKROOT}/libpng-1.6.21/lib/libpng16_${MSVC_LIB_VERSION}${PLATFORM2}.lib
        debug  ${SDKROOT}/libpng-1.6.21/lib/libpng16_${MSVC_LIB_VERSION}${PLATFORM2}d.lib
    )
    # Prefer vcpkg-installed libpng when the bundled import lib/header are missing or invalid
    if(EXISTS "${CMAKE_SOURCE_DIR}/vcpkg")
        if(NOT DEFINED _vcpkg_triplet)
            if(CMAKE_SIZEOF_VOID_P EQUAL 8)
                set(_vcpkg_triplet "x64-windows")
            else()
                set(_vcpkg_triplet "x86-windows")
            endif()
        endif()
        find_file(_vcpkg_png_header NAMES png.h pngconf.h PATHS "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/include" NO_DEFAULT_PATH)
        find_file(_vcpkg_png_lib NAMES libpng16.lib png16.lib PNG16.lib PATHS "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/lib" NO_DEFAULT_PATH)
        if(_vcpkg_png_header AND _vcpkg_png_lib)
            set(PNG_LIB "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/lib/libpng16.lib")
            set(PNG_INCLUDE_DIRS "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/include")
            message(STATUS "Using vcpkg-installed libpng: ${PNG_LIB}")
        endif()
    endif()
    set(GLEW_LIB ${SDKROOT}/glew/glew-1.9.0/lib/glew${PLATFORM}.lib)
    # Prefer vcpkg-installed GLEW when available (fixes missing/corrupt thirdparty import libs)
    if(EXISTS "${CMAKE_SOURCE_DIR}/vcpkg")
        if(NOT DEFINED _vcpkg_triplet)
            if(CMAKE_SIZEOF_VOID_P EQUAL 8)
                set(_vcpkg_triplet "x64-windows")
            else()
                set(_vcpkg_triplet "x86-windows")
            endif()
        endif()
        find_file(_vcpkg_glew_lib NAMES glew.lib glew32.lib glew64.lib glew64s.lib glew64mx.lib PATHS "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/lib" NO_DEFAULT_PATH)
        if(_vcpkg_glew_lib)
            set(GLEW_LIB "${_vcpkg_glew_lib}")
            message(STATUS "Using vcpkg-installed GLEW: ${GLEW_LIB}")
        endif()
    endif()
    # Prefer vcpkg-installed lz4 when available, else use bundled thirdparty
    find_package(lz4 QUIET)
    if(TARGET lz4::lz4)
        set(LZ4_LIB lz4::lz4)
    else()
        set(LZ4_LIB ${SDKROOT}/Lz4/Lz4_131/lz4_${PLATFORM}.lib)
    endif()
    set(SUPERLU_LIB ${SDKROOT}/superlu/SuperLU_${MSVC_LIB_VERSION}_${PLATFORM}.lib)
    # If the bundled SuperLU import library is a Git-LFS pointer, prefer a vcpkg-provided
    # SuperLU or add an ExternalProject to build SuperLU from source so builds don't
    # fail when the repo contains LFS pointer placeholders.
    if(EXISTS "${SUPERLU_LIB}")
        file(READ "${SUPERLU_LIB}" _superlu_lib_head LIMIT 2048)
        string(FIND "${_superlu_lib_head}" "version https://git-lfs.github.com/spec/v1" _is_lfs)
        if(NOT _is_lfs EQUAL -1)
            message(WARNING "Detected Git-LFS pointer for SuperLU import library; will try vcpkg or build from source.")
            # try vcpkg-installed SuperLU first
            if(EXISTS "${CMAKE_SOURCE_DIR}/vcpkg")
                if(CMAKE_SIZEOF_VOID_P EQUAL 8)
                    set(_vcpkg_triplet "x64-windows")
                else()
                    set(_vcpkg_triplet "x86-windows")
                endif()
                find_file(_vcpkg_superlu_lib NAMES superlu.lib libsuperlu.lib PATHS "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/lib" NO_DEFAULT_PATH)
                if(_vcpkg_superlu_lib)
                    set(SUPERLU_LIB "${_vcpkg_superlu_lib}")
                    message(STATUS "Using vcpkg-installed SuperLU: ${SUPERLU_LIB}")
                    # If METIS is available in vcpkg, prefer it for SuperLU's ordering
                    find_file(_vcpkg_metis_lib NAMES metis.lib libmetis.lib PATHS "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/lib" NO_DEFAULT_PATH)
                    if(_vcpkg_metis_lib)
                        set(METIS_LIB "${_vcpkg_metis_lib}")
                        message(STATUS "Using vcpkg-installed METIS: ${METIS_LIB}")
                    endif()
                endif()
            endif()
            # If still not found, add an ExternalProject to build SuperLU from the bundled source
            if(NOT _vcpkg_superlu_lib AND NOT IS_DIRECTORY "${SUPERLU_LIB}")
                message(STATUS "Adding ExternalProject 'superlu_local' — building SuperLU from thirdparty source")
                include(ExternalProject)
                ExternalProject_Add(superlu_local
                    SOURCE_DIR "${CMAKE_SOURCE_DIR}/thirdparty/superlu/SuperLU_4.1/prj/SuperLU"
                    CONFIGURE_COMMAND ""
                    BUILD_COMMAND ${CMAKE_BUILD_TOOL} "${CMAKE_SOURCE_DIR}/thirdparty/superlu/SuperLU_4.1/prj/SuperLU/SuperLU.sln" /p:Configuration=Release /m
                    INSTALL_COMMAND ""
                    BUILD_BYPRODUCTS "${CMAKE_SOURCE_DIR}/thirdparty/superlu/SuperLU_2015_64.lib"
                )
                set(SUPERLU_LIB "${CMAKE_SOURCE_DIR}/thirdparty/superlu/SuperLU_2015_64.lib")
            endif()
        endif()
    endif()

    set(OPENBLAS_LIB ${SDKROOT}/openblas/libopenblas_${PLATFORM}.lib)
    if(EXISTS "${OPENBLAS_LIB}")
        file(READ "${OPENBLAS_LIB}" _openblas_head LIMIT 2048)
        string(FIND "${_openblas_head}" "version https://git-lfs.github.com/spec/v1" _is_lfs_openblas)
        if(NOT _is_lfs_openblas EQUAL -1)
            message(WARNING "Detected Git-LFS pointer for OpenBLAS import library; will try vcpkg-installed OpenBLAS")
            if(EXISTS "${CMAKE_SOURCE_DIR}/vcpkg")
                if(CMAKE_SIZEOF_VOID_P EQUAL 8)
                    set(_vcpkg_triplet "x64-windows")
                else()
                    set(_vcpkg_triplet "x86-windows")
                endif()
                find_file(_vcpkg_openblas_lib NAMES libopenblas.lib openblas.lib PATHS "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/lib" NO_DEFAULT_PATH)
                if(_vcpkg_openblas_lib)
                    set(OPENBLAS_LIB "${_vcpkg_openblas_lib}")
                    message(STATUS "Using vcpkg-installed OpenBLAS: ${OPENBLAS_LIB}")
                endif()
            endif()
        endif()
    endif()
    set(USB_LIB)  # unused

    if(PLATFORM EQUAL 32)
        set(QT_LIB ${SDKROOT}/quicktime/QT73SDK/Libraries/QTMLClient.lib)
    endif()
    set(MYPAINT_LIB_INCLUDE_DIRS ${SDKROOT}/libmypaint/dist/${PLATFORM}/include/libmypaint)
    set(MYPAINT_LIB_LDFLAGS ${SDKROOT}/libmypaint/dist/${PLATFORM}/libmypaint.lib)

    # If bundled import library is missing or is a Git-LFS pointer, prefer vcpkg-installed libmypaint
    if(NOT EXISTS "${MYPAINT_LIB_LDFLAGS}")
        set(_is_lfs_mypaint 1)
    else()
        file(READ "${MYPAINT_LIB_LDFLAGS}" _mypaint_head LIMIT 2048)
        string(FIND "${_mypaint_head}" "version https://git-lfs.github.com/spec/v1" _is_lfs_mypaint)
    endif()

    if(_is_lfs_mypaint GREATER -1)
        if(EXISTS "${CMAKE_SOURCE_DIR}/vcpkg")
            if(NOT DEFINED _vcpkg_triplet)
                if(CMAKE_SIZEOF_VOID_P EQUAL 8)
                    set(_vcpkg_triplet "x64-windows")
                else()
                    set(_vcpkg_triplet "x86-windows")
                endif()
            endif()
            find_file(_vcpkg_mypaint_lib NAMES libmypaint.lib mypaint.lib PATHS "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/lib" NO_DEFAULT_PATH)
            if(_vcpkg_mypaint_lib)
                set(MYPAINT_LIB_LDFLAGS "${_vcpkg_mypaint_lib}")
                set(MYPAINT_LIB_INCLUDE_DIRS "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/include")
                message(STATUS "Using vcpkg-installed libmypaint: ${MYPAINT_LIB_LDFLAGS}")
            endif()
        endif()
    endif()

    if(PLATFORM EQUAL 64)
        if (WITH_CANON)
            set(CANON_LIB ${SDKROOT}/canon/library/EDSDK.lib)
        endif()
    endif()
elseif(BUILD_ENV_APPLE)
    find_library(GLUT_LIB GLUT)
    find_library(GL_LIB OpenGL)
    find_library(Z_LIB z)

    find_package(TIFF REQUIRED)
    set(TIFF_LIB ${TIFF_LIBRARY})
    # the libraries have not .pc nor preferred Find*.cmake. use custom modules.
    find_package(PNG REQUIRED)
    message("******* libpng:" ${PNG_LIBRARY})
    set(PNG_LIB ${PNG_LIBRARY})
    find_package(GLEW)

    find_package(SuperLU REQUIRED)
    set(SUPERLU_INCLUDE_DIR ${SUPERLU_INCLUDE_DIR}/superlu)
    set(SUPERLU_LIB ${SUPERLU_LIBRARY})
    message("SuperLU:" ${SUPERLU_INCLUDE_DIR})

    find_package(JPEG REQUIRED)

    # the libraries have .pc
    find_package(PkgConfig)
    if(GLEW-NOTFOUND)
        pkg_check_modules(GLEW REQUIRED glew)
    endif()
    if (TARGET GLEW::GLEW)
        set(GLEW_LIB GLEW::GLEW)
    else()
        set(GLEW_LIB ${GLEW_LIBRARIES})
    endif()

    pkg_check_modules(LZ4_LIB REQUIRED liblz4)

    message("**************** lz4 lib:" ${LZ4_LIB_INCLUDE_DIRS})
    find_path(LZ4_STATIC lz4frame_static.h PATH ${LZ4_LIB_INCLUDE_DIRS})
    if(LZ4_STATIC-FOUND)
        add_definitions(-DLZ4_STATIC)
    endif()

    pkg_check_modules(USB_LIB REQUIRED libusb-1.0)
    set(OPENBLAS_LIB) # unused?
    if(PLATFORM EQUAL 32)
        find_library(QT_LIB QuickTime)
    endif()

    set(JPEG_LIB ${JPEG_LIBRARY})
    set(LZ4_LIB ${LZ4_LIB_LDFLAGS})

    find_package(LZO REQUIRED)
    message("LZO:" ${LZO_INCLUDE_DIR})

    pkg_check_modules(MYPAINT_LIB REQUIRED libmypaint)

    if(PLATFORM EQUAL 64)
        pkg_check_modules(TURBOJPEG REQUIRED libturbojpeg)
        find_library(TURBOJPEG_LIB turbojpeg ${TURBOJPEG_LIBRARY_DIRS})
        message("**************** turbojpeg lib:" ${TURBOJPEG_LIB})

        if (WITH_CANON)
            set(CANON_LIB ${SDKROOT}/canon/Framework/EDSDK.framework)
        endif()
    endif()

elseif(BUILD_ENV_UNIXLIKE)
    if(BUILD_TARGET_WIN)
        find_library(GL_LIB opengl32)
        find_library(GLU_LIB glu32)
        find_library(GLUT_LIB NAMES glut freeglut)
    else()
        find_library(GL_LIB GL)
        find_library(GLU_LIB GLU)
        find_library(GLUT_LIB glut)
    endif()
    find_library(Z_LIB z)

    if(NOT BUILD_TARGET_BSD)
        find_package(TIFF REQUIRED)
    endif()
    find_package(PNG REQUIRED)
    set(PNG_LIB ${PNG_LIBRARY})
    message("******* libpng:" ${PNG_LIB})
    find_package(GLEW)

    find_package(SuperLU REQUIRED)
    set(SUPERLU_LIB ${SUPERLU_LIBRARY})

    find_package(JPEG REQUIRED)

    # the libraries have .pc
    find_package(PkgConfig)

    # This is not required for Flare itself, but libtiff will pick it up if
    # present and libimage won't be aware causing linker to fail
    pkg_check_modules(LZMA liblzma)
    set(TIFF_LIB ${TIFF_LIBRARY} ${LZMA_LIBRARIES})

    if(GLEW-NOTFOUND)
        pkg_check_modules(GLEW REQUIRED glew)
    endif()
    set(GLEW_LIB ${GLEW_LIBRARIES})

    pkg_check_modules(LZ4_LIB REQUIRED liblz4)

    message("**************** lz4 lib:" ${LZ4_LIB_INCLUDE_DIRS})
    find_path(LZ4_STATIC lz4frame_static.h PATH ${LZ4_LIB_INCLUDE_DIRS})
    if(LZ4_STATIC-FOUND)
        add_definitions(-DLZ4_STATIC)
    endif()

    # TODO: check for them properly in some sane order with pkg-config
    find_library(OPENBLAS_LIB NAMES cblas gslcblas openblas blas)
    message("OpenBLAS:" ${OPENBLAS_LIB})

    set(JPEG_LIB ${JPEG_LIBRARY})
    set(LZ4_LIB ${LZ4_LIB_LDFLAGS})

    find_package(LZO REQUIRED)
    message("LZO:" ${LZO_INCLUDE_DIR})

    if(BUILD_TARGET_BSD OR HAIKU)
        find_library(EXECINFO_LIBRARY execinfo)
    endif()
    find_library(PTHREAD_LIBRARY pthread)

    if(NOT BUILD_TARGET_WIN)
        pkg_check_modules(USB_LIB REQUIRED libusb-1.0)
        pkg_check_modules(FREETYPE REQUIRED freetype2)
    endif()
    # Can be 'libmypaint' or 'libmypaint-1.x'
    pkg_search_module(MYPAINT_LIB REQUIRED libmypaint libmypaint-1.3>=1.3)

    if(PLATFORM EQUAL 64)
        find_library(TURBOJPEG_LIB turbojpeg)
        message("**************** turbojpeg lib:" ${TURBOJPEG_LIB})
    endif()
endif()


include(TestBigEndian)
test_big_endian(_SYSTEM_BIG_ENDIAN)
if(_SYSTEM_BIG_ENDIAN)
    add_definitions(-DTNZ_LITTLE_ENDIAN=0)
else()
    add_definitions(-DTNZ_LITTLE_ENDIAN=1)
endif()
unset(_SYSTEM_BIG_ENDIAN)


set(C_WARNINGS)
set(CXX_WARNINGS)

if(CMAKE_COMPILER_IS_GNUCC)
    ADD_CHECK_C_COMPILER_FLAG(C_WARNINGS C_WARN_WRITE_STRINGS -Wwrite-strings)
    ADD_CHECK_C_COMPILER_FLAG(C_WARNINGS C_WARN_UNDEF -Wundef)
    ADD_CHECK_C_COMPILER_FLAG(C_WARNINGS C_WARN_MISSING_DECLARATIONS -Wmissing-declarations)

    ADD_CHECK_CXX_COMPILER_FLAG(CXX_WARNINGS CXX_WARN_WRITE_STRINGS -Wwrite-strings)
    ADD_CHECK_CXX_COMPILER_FLAG(CXX_WARNINGS CXX_WARN_UNDEF -Wundef)
    ADD_CHECK_CXX_COMPILER_FLAG(CXX_WARNINGS CXX_WARN_MISSING_DECLARATIONS -Wmissing-declarations)
endif()


include_directories(
    SYSTEM
    BEFORE
    ${TIFF_INCLUDE_DIR}
    ${PNG_INCLUDE_DIRS}
)

if(BUILD_ENV_MSVC OR BUILD_ENV_APPLE)
    # Handle CMake 3.30+/4.x: Force legacy module for prebuilt Boost (no config files)
    if(CMAKE_VERSION VERSION_GREATER_EQUAL 3.30)
        cmake_policy(SET CMP0167 OLD)  # Use FindBoost module, not config
    endif()
    # Set hints/ROOT for both branches
    set(Boost_NO_BOOST_CMAKE ON)  # Explicitly disable config search
    find_path(
        BOOST_ROOT
            include/boost
            boost
        HINTS
            ${BOOST_ROOT}  # From -D or env
            ${THIRDPARTY_LIBS_HINTS}
        PATH_SUFFIXES
            boost/boost_1_89_0/  # Added recent versions (e.g., Homebrew 1.89)
            boost/boost_1_87_0/
            boost/boost_1_86_0/
            boost/boost_1_85_0/
            boost/boost_1_75_0/
            boost/boost_1_74_0/
            boost/boost_1_73_0/
            boost/boost_1_72_0/
    )
    find_package(Boost 1.55 REQUIRED)
else()
    find_package(Boost)
endif()

include_directories(
    SYSTEM
    ${Boost_INCLUDE_DIR}
    ${LZ4_LIB_INCLUDE_DIRS}
    ${USB_LIB_INCLUDE_DIRS}
    ${SUPERLU_INCLUDE_DIR}
    ${JPEG_INCLUDE_DIR}
    ${MYPAINT_LIB_INCLUDE_DIRS}
)

if(PLATFORM EQUAL 64)
    add_definitions(-Dx64)
endif()

if(BUILD_ENV_MSVC)
    # place Toonz.exe and dlls in the same directory
    set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR})
    set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} /MP")
    # use secure functions by defaualt
    set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} /D _CRT_SECURE_CPP_OVERLOAD_STANDARD_NAMES=1")
    set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} /D _CRT_SECURE_CPP_OVERLOAD_STANDARD_NAMES_COUNT=1")
elseif(BUILD_ENV_APPLE)
    # pass
elseif(BUILD_ENV_UNIXLIKE)
    set(EXECUTABLE_OUTPUT_PATH ${CMAKE_BINARY_DIR}/bin)

    # Mirror relative bin/lib location for installation
    # so the generated shell script works in both cases.
    set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/lib/flare)
    set(CMAKE_LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/lib/flare)
    set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)

    if(CMAKE_INSTALL_PREFIX_INITIALIZED_TO_DEFAULT)
        set(CMAKE_INSTALL_PREFIX "/opt/flare" CACHE PATH "Flare install prefix" FORCE)
    endif()
endif()

if(BUILD_ENV_MSVC AND MSVC_VERSION EQUAL 1800)
    # Visual C++ 2013 cannot handle UTF-8 without BOM
    # Non-ascii characters in comments would not be a problem
    add_compile_options("/wd4819")
endif()

# =============================================================================
# Translation compilation (.ts -> .qm) and folder renaming
# =============================================================================
if(WITH_TRANSLATION)

    # -------------------------------------------------------------------------
    # Dummy compatibility function
    # -------------------------------------------------------------------------
    function(add_translation module)
    endfunction()

    # ---------------------------------------------------------------------
    # Language data and display names
    # ---------------------------------------------------------------------
    set(LANG_CODES
        japanese italian french spanish chinese german russian korean czech
    )

    set(LANG_DISPLAY_NAMES
        "日本語" "Italiano" "Français" "Español" "中文" "Deutsch" "Русский" "한국어" "Čeština"
    )

    list(LENGTH LANG_CODES CODE_COUNT)
    list(LENGTH LANG_DISPLAY_NAMES DISPLAY_COUNT)
    if(NOT CODE_COUNT EQUAL DISPLAY_COUNT)
        message(FATAL_ERROR "LANG_CODES and LANG_DISPLAY_NAMES must have the same length!")
    endif()

    # ---------------------------------------------------------------------
    # Compile .ts -> .qm
    # ---------------------------------------------------------------------
    function(compile_translations module)
        set(qm_files "")
        foreach(lang ${LANG_CODES})
            set(ts_file "${CMAKE_SOURCE_DIR}/translations/${lang}/${module}.ts")
            set(qm_dir  "${CMAKE_BINARY_DIR}/../../stuff/config/loc/${lang}")
            set(qm_file "${qm_dir}/${module}.qm")

            # Ensure output directory exists at configure time (safe for parallel builds)
            file(MAKE_DIRECTORY "${qm_dir}")

            if(EXISTS "${ts_file}")
                add_custom_command(
                    OUTPUT  "${qm_file}"
                    COMMAND ${Qt5_LRELEASE_EXECUTABLE} "${ts_file}" -qm "${qm_file}"
                    DEPENDS "${ts_file}"
                    COMMENT "lrelease ${lang}/${module}.qm"
                    VERBATIM
                )
                list(APPEND qm_files "${qm_file}")
            endif()
        endforeach()

        if(qm_files)
            add_custom_target(translate_${module} ALL DEPENDS ${qm_files})
        endif()
    endfunction()

    # List of all modules that have translations
    set(TRANSLATION_MODULES
        toonz tnzcore toonzlib tnztools toonzqt image colorfx
    )

    foreach(mod ${TRANSLATION_MODULES})
        compile_translations(${mod})
    endforeach()

    # ---------------------------------------------------------------------
    # Rename language folders from code to native name (e.g. japanese → 日本語)
    # ---------------------------------------------------------------------
    set(LOC_ROOT "${CMAKE_BINARY_DIR}/../../stuff/config/loc")

    add_custom_target(rename_translation_folders ALL
        DEPENDS
            translate_toonz
            translate_tnzcore
            translate_toonzlib
            translate_tnztools
            translate_toonzqt
            translate_image
            translate_colorfx
        COMMENT "Renaming language folders to native display names"
    )

    # Generate the rename script (runs once per build)
    file(GENERATE OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/rename_languages.cmake" CONTENT
"set(LOC_ROOT \"${LOC_ROOT}\")
set(LANG_CODES ${LANG_CODES})
set(LANG_DISPLAY_NAMES ${LANG_DISPLAY_NAMES})

list(LENGTH LANG_CODES NUM_LANGS)
math(EXPR LAST_INDEX \"\${NUM_LANGS}-1\")

message(STATUS \"Renaming language folders to native display names...\")
foreach(idx RANGE 0 \${LAST_INDEX})
    list(GET LANG_CODES \${idx} src)
    list(GET LANG_DISPLAY_NAMES \${idx} dst)
    set(src_path \"\${LOC_ROOT}/\${src}\")
    set(dst_path \"\${LOC_ROOT}/\${dst}\")

    if(EXISTS \"\${src_path}\")
        if(EXISTS \"\${dst_path}\")
            file(REMOVE_RECURSE \"\${dst_path}\")
        endif()
        file(RENAME \"\${src_path}\" \"\${dst_path}\")
        message(STATUS \" => \${src}\")
    endif()
endforeach()
")

    # Execute rename after all .qm files are built
    add_custom_command(TARGET rename_translation_folders POST_BUILD
        COMMAND ${CMAKE_COMMAND} -P "${CMAKE_CURRENT_BINARY_DIR}/rename_languages.cmake"
        VERBATIM
    )

endif()

set(CMAKE_C_FLAGS "${C_WARNINGS} ${CMAKE_C_FLAGS}")
set(CMAKE_CXX_FLAGS "${CXX_WARNINGS} ${CMAKE_CXX_FLAGS}")

add_subdirectory(tnzcore)
add_subdirectory(tnzbase)
add_subdirectory(tnzext)
add_subdirectory(toonzlib)
add_subdirectory(toonzfarm/tfarm)
add_subdirectory(stdfx)
add_subdirectory(sound)
add_subdirectory(colorfx)
add_subdirectory(image)
add_subdirectory(toonzqt)
add_subdirectory(tnztools)

add_subdirectory(${SDKROOT}/lzo/driver lzodriver)

add_subdirectory(flare)
add_subdirectory(tcleanupper)
add_subdirectory(tcomposer)
add_subdirectory(tconverter)
add_subdirectory(toonzfarm)

if(BUILD_ENV_APPLE)
    add_subdirectory(mousedragfilter)
endif()

if((PLATFORM EQUAL 32) AND ((BUILD_TARGET_WIN AND BUILD_ENV_MSVC) OR BUILD_TARGET_APPLE))
    add_subdirectory(t32bitsrv)
endif()

if(BUILD_ENV_UNIXLIKE AND NOT BUILD_TARGET_WIN)
    add_subdirectory(xdg-data)
endif()