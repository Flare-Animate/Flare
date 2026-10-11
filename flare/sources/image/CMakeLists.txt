set(HEADERS
    sgi/tiio_sgi.h
    pli/pli_io.h
    pli/tiio_pli.h
    tga/tiio_tga.h
    compatibility/inforegion.h
    compatibility/tfile_io.h
    compatibility/tnz4.h
    quantel/filequantel.h
    quantel/filequantelP.h
    quantel/tiio_quantel.h
    png/tiio_png.h
    psd/tiio_psd.h
    svg/tiio_svg.h
    tzl/tiio_tzl.h
    ../include/tnzimage.h
    mov/tiio_mov_proxy.h
    3gp/tiio_3gp_proxy.h
    ffmpeg/tiio_gif.h
    ffmpeg/tiio_webm.h
    ffmpeg/tiio_apng.h
    ffmpeg/tiio_mp4.h
    ffmpeg/tiio_ffmpeg.h
    ffmpeg/tiio_ff_mov.h
    sprite/tiio_sprite.h
    mesh/tiio_mesh.h
    exr/tinyexr_otmod.h
    exr/tiio_exr.h
    tzm/tiio_tzm.h
)

set(SOURCES
    tiio.cpp
    sgi/filesgi.cpp
    pli/pli_io.cpp
    pli/tags.cpp
    pli/tiio_pli.cpp
    tga/tiio_tga.cpp
    compatibility/inforegion.c
    compatibility/tfile_io.c
    compatibility/tnz4.c
    compatibility/tnz4_cpp.cpp
    quantel/filequantel.c
    quantel/tiio_quantel.cpp
    png/tiio_png.cpp
    psd/tiio_psd.cpp
    svg/tiio_svg.cpp
    tzl/tiio_tzl.cpp
    mov/tiio_mov_proxy.cpp
    3gp/tiio_3gp_proxy.cpp
    ffmpeg/tiio_gif.cpp
    ffmpeg/tiio_webm.cpp
    ffmpeg/tiio_apng.cpp
    ffmpeg/tiio_mp4.cpp
    ffmpeg/tiio_ffmpeg.cpp
    ffmpeg/tiio_ff_mov.cpp
    sprite/tiio_sprite.cpp
    mesh/tiio_mesh.cpp
    exr/tiio_exr.cpp
    tzm/tiio_tzm.cpp
)


# Can't build this on FreeBSD: libtiff requires internal API access.
# Probably some one get luck to fix this.
if(NOT BUILD_TARGET_BSD)
    set(HEADERS ${HEADERS}
        tif/tiio_tif.h
        tzp/tiio_plt.h
        tzp/tiio_tzp.h
        tzp/toonztags.h
    )
    set(SOURCES ${SOURCES}
        tif/tiio_tif.cpp
        tzp/tiio_plt.cpp
        tzp/tiio_tzp.cpp
        tzp/toonzrle.cpp
        tzp/avl.c
    )
    # private tiffiop.h (system libtiff-dev lacks it); needs configure-generated tif_config.h
    if(NOT WIN32)
        set_source_files_properties(tzp/tiio_tzp.cpp tzp/toonzrle.cpp PROPERTIES
            COMPILE_OPTIONS "-idirafter;${FLARE_ROOT}/thirdparty/tiff-4.0.3/libtiff")
    endif()
endif()


if(BUILD_TARGET_WIN)
    set(HEADERS ${HEADERS}
        avi/tiio_avi.h
        mov/tiio_mov.h
        3gp/tiio_3gp.h
    )
    set(SOURCES ${SOURCES}
        avi/tiio_avi.cpp
        mov/tiio_movW.cpp
        3gp/tiio_3gpW.cpp
    )
elseif(BUILD_TARGET_APPLE)
    set(HEADERS ${HEADERS}
        mov/tiio_movM.h
        3gp/tiio_3gpM.h
    )
    set(SOURCES ${SOURCES}
        mov/tiio_movM.cpp
        3gp/tiio_3gpM.cpp
    )
elseif(BUILD_TARGET_UNIX)
    set(HEADERS ${HEADERS}
        3gp/tiio_3gp_proxy.h
        mov/tiio_mov_proxy.h
    )
    set(SOURCES ${SOURCES}
        mov/tiio_mov_proxy.cpp
        3gp/tiio_3gp_proxy.cpp
    )
