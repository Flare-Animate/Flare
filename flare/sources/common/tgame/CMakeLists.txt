# CMakeLists.txt for tgame module
# Game engine module for Flare

cmake_minimum_required(VERSION 3.16)

project(tgame LANGUAGES CXX)

# Find required packages
find_package(Qt5 COMPONENTS Core Gui Widgets Quick REQUIRED)
find_package(Box2D REQUIRED)

# Include directories
include_directories(
    ${CMAKE_CURRENT_SOURCE_DIR}
    ${CMAKE_CURRENT_BINARY_DIR}
    ${CMAKE_SOURCE_DIR}/include
    ${CMAKE_SOURCE_DIR}/flare/sources/include
    ${Qt5Core_INCLUDE_DIRS}
    ${Qt5Gui_INCLUDE_DIRS}
    ${Qt5Widgets_INCLUDE_DIRS}
    ${BOX2D_INCLUDE_DIRS}
)

# Source files
set(TGAME_SOURCES
    GameEngine.cpp
)

# Headers
set(TGAME_HEADERS
    ${CMAKE_SOURCE_DIR}/include/flare/tgame/GameEngine.h
)

# Create library
add_library(tgame SHARED ${TGAME_SOURCES})

# Target properties
set_target_properties(tgame PROPERTIES
    CXX_STANDARD 17
    CXX_STANDARD_REQUIRED ON
    CXX_EXTENSIONS OFF
    POSITION_INDEPENDENT_CODE ON
    VERSION ${PROJECT_VERSION}
    SOVERSION ${PROJECT_VERSION_MAJOR}
)

# Target include directories
target_include_directories(tgame
    PUBLIC
        $<BUILD_INTERFACE:${CMAKE_SOURCE_DIR}/include>
        $<BUILD_INTERFACE:${CMAKE_SOURCE_DIR}/flare/sources/include>
        $<INSTALL_INTERFACE:include>
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}
        ${CMAKE_CURRENT_BINARY_DIR}
)

# Link libraries
target_link_libraries(tgame
    PUBLIC
        Qt5::Core
        Qt5::Gui
        Qt5::Widgets
        Qt5::Quick
        Box2D::Box2D
)

# Install targets
install(TARGETS tgame
    EXPORT tgame-targets
    LIBRARY DESTINATION lib
    ARCHIVE DESTINATION lib
    RUNTIME DESTINATION bin
    INCLUDES DESTINATION include
)

install(FILES ${TGAME_HEADERS}
    DESTINATION include/flare/tgame
)

install(EXPORT tgame-targets
    FILE tgame-targets.cmake
    NAMESPACE tgame::
    DESTINATION lib/cmake/tgame
)

# Config file for find_package
configure_file(
    ${CMAKE_CURRENT_SOURCE_DIR}/cmake/tgameConfig.cmake.in
    ${CMAKE_CURRENT_BINARY_DIR}/tgameConfig.cmake
    @ONLY
)

install(FILES ${CMAKE_CURRENT_BINARY_DIR}/tgameConfig.cmake
    DESTINATION lib/cmake/tgame
)