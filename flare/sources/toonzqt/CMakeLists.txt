set(MOC_HEADERS
    palettesscanpopup.h
    pluginhost.h
    stageobjectselection.h
    tdockwindows.h
    ../include/flareqt/addfxcontextmenu.h
    ../include/flareqt/camerasettingswidget.h
    ../include/flareqt/checkbox.h
    ../include/flareqt/cleanupcamerasettingswidget.h
    ../include/flareqt/colorfield.h
    ../include/flareqt/combohistogram.h
    ../include/flareqt/doublefield.h
    ../include/flareqt/doublepairfield.h
    ../include/flareqt/dvdialog.h
    ../include/flareqt/dvscrollwidget.h
    ../include/flareqt/dvtextedit.h
    ../include/flareqt/expressionfield.h
    ../include/flareqt/filefield.h
    ../include/flareqt/flipconsole.h
    ../include/flareqt/framenavigator.h
    ../include/flareqt/functionkeyframenavigator.h
    ../include/flareqt/functionpanel.h
    ../include/flareqt/functionsegmentviewer.h
    ../include/flareqt/functionselection.h
    ../include/flareqt/functionsheet.h
    ../include/flareqt/functiontoolbar.h
    ../include/flareqt/functiontreeviewer.h
    ../include/flareqt/functionviewer.h
    ../include/flareqt/fxhistogramrender.h
    ../include/flareqt/fxiconmanager.h
    ../include/flareqt/fxschematicnode.h
    ../include/flareqt/fxschematicscene.h
    ../include/flareqt/fxselection.h
    ../include/flareqt/fxsettings.h
    ../include/flareqt/gutil.h
    ../include/flareqt/hexcolornames.h
    ../include/flareqt/histogram.h
    ../include/flareqt/icongenerator.h
    ../include/flareqt/imageutils.h
    ../include/flareqt/infoviewer.h
    ../include/flareqt/intfield.h
    ../include/flareqt/intpairfield.h
    ../include/flareqt/keyframenavigator.h
    ../include/flareqt/lineedit.h
    ../include/flareqt/marksbar.h
    ../include/flareqt/menubarcommand.h
    ../include/flareqt/paletteviewer.h
    ../include/flareqt/paletteviewergui.h
    ../include/flareqt/paramfield.h
    ../include/flareqt/planeviewer.h
    ../include/flareqt/popupbutton.h
    ../include/flareqt/schematicgroupeditor.h
    ../include/flareqt/schematicnode.h
    ../include/flareqt/schematicviewer.h
    ../include/flareqt/screenboard.h
    ../include/flareqt/scriptconsole.h
    ../include/flareqt/seethroughwindow.h
    ../include/flareqt/spectrumfield.h
    ../include/flareqt/spreadsheetviewer.h
    ../include/flareqt/stageschematicnode.h
    ../include/flareqt/stageschematicscene.h
    ../include/flareqt/studiopaletteviewer.h
    ../include/flareqt/styleeditor.h
    ../include/flareqt/styleindexlineedit.h
    ../include/flareqt/stylenameeditor.h
    ../include/flareqt/swatchviewer.h
    ../include/flareqt/tabbar.h
    ../include/flareqt/tmessageviewer.h
    ../include/flareqt/tonecurvefield.h
    ../include/flareqt/treemodel.h
    ../include/flareqt/tselectionhandle.h
    ../include/flareqt/updatechecker.h
    ../include/flareqt/validatedchoicedialog.h
)

set(HEADERS
    ${MOC_HEADERS}
    ../include/tools/cursormanager.h
    docklayout.h
    functionpaneltools.h
    fxdata.h
    palettedata.h
    plugin_fxnode_interface.h
    plugin_port_interface.h
    plugin_tile_interface.h
    styledata.h
    flare_hostif.h
    flare_plugin.h
    ../include/historytypes.h
    ../include/flareqt/dvmimedata.h
    ../include/flareqt/flipconsoleowner.h
    ../include/flareqt/freelayout.h
    ../include/flareqt/fxtypes.h
    ../include/flareqt/glwidget_for_highdpi.h
    ../include/flareqt/lutcalibrator.h
    ../include/flareqt/multipleselection.h
    ../include/flareqt/pickrgbutils.h
    ../include/flareqt/rasterimagedata.h
    ../include/flareqt/selection.h
    ../include/flareqt/selectioncommandids.h
    ../include/flareqt/stageobjectsdata.h
    ../include/flareqt/strokesdata.h
    ../include/flareqt/styleselection.h
    ../include/flareqt/trepetitionguard.h
    ../include/flareqt/viewcommandids.h
)