endif()

if(WITH_TRANSLATION)
    add_translation(image ${HEADERS} ${SOURCES})
endif()

add_library(image SHARED ${HEADERS} ${SOURCES})
add_definitions(
    -DTNZIMAGE_DLL
    -D_USRDLL
    -DIMAGE_EXPORTS
)

if(BUILD_ENV_APPLE)
    set(CMAKE_SHARED_LINKER_FLAGS "${CMAKE_SHARED_LINKER_FLAGS} -Wl,-install_name,@executable_path/libimage.dylib")
    add_dependencies(image tnzcore tnzbase flarelib)
endif()

message("subdir: image")
message("Bin: " ${CMAKE_CURRENT_BINARY_DIR})

include_directories(
    SYSTEM
    ${TIFF_INCLUDE_DIR}
)

if(NOT BUILD_TARGET_BSD)
    include_directories(
        ${SDKROOT}/LibJPEG/jpeg-9
    )
endif()


if(BUILD_ENV_MSVC)
    include_directories(
        SYSTEM
        ${SDKROOT}/libpng-1.6.21
    )
endif()
if(BUILD_TARGET_WIN AND PLATFORM EQUAL 32)
    include_directories(
        SYSTEM
        ${SDKROOT}/quicktime/QT73SDK/CIncludes
    )
endif()

include_directories(
    ${SDKROOT}/tinyexr
)


_find_flare_library(TNZLIBS "tnzcore;tnzbase;flarelib")


if(BUILD_ENV_MSVC)
    set(EXTRA_LIBS
        ${TNZLIBS}
        vfw32.lib
    )
elseif(BUILD_ENV_APPLE)
    if(PLATFORM EQUAL 32)
        find_library(CORE_SERVICES_LIB CoreServices)
        set(OSX_10_6_SDK_PATH "")
        if(NOT OSX_10_6_SDK_PATH)
            message(FATAL_ERROR "Set variable 'OSX_10_6_SDK_PATH'")
        endif()
        find_library(QD_LIB
            NAMES QD
            PATHS OSX10_6_SDK_PATH
        )
        set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -F ${OSX_10_6_SDK_PATH}")
    endif()
    set(EXTRA_LIBS
        Qt5::Widgets
        ${TNZLIBS}
        ${QT_LIB}
        ${CARBON_LIB}
        ${CORE_SERVICES_LIB}
        ${QD_LIB}
    )
elseif(BUILD_ENV_UNIXLIKE)
    # Generic Unix
    set(EXTRA_LIBS
        ${TNZLIBS}
    )

    if(BUILD_TARGET_WIN)
        set(EXTRA_LIBS ${EXTRA_LIBS} -lvfw32)
    endif()
endif()

target_link_libraries(image Qt5::Core Qt5::Gui Qt5::Network ${Z_LIB} ${GLUT_LIB} ${GL_LIB} ${JPEG_LIB} ${TIFF_LIB} ${PNG_LIB} ${EXTRA_LIBS})

# The TIFF reader wants libtiff's 64-bit RGBA entry points, but some libtiff
# builds (Ubuntu's, for one) do not export them, which is an unresolved
# external at link time rather than a compile error. Probe for the symbol and
# fall back to a widening shim over the 32-bit API when it is missing.
include(CheckCXXSymbolExists)
set(_flare_saved_req_libs "${CMAKE_REQUIRED_LIBRARIES}")
set(_flare_saved_req_flags "${CMAKE_REQUIRED_FLAGS}")
set(CMAKE_REQUIRED_LIBRARIES "${TIFF_LIB}")
set(CMAKE_REQUIRED_FLAGS "")
check_cxx_symbol_exists(TIFFReadRGBAStrip_64 "tiffio.h" FLARE_HAVE_TIFF_RGBA_64)
set(CMAKE_REQUIRED_LIBRARIES "${_flare_saved_req_libs}")
set(CMAKE_REQUIRED_FLAGS "${_flare_saved_req_flags}")
unset(_flare_saved_req_libs)
unset(_flare_saved_req_flags)
if(NOT FLARE_HAVE_TIFF_RGBA_64)
    message(STATUS "libtiff lacks TIFFReadRGBAStrip_64; using the 32-bit widening shim.")
    target_compile_definitions(image PRIVATE FLARE_TIFF_NO_RGBA_64=1)
endif()
