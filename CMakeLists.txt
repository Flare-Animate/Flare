# Root CMakeLists.txt for Flare project
#
# This file provides backward compatibility for build scripts that expect
# a CMakeLists.txt in the repository root.
#
# The actual project configuration is in flare/sources/CMakeLists.txt
#
# This wrapper simply includes the flare/sources project configuration.

cmake_minimum_required(VERSION 3.10)

# Note: We need to set up the project before including subdirectories
# to avoid CMake warnings

# The flare/sources/CMakeLists.txt file has its own project() call and
# expects to be the root project. To make this work from the repository root,
# we include it as a subdirectory but need to fix up some paths.

# Change the effective source directory to flare/sources by using
# CMAKE_CURRENT_SOURCE_DIR in the subdirectory

project(FlareWrapper)

message(STATUS "Building from repository root")
message(STATUS "Main project sources are in: ${CMAKE_CURRENT_SOURCE_DIR}/flare/sources")

# Add the flare cmake modules to the module path so they can be found
list(APPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/flare/cmake")

# Include the actual Flare project
# We use a custom binary directory name to avoid conflicts
add_subdirectory(flare/sources ${CMAKE_BINARY_DIR}/sources)
