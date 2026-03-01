# This CMake script recursively scans the build directory for
# vcpkg.applocal.log AND .tlog files and removes any lines
# containing wildcard characters (* or ?).  It is invoked both
# during configure and as part of the build (via the
# sanitize_vcpkg_logs custom target) so that logs created later
# in the build (e.g. by Qt deploy scripts / applocal.ps1) do not
# trigger MSBuild MSB3541 errors ("Illegal characters in path").

if(NOT DEFINED CMAKE_BINARY_DIR)
    message(FATAL_ERROR "CMAKE_BINARY_DIR is not defined")
endif()

# --- 1. Sanitize vcpkg.applocal.log files ---
file(GLOB_RECURSE _vcpkg_applocal_logs "${CMAKE_BINARY_DIR}/**/vcpkg.applocal.log")
foreach(_applog ${_vcpkg_applocal_logs})
    file(READ ${_applog} _applog_contents)
    string(REGEX REPLACE "^[^\\n]*\\*[^\\n]*\\n" "" _applog_sanitized "${_applog_contents}")
    if(NOT _applog_sanitized STREQUAL _applog_contents)
        file(WRITE ${_applog} "${_applog_sanitized}")
    endif()
endforeach()

# --- 2. Sanitize .write.1u.tlog files ---
# These are the files MSBuild's RegisterOutput target reads.
# If applocal.ps1 / qtdeploy.ps1 wrote wildcard DLL paths into them,
# MSBuild will abort with MSB3541 on the next build.
file(GLOB_RECURSE _vcpkg_tlog_files "${CMAKE_BINARY_DIR}/**/*.write.1u.tlog")
foreach(_tlog ${_vcpkg_tlog_files})
    file(READ ${_tlog} _tlog_contents)
    string(REGEX REPLACE "[^\\n]*[*?][^\\n]*\\n?" "" _tlog_sanitized "${_tlog_contents}")
    if(NOT _tlog_sanitized STREQUAL _tlog_contents)
        file(WRITE ${_tlog} "${_tlog_sanitized}")
    endif()
endforeach()
