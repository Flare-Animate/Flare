# This CMake script recursively scans the build directory for
# vcpkg.applocal.log files and removes any lines containing
# wildcard characters.  It is invoked both during configure and
# as part of the build (via the sanitize_vcpkg_logs custom target)
# so that logs created later in the build (e.g. by Qt deploy
# scripts) do not trigger MSBuild errors.

if(NOT DEFINED CMAKE_BINARY_DIR)
    message(FATAL_ERROR "CMAKE_BINARY_DIR is not defined")
endif()

file(GLOB_RECURSE _vcpkg_applocal_logs "${CMAKE_BINARY_DIR}/*/vcpkg.applocal.log")
foreach(_applog ${_vcpkg_applocal_logs})
    file(READ ${_applog} _applog_contents)
    # filter out lines containing '*' characters
    string(REGEX REPLACE "^[^\n]*\*[^\n]*\n" "" _applog_sanitized "${_applog_contents}")
    if(NOT _applog_sanitized STREQUAL _applog_contents)
        file(WRITE ${_applog} "${_applog_sanitized}")
    endif()
endforeach()
