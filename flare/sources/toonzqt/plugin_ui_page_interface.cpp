#include "plugin_ui_page_interface.h"

int begin_group(flare_ui_page_handle_t page, const char *name) {
  if (UIPage *pages = reinterpret_cast<UIPage *>(page)) {
    return pages->begin_group(name);
  }

  return flare_ERROR_INVALID_HANDLE;
}

int end_group(flare_ui_page_handle_t page, const char *name) {
  if (UIPage *pages = reinterpret_cast<UIPage *>(page)) {
    return pages->end_group(name);
  }

  return flare_ERROR_INVALID_HANDLE;
}

int bind_param(flare_ui_page_handle_t page, flare_param_handle_t param,
               flare_param_view_handle_t traits) {
  if (UIPage *pages = reinterpret_cast<UIPage *>(page)) {
    if (Param *p = reinterpret_cast<Param *>(param)) {
      if (ParamView *v = reinterpret_cast<ParamView *>(traits)) {
        return pages->bind_param(p, v);
      }
    }
  }

  return flare_ERROR_INVALID_HANDLE;
}

