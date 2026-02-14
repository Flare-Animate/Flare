
add_library(Qt5::QVirtualKeyboardPlugin MODULE IMPORTED)


_populate_Gui_plugin_properties(QVirtualKeyboardPlugin RELEASE "platforminputcontexts/qtvirtualkeyboardplugin.dll" TRUE)
_populate_Gui_plugin_properties(QVirtualKeyboardPlugin DEBUG "platforminputcontexts/qtvirtualkeyboardplugind.dll" TRUE)

list(APPEND Qt5Gui_PLUGINS Qt5::QVirtualKeyboardPlugin)
set_property(TARGET Qt5::Gui APPEND PROPERTY QT_ALL_PLUGINS_platforminputcontexts Qt5::QVirtualKeyboardPlugin)
set_property(TARGET Qt5::QVirtualKeyboardPlugin PROPERTY QT_PLUGIN_TYPE "platforminputcontexts")
set_property(TARGET Qt5::QVirtualKeyboardPlugin PROPERTY QT_PLUGIN_EXTENDS "-")
set_property(TARGET Qt5::QVirtualKeyboardPlugin PROPERTY QT_PLUGIN_CLASS_NAME "QVirtualKeyboardPlugin")
