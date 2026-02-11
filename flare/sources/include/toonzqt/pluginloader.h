#pragma once

#if !defined(flare_PLUGIN_LOADER_H__)
#define flare_PLUGIN_LOADER_H__

#include <functional>
#include <string>
#include <QTreeWidget>

#undef DVAPI
#undef DVVAR
#ifdef flareQT_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

/* Plugin Loader の inter-module interfaces:
   flareqt 以下のモジュールからアクセスするのに必要 (see:
   flare/insertfxpopup.cpp)
   implements in pluginhost.cpp */
class TFx;

class DVAPI PluginLoader {
public:
  static std::map<std::string, QTreeWidgetItem *> create_menu_items(
      std::function<void(QTreeWidgetItem *)> &&,
      std::function<void(QTreeWidgetItem *)> &&);
  static TFx *create_host(const std::string &id);
  static bool load_entries(const std::string &basepath);
};

#endif

