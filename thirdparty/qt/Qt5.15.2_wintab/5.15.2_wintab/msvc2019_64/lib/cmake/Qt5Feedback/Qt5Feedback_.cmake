
add_library(Qt5:: MODULE IMPORTED)


_populate_Feedback_plugin_properties( RELEASE "feedback/qtfeedback_mmk.dll" TRUE)
_populate_Feedback_plugin_properties( DEBUG "feedback/qtfeedback_mmkd.dll" TRUE)

list(APPEND Qt5Feedback_PLUGINS Qt5::)
set_property(TARGET Qt5::Feedback APPEND PROPERTY QT_ALL_PLUGINS_feedback Qt5::)
set_property(TARGET Qt5:: PROPERTY QT_PLUGIN_TYPE "feedback")
set_property(TARGET Qt5:: PROPERTY QT_PLUGIN_EXTENDS "")
set_property(TARGET Qt5:: PROPERTY QT_PLUGIN_CLASS_NAME "")
