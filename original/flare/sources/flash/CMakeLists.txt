# Flash/XFL document reader used by the FLA/XFL import in flarelib.
add_library(tnzflash STATIC
    archive.cpp
    document.cpp
    timing.cpp
    geometry.cpp
    model.cpp
    placement.cpp
    fill.cpp
    media.cpp
    video.cpp
    fieldaccounting.cpp
    gradient.cpp
    shapetween.cpp
    effects.cpp
    motion.cpp)
set_target_properties(tnzflash PROPERTIES POSITION_INDEPENDENT_CODE ON)
target_include_directories(tnzflash PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
target_link_libraries(tnzflash PUBLIC Qt5::Core PRIVATE ${Z_LIB})
