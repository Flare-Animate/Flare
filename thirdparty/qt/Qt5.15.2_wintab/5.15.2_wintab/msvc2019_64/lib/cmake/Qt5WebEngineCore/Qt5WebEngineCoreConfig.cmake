if (CMAKE_VERSION VERSION_LESS 3.1.0)
    message(FATAL_ERROR "Qt 5 WebEngineCore module requires at least CMake version 3.1.0")
endif()

get_filename_component(_qt5WebEngineCore_install_prefix "${CMAKE_CURRENT_LIST_DIR}/../../../" ABSOLUTE)


macro(_qt5_WebEngineCore_check_file_exists file)
    if(NOT EXISTS "${file}" )
        message(FATAL_ERROR "The imported target \"Qt5::WebEngineCore\" references the file
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



if (NOT TARGET Qt5::WebEngineCore)

    set(_Qt5WebEngineCore_OWN_INCLUDE_DIRS "${_qt5WebEngineCore_install_prefix}/include/" "${_qt5WebEngineCore_install_prefix}/include/QtWebEngineCore")
    set(Qt5WebEngineCore_PRIVATE_INCLUDE_DIRS
        "${_qt5WebEngineCore_install_prefix}/include/QtWebEngineCore/5.15.2"
        "${_qt5WebEngineCore_install_prefix}/include/QtWebEngineCore/5.15.2/QtWebEngineCore"
    )

    foreach(_dir ${_Qt5WebEngineCore_OWN_INCLUDE_DIRS})
        _qt5_WebEngineCore_check_file_exists(${_dir})
    endforeach()

    # Only check existence of private includes if the Private component is
    # specified.
    list(FIND Qt5WebEngineCore_FIND_COMPONENTS Private _check_private)
    if (NOT _check_private STREQUAL -1)
        foreach(_dir ${Qt5WebEngineCore_PRIVATE_INCLUDE_DIRS})
            _qt5_WebEngineCore_check_file_exists(${_dir})
        endforeach()
    endif()

    set(_Qt5WebEngineCore_MODULE_DEPENDENCIES "")


    set(Qt5WebEngineCore_OWN_PRIVATE_INCLUDE_DIRS ${Qt5WebEngineCore_PRIVATE_INCLUDE_DIRS})

    set(_Qt5WebEngineCore_FIND_DEPENDENCIES_REQUIRED)
    if (Qt5WebEngineCore_FIND_REQUIRED)
        set(_Qt5WebEngineCore_FIND_DEPENDENCIES_REQUIRED REQUIRED)
    endif()
    set(_Qt5WebEngineCore_FIND_DEPENDENCIES_QUIET)
    if (Qt5WebEngineCore_FIND_QUIETLY)
        set(_Qt5WebEngineCore_DEPENDENCIES_FIND_QUIET QUIET)
    endif()
    set(_Qt5WebEngineCore_FIND_VERSION_EXACT)
    if (Qt5WebEngineCore_FIND_VERSION_EXACT)
        set(_Qt5WebEngineCore_FIND_VERSION_EXACT EXACT)
    endif()


    foreach(_module_dep ${_Qt5WebEngineCore_MODULE_DEPENDENCIES})
        if (NOT Qt5${_module_dep}_FOUND)
            find_package(Qt5${_module_dep}
                5.15.2 ${_Qt5WebEngineCore_FIND_VERSION_EXACT}
                ${_Qt5WebEngineCore_DEPENDENCIES_FIND_QUIET}
                ${_Qt5WebEngineCore_FIND_DEPENDENCIES_REQUIRED}
                PATHS "${CMAKE_CURRENT_LIST_DIR}/.." NO_DEFAULT_PATH
            )
        endif()

        if (NOT Qt5${_module_dep}_FOUND)
            set(Qt5WebEngineCore_FOUND False)
            return()
        endif()

    endforeach()

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
    if(TARGET Qt5::WebEngineCore)
        return()
    endif()

    set(_Qt5WebEngineCore_LIB_DEPENDENCIES "")


    add_library(Qt5::WebEngineCore INTERFACE IMPORTED)


    set_property(TARGET Qt5::WebEngineCore PROPERTY
      INTERFACE_INCLUDE_DIRECTORIES ${_Qt5WebEngineCore_OWN_INCLUDE_DIRS})
    set_property(TARGET Qt5::WebEngineCore PROPERTY
      INTERFACE_COMPILE_DEFINITIONS QT_WEBENGINECOREHEADERS_LIB)

    set_property(TARGET Qt5::WebEngineCore PROPERTY INTERFACE_QT_ENABLED_FEATURES )
    set_property(TARGET Qt5::WebEngineCore PROPERTY INTERFACE_QT_DISABLED_FEATURES )

    # Qt 6 forward compatible properties.
    set_property(TARGET Qt5::WebEngineCore
                 PROPERTY INTERFACE_QT_ENABLED_PUBLIC_FEATURES
                 )
    set_property(TARGET Qt5::WebEngineCore
                 PROPERTY INTERFACE_QT_DISABLED_PUBLIC_FEATURES
                 )
    set_property(TARGET Qt5::WebEngineCore
                 PROPERTY INTERFACE_QT_ENABLED_PRIVATE_FEATURES
                 )
    set_property(TARGET Qt5::WebEngineCore
                 PROPERTY INTERFACE_QT_DISABLED_PRIVATE_FEATURES
                 )

    set_property(TARGET Qt5::WebEngineCore PROPERTY INTERFACE_QT_PLUGIN_TYPES "")

    set(_Qt5WebEngineCore_PRIVATE_DIRS_EXIST TRUE)
    foreach (_Qt5WebEngineCore_PRIVATE_DIR ${Qt5WebEngineCore_OWN_PRIVATE_INCLUDE_DIRS})
        if (NOT EXISTS ${_Qt5WebEngineCore_PRIVATE_DIR})
            set(_Qt5WebEngineCore_PRIVATE_DIRS_EXIST FALSE)
        endif()
    endforeach()

    if (_Qt5WebEngineCore_PRIVATE_DIRS_EXIST)
        add_library(Qt5::WebEngineCorePrivate INTERFACE IMPORTED)
        set_property(TARGET Qt5::WebEngineCorePrivate PROPERTY
            INTERFACE_INCLUDE_DIRECTORIES ${Qt5WebEngineCore_OWN_PRIVATE_INCLUDE_DIRS}
        )
        set(_Qt5WebEngineCore_PRIVATEDEPS)
        foreach(dep ${_Qt5WebEngineCore_LIB_DEPENDENCIES})
            if (TARGET ${dep}Private)
                list(APPEND _Qt5WebEngineCore_PRIVATEDEPS ${dep}Private)
            endif()
        endforeach()
        set_property(TARGET Qt5::WebEngineCorePrivate PROPERTY
            INTERFACE_LINK_LIBRARIES Qt5::WebEngineCore ${_Qt5WebEngineCore_PRIVATEDEPS}
        )

        # Add a versionless target, for compatibility with Qt6.
        if(NOT "${QT_NO_CREATE_VERSIONLESS_TARGETS}" AND NOT TARGET Qt::WebEngineCorePrivate)
            add_library(Qt::WebEngineCorePrivate INTERFACE IMPORTED)
            set_target_properties(Qt::WebEngineCorePrivate PROPERTIES
                INTERFACE_LINK_LIBRARIES "Qt5::WebEngineCorePrivate"
            )
        endif()
    endif()

    set_target_properties(Qt5::WebEngineCore PROPERTIES
        INTERFACE_LINK_LIBRARIES "${_Qt5WebEngineCore_LIB_DEPENDENCIES}"
    )




    _qt5_WebEngineCore_check_file_exists("${CMAKE_CURRENT_LIST_DIR}/Qt5WebEngineCoreConfigVersion.cmake")
endif()

# Add a versionless target, for compatibility with Qt6.
if(NOT "${QT_NO_CREATE_VERSIONLESS_TARGETS}" AND TARGET Qt5::WebEngineCore AND NOT TARGET Qt::WebEngineCore)
    add_library(Qt::WebEngineCore INTERFACE IMPORTED)
    set_target_properties(Qt::WebEngineCore PROPERTIES
        INTERFACE_LINK_LIBRARIES "Qt5::WebEngineCore"
    )
endif()
