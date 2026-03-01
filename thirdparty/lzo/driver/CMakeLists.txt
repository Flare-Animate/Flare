project(lzotools)

if(MSVC)
    # use pre-built 2.03 when available; prefer vcpkg-installed lzo; otherwise
    # fall back to building minilzo from source
    get_filename_component(LZO_PATH ${SDKROOT}/lzo/2.03 ABSOLUTE)

    # detect vcpkg-installed lzo first
    if(CMAKE_SIZEOF_VOID_P EQUAL 8)
        set(_vcpkg_triplet "x64-windows")
    else()
        set(_vcpkg_triplet "x86-windows")
    endif()
    set(_vcpkg_lzo_lib "${CMAKE_SOURCE_DIR}/vcpkg/installed/${_vcpkg_triplet}/lib/lzo2.lib")

    if(EXISTS "${_vcpkg_lzo_lib}")
        set(_use_vcpkg_lzo TRUE)
    else()
        set(_use_vcpkg_lzo FALSE)
    endif()

    if(NOT _use_vcpkg_lzo AND EXISTS "${LZO_PATH}/LZO_lib/lzo2_64.lib")
        link_directories(${LZO_PATH}/LZO_lib/)
        if (CMAKE_SIZEOF_VOID_P EQUAL 8)
            set(PREFIX _64)
        endif()
        set(_use_prebuilt_lzo TRUE)
    else()
        if(NOT _use_vcpkg_lzo)
            set(_use_prebuilt_lzo FALSE)
        endif()
    endif()
else()
    set(LZO_PATH ${LZO_INCLUDE_DIR})
endif()

include_directories(${LZO_PATH} ${LZO_PATH}/include)
add_definitions(-DLZO_USE_ASM)
add_executable(lzocompress  lzocompress.c)
add_executable(lzodecompress lzodecompress.c)

# If prebuilt LZO import libs are not present (LFS placeholders), build a small
# embedded minilzo static library and link against it so the driver can be
# compiled successfully on Windows.
if(MSVC AND _use_vcpkg_lzo)
    # use vcpkg-installed lzo import lib directly
    target_link_libraries(lzocompress PRIVATE "${_vcpkg_lzo_lib}")
    target_link_libraries(lzodecompress PRIVATE "${_vcpkg_lzo_lib}")
elseif(MSVC AND NOT _use_prebuilt_lzo)
    add_library(minilzo STATIC "${LZO_PATH}/minilzo/minilzo.c")
    target_include_directories(minilzo PUBLIC ${LZO_PATH})
    target_link_libraries(lzocompress PRIVATE minilzo)
    target_link_libraries(lzodecompress PRIVATE minilzo)
elseif(MSVC AND _use_prebuilt_lzo)
    target_link_libraries(lzocompress lzo2${PREFIX} ${LZO_LIBRARY})
    target_link_libraries(lzodecompress lzo2${PREFIX} ${LZO_LIBRARY})
else()
    target_link_libraries(lzocompress ${LZO_LIBRARY})
    target_link_libraries(lzodecompress ${LZO_LIBRARY})
endif()
set(LZODRIVER_FOUND true PARENT_SCOPE)
