set(HEADERS
    ../include/ext/ContextStatus.h
    ../include/ext/ExtUtil.h
    ../include/ext/Selector.h
    ../include/ext/StrokeParametricDeformer.h
    ../include/ext/Types.h
    ../include/ext/Designer.h
    ../include/ext/OverallDesigner.h
    ../include/ext/CornerDeformation.h
    ../include/ext/SmoothDeformation.h
    ../include/ext/StraightCornerDeformation.h
    ../include/ext/StrokeDeformation.h
    ../include/ext/StrokeDeformationImpl.h
    ../include/ext/LinearPotential.h
    ../include/ext/NotSymmetricBezierPotential.h
    ../include/ext/NotSymmetricExpPotential.h
    ../include/ext/Potential.h
    ../include/ext/SquarePotential.h
    DeformationSelector.h
    ../include/ext/meshbuilder.h
    ../include/ext/meshtexturizer.h
    ../include/ext/meshutils.h
    ../include/ext/plasticdeformer.h
    ../include/ext/plasticdeformerstorage.h
    ../include/ext/plastichandle.h
    ../include/ext/plasticskeleton.h
    ../include/ext/plasticskeletondeformation.h
    ../include/ext/plasticvisualsettings.h
    ../include/ext/ttexturesstorage.h
    ../include/tlin/cblas.h
    ../include/tlin/tlin.h
    ../include/tlin/tlin_basicops.h
    ../include/tlin/tlin_cblas_wrap.h
    ../include/tlin/tlin_matrix.h
    ../include/tlin/tlin_sparsemat.h
    ../include/tlin/tlin_superlu_wrap.h
    ../include/tlin/tlin_vector.h
)

set(SOURCES
    ContextStatus.cpp
    DeformationSelector.cpp
    ExtUtil.cpp
    Selector.cpp
    StrokeParametricDeformer.cpp
    Types.cpp
    LinearPotential.cpp
    NotSymmetricBezierPotential.cpp
    NotSymmetricExpPotential.cpp
    Potential.cpp
    SquarePotential.cpp
    CornerDeformation.cpp
    SmoothDeformation.cpp
    StraightCornerDeformation.cpp
    StrokeDeformation.cpp
    StrokeDeformationImpl.cpp
    Designer.cpp
    OverallDesigner.cpp
    meshbuilder.cpp
    meshtexturizer.cpp
    meshutils.cpp
    plasticdeformer.cpp
    plasticdeformerstorage.cpp
    plastichandle.cpp
    plasticskeleton.cpp
    plasticskeletondeformation.cpp
    ttexturesstorage.cpp
    tlin/tlin_cblas_wrap.cpp
    tlin/tlin_superlu_wrap.cpp
)

set(OBJCSOURCES
)

if(BUILD_ENV_MSVC AND PLATFORM EQUAL 32)
    # can not link libopenblas_32.lib with SAFESEH
    set(CMAKE_SHARED_LINKER_FLAGS "${CMAKE_SHARED_LINKER_FLAGS} /SAFESEH:NO")
endif()

add_library(tnzext SHARED ${HEADERS} ${SOURCES} ${OBJCSOURCES})

if(BUILD_ENV_APPLE)
    set(CMAKE_SHARED_LINKER_FLAGS "${CMAKE_SHARED_LINKER_FLAGS} -Wl,-install_name,@executable_path/libtnzext.dylib")
    add_dependencies(tnzext tnzcore tnzbase)
endif()

target_compile_definitions(tnzext PRIVATE TNZEXT_EXPORTS)

if(BUILD_ENV_APPLE)
    find_library(ACCE_LIB Accelerate)
endif()

if(BUILD_ENV_MSVC OR BUILD_ENV_APPLE)
    set(_tnzext_superlu_include_dir "${SDKROOT}/superlu/SuperLU_4.1/SRC")
    if(NOT EXISTS "${_tnzext_superlu_include_dir}")
        set(_tnzext_superlu_include_dir "${SUPERLU_INCLUDE_DIR}")
    endif()
    target_include_directories(tnzext SYSTEM PRIVATE "${_tnzext_superlu_include_dir}")
else()
    target_include_directories(tnzext SYSTEM PRIVATE "${SUPERLU_INCLUDE_DIR}")
endif()

if(BUILD_ENV_MSVC)
    target_link_libraries(tnzext
        Qt5::Core Qt5::Gui Qt5::OpenGL Qt5::Network
        ${GLUT_LIB} ${GL_LIB} ${SUPERLU_LIB} ${METIS_LIB} ${OPENBLAS_LIB} ${EXTRA_LIBS}
        tnzcore tnzbase
    )
    # If we added an ExternalProject to build SuperLU, ensure tnzext depends on it
    if(TARGET superlu_local)
        add_dependencies(tnzext superlu_local)
    endif()
elseif(BUILD_ENV_APPLE)
    set(EXTRA_LIBS
        "$<TARGET_FILE:tnzcore>"
        "$<TARGET_FILE:tnzbase>"
    )

    # If SuperLU is built from thirdparty source (architecture mismatch with the
    # bundled archive), make sure the archive exists before linking tnzext.
    if(TARGET superlu_local)
        add_dependencies(tnzext superlu_local)
    endif()

    target_link_libraries(tnzext
        Qt5::Core Qt5::Gui Qt5::OpenGL Qt5::Network
        ${ACCE_LIB} ${GLUT_LIB} ${GL_LIB} ${SUPERLU_LIB} ${EXTRA_LIBS}
    )
elseif(BUILD_ENV_UNIXLIKE)
    _find_flare_library(EXTRA_LIBS "tnzcore;tnzbase")

    target_link_libraries(tnzext
        Qt5::Core Qt5::Gui Qt5::OpenGL Qt5::Network
        ${GLUT_LIB} ${GL_LIB} ${SUPERLU_LIB} ${OPENBLAS_LIB} ${EXTRA_LIBS}
    )
endif()

# tmathutil.h (and friends) forward to Rust in flare_formats; on Apple tnzcore is
# linked by $<TARGET_FILE:...>, which does not carry that dependency along.
target_link_libraries(tnzext flare_formats)