set(SOURCES
    ../tnztools/cursormanager.cpp
    checkbox.cpp
    colorfield.cpp
    docklayout.cpp
    dockwidget.cpp
    doublefield.cpp
    doublepairfield.cpp
    dvdialog.cpp
    dvscrollwidget.cpp
    dvtextedit.cpp
    expressionfield.cpp
    filefield.cpp
    flipconsole.cpp
    framenavigator.cpp
    freelayout.cpp
    fxhistogramrender.cpp
    fxsettings.cpp
    gutil.cpp
    hexcolornames.cpp
    histogram.cpp
    icongenerator.cpp
    imageutils.cpp
    infoviewer.cpp
    intfield.cpp
    intpairfield.cpp
    keyframenavigator.cpp
    lineedit.cpp
    menubarcommand.cpp
    palettesscanpopup.cpp
    paletteviewer.cpp
    paletteviewergui.cpp
    paramfield.cpp
    planeviewer.cpp
    popupbutton.cpp
    rasterimagedata.cpp
    scriptconsole.cpp
    seethroughwindow.cpp
    selection.cpp
    stageobjectsdata.cpp
    spectrumfield.cpp
    spreadsheetviewer.cpp
    strokesdata.cpp
    studiopaletteviewer.cpp
    styleeditor.cpp
    styleindexlineedit.cpp
    swatchviewer.cpp
    tabbar.cpp
    tdockwindows.cpp
    tonecurvefield.cpp
    treemodel.cpp
    tselectionhandle.cpp
    addfxcontextmenu.cpp
    fxschematicnode.cpp
    fxschematicscene.cpp
    schematicnode.cpp
    schematicviewer.cpp
    schematicgroupeditor.cpp
    stageschematicnode.cpp
    stageschematicscene.cpp
    functionselection.cpp
    fxselection.cpp
    stageobjectselection.cpp
    styleselection.cpp
    dvmimedata.cpp
    fxdata.cpp
    palettedata.cpp
    styledata.cpp
    functionkeyframenavigator.cpp
    functionpanel.cpp
    functionpaneltools.cpp
    functionsegmentviewer.cpp
    functionsheet.cpp
    functiontoolbar.cpp
    functiontreeviewer.cpp
    functionviewer.cpp
    updatechecker.cpp
    validatedchoicedialog.cpp
    trepetitionguard.cpp
    camerasettingswidget.cpp
    cleanupcamerasettingswidget.cpp
    pickrgbutils.cpp
    screenboard.cpp
    marksbar.cpp
    tmessageviewer.cpp
    stylenameeditor.cpp
    combohistogram.cpp
    fxiconmanager.cpp
    pluginhost.cpp
    plugin_tile_interface.cpp
    plugin_port_interface.cpp
    plugin_fxnode_interface.cpp
    plugin_param_interface.cpp
    plugin_param_view_interface.cpp
    plugin_ui_page_interface.cpp
    flare_params.cpp
	lutcalibrator.cpp
)

set(RESOURCES flareqt.qrc)

if(WITH_TRANSLATION)
    add_translation(flareqt ${HEADERS} ${SOURCES})
endif()

get_target_property(QW_LOC Qt5::Widgets INTERFACE_INCLUDE_DIRECTORIES)

set(CMAKE_INCLUDE_CURRENT_DIR ON)

set(incs)
foreach(dir ${Qt5Widgets_INCLUDE_DIRS})
    set(incs ${incs} -I${dir})
endforeach()

qt5_add_resources(SOURCES ${RESOURCES})

qt5_wrap_cpp(SOURCES ${MOC_HEADERS} OPTIONS ${incs})

add_library(flareqt SHARED ${HEADERS} ${SOURCES} ${RESOURCES})
if(BUILD_ENV_APPLE)
    set(CMAKE_SHARED_LINKER_FLAGS "${CMAKE_SHARED_LINKER_FLAGS} -Wl,-install_name,@executable_path/libflareqt.dylib")
    add_dependencies(flareqt tnzcore tnzbase tnzext flarelib sound)
endif()

add_definitions(
    -DQT_WIDGETS_LIB
    -DflareQT_EXPORTS
)

message("subdir: flareqt")
message("type:" ${CMAKE_SHARED_LIBRARY_SUFFIX})
message("Bin: " ${CMAKE_CURRENT_BINARY_DIR})

_find_flare_library(EXTRA_LIBS "tnzcore;tnzbase;tnzext;flarelib;sound")

if(BUILD_ENV_MSVC)
    set(EXTRA_LIBS ${EXTRA_LIBS}
        advapi32.lib
    )
elseif(BUILD_ENV_UNIXLIKE)
    if(BUILD_TARGET_WIN)
        set(EXTRA_LIBS ${EXTRA_LIBS}
            -ladvapi32
        )
    endif()
endif()

target_link_libraries(flareqt Qt5::Core Qt5::Gui Qt5::Widgets Qt5::Network Qt5::OpenGL Qt5::Svg ${EXTRA_LIBS} ${GL_LIB})

