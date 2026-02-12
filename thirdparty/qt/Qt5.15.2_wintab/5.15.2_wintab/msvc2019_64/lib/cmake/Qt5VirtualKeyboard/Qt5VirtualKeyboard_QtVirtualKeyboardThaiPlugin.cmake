
add_library(Qt5::QtVirtualKeyboardThaiPlugin MODULE IMPORTED)


_populate_VirtualKeyboard_plugin_properties(QtVirtualKeyboardThaiPlugin RELEASE "virtualkeyboard/qtvirtualkeyboard_thai.dll" TRUE)
_populate_VirtualKeyboard_plugin_properties(QtVirtualKeyboardThaiPlugin DEBUG "virtualkeyboard/qtvirtualkeyboard_thaid.dll" TRUE)

list(APPEND Qt5VirtualKeyboard_PLUGINS Qt5::QtVirtualKeyboardThaiPlugin)
set_property(TARGET Qt5::VirtualKeyboard APPEND PROPERTY QT_ALL_PLUGINS_virtualkeyboard Qt5::QtVirtualKeyboardThaiPlugin)
set_property(TARGET Qt5::QtVirtualKeyboardThaiPlugin PROPERTY QT_PLUGIN_TYPE "virtualkeyboard")
set_property(TARGET Qt5::QtVirtualKeyboardThaiPlugin PROPERTY QT_PLUGIN_EXTENDS "")
set_property(TARGET Qt5::QtVirtualKeyboardThaiPlugin PROPERTY QT_PLUGIN_CLASS_NAME "QtVirtualKeyboardThaiPlugin")
