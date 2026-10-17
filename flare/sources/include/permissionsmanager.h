#pragma once

#ifndef PERMISSIONSMANAGER_INCLUDED
#define PERMISSIONSMANAGER_INCLUDED

#include <memory>
#include <string>

// TnzCore includes
#include "tcommon.h"
#include "tenv.h"
#include "tsystem.h"

#undef DVAPI
#undef DVVAR
#ifdef TNZBASE_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

extern "C" typedef void (*LICENSE_OBSERVER_CB)(char *);

//************************************************************************
//    PermissionsManager  declaration (implemented in
//    tnzbase/permissionsmanager.rs; this header only forwards to it)
//************************************************************************

extern "C" {
void flare_permissions_init(const char *configDir, const char *userName);
char *flare_permissions_svn_username(int index);
char *flare_permissions_svn_password(int index);
void flare_permissions_free(char *p);
}

class PermissionsManager  // singleton; header-only, forwards to permissionsmanager.rs
{
public:
  static PermissionsManager *instance() {
    static PermissionsManager _instance;
    return &_instance;
  }
  std::string getSVNUserName(int index) const {
    return take(flare_permissions_svn_username(index));
  }
  std::string getSVNPassword(int index) const {
    return take(flare_permissions_svn_password(index));
  }

private:
  PermissionsManager() {
    flare_permissions_init(
        TEnv::getConfigDir().getQString().toUtf8().constData(),
        TSystem::getUserName().toUtf8().constData());
  }
  static std::string take(char *p) {
    std::string s = p ? p : "";
    flare_permissions_free(p);
    return s;
  }
};

#endif
