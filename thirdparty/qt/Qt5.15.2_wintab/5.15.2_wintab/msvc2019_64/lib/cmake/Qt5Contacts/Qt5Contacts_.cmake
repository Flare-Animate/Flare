
add_library(Qt5:: MODULE IMPORTED)


_populate_Contacts_plugin_properties( RELEASE "contacts/qtcontacts_memory.dll" TRUE)
_populate_Contacts_plugin_properties( DEBUG "contacts/qtcontacts_memoryd.dll" TRUE)

list(APPEND Qt5Contacts_PLUGINS Qt5::)
set_property(TARGET Qt5::Contacts APPEND PROPERTY QT_ALL_PLUGINS_contacts Qt5::)
set_property(TARGET Qt5:: PROPERTY QT_PLUGIN_TYPE "contacts")
set_property(TARGET Qt5:: PROPERTY QT_PLUGIN_EXTENDS "")
set_property(TARGET Qt5:: PROPERTY QT_PLUGIN_CLASS_NAME "")
