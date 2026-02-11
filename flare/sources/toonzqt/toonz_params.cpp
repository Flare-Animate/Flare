#include "pluginhost.h"
#include "flare_params.h"

extern "C" {
int set_parameter_pages(void *, int num, flare_param_page_t *params);
int set_parameter_pages_with_error(void *, int num, flare_param_page_t *params,
                                   int *, void **);
}

int set_parameter_pages_with_error(void *host, int num,
                                   flare_param_page_t *params, int *err,
                                   void **position) {
  if (!host) return flare_ERROR_NULL;
  if (num == 0) return flare_OK; /* num==0 の場合は無視してよい */
  if (params == NULL) return flare_ERROR_NULL;
  int e     = 0;
  void *pos = NULL;
  bool ret  = reinterpret_cast<RasterFxPluginHost *>(host)->setParamStructure(
      num, params, e, pos);
  if (!ret) {
    if (err) {
      *err                    = e;
      if (position) *position = pos;
    }
    return flare_ERROR_INVALID_VALUE;
  }
  return flare_OK;
}

int set_parameter_pages(void *host, int num, flare_param_page_t *params) {
  return set_parameter_pages_with_error(host, num, params, NULL, NULL);
}

