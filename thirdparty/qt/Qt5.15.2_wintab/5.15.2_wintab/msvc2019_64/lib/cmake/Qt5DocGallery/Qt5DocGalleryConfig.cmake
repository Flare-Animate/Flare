if (CMAKE_VERSION VERSION_LESS 3.1.0)
    message(FATAL_ERROR "Qt 5 DocGallery module requires at least CMake version 3.1.0")
endif()

get_filename_component(_qt5DocGallery_install_prefix "${CMAKE_CURRENT_LIST_DIR}/../../../" ABSOLUTE)

# For backwards compatibility only. Use Qt5DocGallery_VERSION instead.
set(Qt5DocGallery_VERSION_STRING 0.0.0)

set(Qt5DocGallery_LIBRARIES Qt5::DocGallery)

macro(_qt5_DocGallery_check_file_exists file)
    if(NOT EXISTS "${file}" )
        message(FATAL_ERROR "The imported target \"Qt5::DocGallery\" references the file
   \"${file}\"
but this file does not exist.  Possible reasons include:
* The file was deleted, renamed, or moved to another location.
* An install or uninstall procedure did not complete successfully.
* The installation package was faulty and contained
   \"${CMAKE_CURRENT_LIST_FILE}\"
but not all the files it references.
")
    endif()
endmacro()


macro(_populate_DocGallery_target_properties Configuration LIB_LOCATION IMPLIB_LOCATION
      IsDebugAndRelease)
    set_property(TARGET Qt5::DocGallery APPEND PROPERTY IMPORTED_CONFIGURATIONS ${Configuration})

    set(imported_location "${_qt5DocGallery_install_prefix}/bin/${LIB_LOCATION}")
    _qt5_DocGallery_check_file_exists(${imported_location})
    set(_deps
        ${_Qt5DocGallery_LIB_DEPENDENCIES}
    )
    set(_static_deps
    )

    set_target_properties(Qt5::DocGallery PROPERTIES
        "IMPORTED_LOCATION_${Configuration}" ${imported_location}
        # For backward compatibility with CMake < 2.8.12
        "IMPORTED_LINK_INTERFACE_LIBRARIES_${Configuration}" "${_deps};${_static_deps}"
    )
    set_property(TARGET Qt5::DocGallery APPEND PROPERTY INTERFACE_LINK_LIBRARIES
                 "${_deps}"
    )


    set(imported_implib "${_qt5DocGallery_install_prefix}/lib/${IMPLIB_LOCATION}")
    _qt5_DocGallery_check_file_exists(${imported_implib})
    if(NOT "${IMPLIB_LOCATION}" STREQUAL "")
        set_target_properties(Qt5::DocGallery PROPERTIES
        "IMPORTED_IMPLIB_${Configuration}" ${imported_implib}
        )
    endif()
endmacro()

if (NOT TARGET Qt5::DocGallery)

    set(_Qt5DocGallery_OWN_INCLUDE_DIRS "${_qt5DocGallery_install_prefix}/include/" "${_qt5DocGallery_install_prefix}/include/QtDocGallery")
    set(Qt5DocGallery_PRIVATE_INCLUDE_DIRS
        "${_qt5DocGallery_install_prefix}/include/QtDocGallery/0.0.0"
        "${_qt5DocGallery_install_prefix}/include/QtDocGallery/0.0.0/QtDocGallery"
    )

    foreach(_dir ${_Qt5DocGallery_OWN_INCLUDE_DIRS})
        _qt5_DocGallery_check_file_exists(${_dir})
    endforeach()

    # Only check existence of private includes if the Private component is
    # specified.
    list(FIND Qt5DocGallery_FIND_COMPONENTS Private _check_private)
    if (NOT _check_private STREQUAL -1)
        foreach(_dir ${Qt5DocGallery_PRIVATE_INCLUDE_DIRS})
            _qt5_DocGallery_check_file_exists(${_dir})
        endforeach()
    endif()

    set(Qt5DocGallery_INCLUDE_DIRS ${_Qt5DocGallery_OWN_INCLUDE_DIRS})

    set(Qt5DocGallery_DEFINITIONS -DQT_DOCGALLERY_LIB)
    set(Qt5DocGallery_COMPILE_DEFINITIONS QT_DOCGALLERY_LIB)
    set(_Qt5DocGallery_MODULE_DEPENDENCIES "Core")


    set(Qt5DocGallery_OWN_PRIVATE_INCLUDE_DIRS ${Qt5DocGallery_PRIVATE_INCLUDE_DIRS})

    set(_Qt5DocGallery_FIND_DEPENDENCIES_REQUIRED)
    if (Qt5DocGallery_FIND_REQUIRED)
        set(_Qt5DocGallery_FIND_DEPENDENCIES_REQUIRED REQUIRED)
    endif()
    set(_Qt5DocGallery_FIND_DEPENDENCIES_QUIET)
    if (Qt5DocGallery_FIND_QUIETLY)
        set(_Qt5DocGallery_DEPENDENCIES_FIND_QUIET QUIET)
    endif()
    set(_Qt5DocGallery_FIND_VERSION_EXACT)
    if (Qt5DocGallery_FIND_VERSION_EXACT)
        set(_Qt5DocGallery_FIND_VERSION_EXACT EXACT)
    endif()

    set(Qt5DocGallery_EXECUTABLE_COMPILE_FLAGS "")

    foreach(_module_dep ${_Qt5DocGallery_MODULE_DEPENDENCIES})
        if (NOT Qt5${_module_dep}_FOUND)
            find_package(Qt5${_module_dep}
                0.0.0 ${_Qt5DocGallery_FIND_VERSION_EXACT}
                ${_Qt5DocGallery_DEPENDENCIES_FIND_QUIET}
                ${_Qt5DocGallery_FIND_DEPENDENCIES_REQUIRED}
                PATHS "${CMAKE_CURRENT_LIST_DIR}/.." NO_DEFAULT_PATH
            )
        endif()

        if (NOT Qt5${_module_dep}_FOUND)
            set(Qt5DocGallery_FOUND False)
            return()
        endif()

        list(APPEND Qt5DocGallery_INCLUDE_DIRS "${Qt5${_module_dep}_INCLUDE_DIRS}")
        list(APPEND Qt5DocGallery_PRIVATE_INCLUDE_DIRS "${Qt5${_module_dep}_PRIVATE_INCLUDE_DIRS}")
        list(APPEND Qt5DocGallery_DEFINITIONS ${Qt5${_module_dep}_DEFINITIONS})
        list(APPEND Qt5DocGallery_COMPILE_DEFINITIONS ${Qt5${_module_dep}_COMPILE_DEFINITIONS})
        list(APPEND Qt5DocGallery_EXECUTABLE_COMPILE_FLAGS ${Qt5${_module_dep}_EXECUTABLE_COMPILE_FLAGS})
    endforeach()
    list(REMOVE_DUPLICATES Qt5DocGallery_INCLUDE_DIRS)
    list(REMOVE_DUPLICATES Qt5DocGallery_PRIVATE_INCLUDE_DIRS)
    list(REMOVE_DUPLICATES Qt5DocGallery_DEFINITIONS)
    list(REMOVE_DUPLICATES Qt5DocGallery_COMPILE_DEFINITIONS)
    list(REMOVE_DUPLICATES Qt5DocGallery_EXECUTABLE_COMPILE_FLAGS)

    # It can happen that the same FooConfig.cmake file is included when calling find_package()
    # on some Qt component. An example of that is when using a Qt static build with auto inclusion
    # of plugins:
    #
    # Qt5WidgetsConfig.cmake -> Qt5GuiConfig.cmake -> Qt5Gui_QSvgIconPlugin.cmake ->
    # Qt5SvgConfig.cmake -> Qt5WidgetsConfig.cmake ->
    # finish processing of second Qt5WidgetsConfig.cmake ->
    # return to first Qt5WidgetsConfig.cmake ->
    # add_library cannot create imported target Qt5::Widgets.
    #
    # Make sure to return early in the original Config inclusion, because the target has already
    # been defined as part of the second inclusion.
    if(TARGET Qt5::DocGallery)
        return()
    endif()

    set(_Qt5DocGallery_LIB_DEPENDENCIES "Qt5::Core")


    add_library(Qt5::DocGallery SHARED IMPORTED)


    set_property(TARGET Qt5::DocGallery PROPERTY
      INTERFACE_INCLUDE_DIRECTORIES ${_Qt5DocGallery_OWN_INCLUDE_DIRS})
    set_property(TARGET Qt5::DocGallery PROPERTY
      INTERFACE_COMPILE_DEFINITIONS QT_DOCGALLERY_LIB)

    set_property(TARGET Qt5::DocGallery PROPERTY INTERFACE_QT_ENABLED_FEATURES )
    set_property(TARGET Qt5::DocGallery PROPERTY INTERFACE_QT_DISABLED_FEATURES )

    # Qt 6 forward compatible properties.
    set_property(TARGET Qt5::DocGallery
                 PROPERTY QT_ENABLED_PUBLIC_FEATURES
                 )
    set_property(TARGET Qt5::DocGallery
                 PROPERTY QT_DISABLED_PUBLIC_FEATURES
                 )
    set_property(TARGET Qt5::DocGallery
                 PROPERTY QT_ENABLED_PRIVATE_FEATURES
                 )
    set_property(TARGET Qt5::DocGallery
                 PROPERTY QT_DISABLED_PRIVATE_FEATURES
                 )

    set_property(TARGET Qt5::DocGallery PROPERTY INTERFACE_QT_PLUGIN_TYPES "")

    set(_Qt5DocGallery_PRIVATE_DIRS_EXIST TRUE)
    foreach (_Qt5DocGallery_PRIVATE_DIR ${Qt5DocGallery_OWN_PRIVATE_INCLUDE_DIRS})
        if (NOT EXISTS ${_Qt5DocGallery_PRIVATE_DIR})
            set(_Qt5DocGallery_PRIVATE_DIRS_EXIST FALSE)
        endif()
    endforeach()

    if (_Qt5DocGallery_PRIVATE_DIRS_EXIST)
        add_library(Qt5::DocGalleryPrivate INTERFACE IMPORTED)
        set_property(TARGET Qt5::DocGalleryPrivate PROPERTY
            INTERFACE_INCLUDE_DIRECTORIES ${Qt5DocGallery_OWN_PRIVATE_INCLUDE_DIRS}
        )
        set(_Qt5DocGallery_PRIVATEDEPS)
        foreach(dep ${_Qt5DocGallery_LIB_DEPENDENCIES})
            if (TARGET ${dep}Private)
                list(APPEND _Qt5DocGallery_PRIVATEDEPS ${dep}Private)
            endif()
        endforeach()
        set_property(TARGET Qt5::DocGalleryPrivate PROPERTY
            INTERFACE_LINK_LIBRARIES Qt5::DocGallery ${_Qt5DocGallery_PRIVATEDEPS}
        )

        # Add a versionless target, for compatibility with Qt6.
        if(NOT "${QT_NO_CREATE_VERSIONLESS_TARGETS}" AND NOT TARGET Qt::DocGalleryPrivate)
            add_library(Qt::DocGalleryPrivate INTERFACE IMPORTED)
            set_target_properties(Qt::DocGalleryPrivate PROPERTIES
                INTERFACE_LINK_LIBRARIES "Qt5::DocGalleryPrivate"
            )
        endif()
    endif()

    _populate_DocGallery_target_properties(RELEASE "Qt5DocGallery.dll" "Qt5DocGallery.lib" TRUE)



    _populate_DocGallery_target_properties(DEBUG "Qt5DocGalleryd.dll" "Qt5DocGalleryd.lib" TRUE)



    # In Qt 5.15 the glob pattern was relaxed to also catch plugins not literally named Plugin.
    # Define QT5_STRICT_PLUGIN_GLOB or ModuleName_STRICT_PLUGIN_GLOB to revert to old behavior.
    if (QT5_STRICT_PLUGIN_GLOB OR Qt5DocGallery_STRICT_PLUGIN_GLOB)
        file(GLOB pluginTargets "${CMAKE_CURRENT_LIST_DIR}/Qt5DocGallery_*Plugin.cmake")
    else()
        file(GLOB pluginTargets "${CMAKE_CURRENT_LIST_DIR}/Qt5DocGallery_*.cmake")
    endif()

    macro(_populate_DocGallery_plugin_properties Plugin Configuration PLUGIN_LOCATION
          IsDebugAndRelease)
        set_property(TARGET Qt5::${Plugin} APPEND PROPERTY IMPORTED_CONFIGURATIONS ${Configuration})

        set(imported_location "${_qt5DocGallery_install_prefix}/plugins/${PLUGIN_LOCATION}")
        _qt5_DocGallery_check_file_exists(${imported_location})
        set_target_properties(Qt5::${Plugin} PROPERTIES
            "IMPORTED_LOCATION_${Configuration}" ${imported_location}
        )

    endmacro()

    if (pluginTargets)
        foreach(pluginTarget ${pluginTargets})
            include(${pluginTarget})
        endforeach()
    endif()



    _qt5_DocGallery_check_file_exists("${CMAKE_CURRENT_LIST_DIR}/Qt5DocGalleryConfigVersion.cmake")
endif()

# Add a versionless target, for compatibility with Qt6.
if(NOT "${QT_NO_CREATE_VERSIONLESS_TARGETS}" AND TARGET Qt5::DocGallery AND NOT TARGET Qt::DocGallery)
    add_library(Qt::DocGallery INTERFACE IMPORTED)
    set_target_properties(Qt::DocGallery PROPERTIES
        INTERFACE_LINK_LIBRARIES "Qt5::DocGallery"
    )
endif()
