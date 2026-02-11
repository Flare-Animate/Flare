#include "plugin_fxnode_interface.h"
#include "plugin_utilities.h"
#include "trasterfx.h"
#include <functional>

static const TRenderSettings &restore_render_settings(
    const flare_rendering_setting_t *src) {
  return *reinterpret_cast<const TRenderSettings *>(src->context);
}

int fxnode_compute_to_tile(flare_fxnode_handle_t fxnode,
                           const flare_rendering_setting_t *rendering_setting,
                           double frame, const flare_rect_t *rect,
                           flare_tile_handle_t intile,
                           flare_tile_handle_t tile) {
  if (!fxnode || !rendering_setting || !rect || !tile)
    return flare_ERROR_NULL;  // inval
  TRasterFx *trasterfx =
      dynamic_cast<TRasterFx *>(reinterpret_cast<TFx *>(fxnode));
  if (!trasterfx) return flare_ERROR_INVALID_HANDLE;
  const TRenderSettings renderSettings =
      restore_render_settings(rendering_setting);
  TTile *t = reinterpret_cast<TTile *>(tile);
  TTile *i = reinterpret_cast<TTile *>(intile);
  TRasterP raster;
  if (i)
    raster = i->getRaster();
  else
    raster = 0;
  TPointD pos(rect->x0, rect->y0);
  TDimension dim(rect->x1 - rect->x0, rect->y1 - rect->y0);
  trasterfx->allocateAndCompute(*t, pos, dim, raster, frame, renderSettings);
  return flare_OK;
}

struct fxnode_dipatch {
  int operator()(
      flare_fxnode_handle_t fxnode, const flare_rendering_setting_t *rs,
      flare_rect_t *rect, int *ret,
      std::function<int(TRasterFx *, const TRenderSettings &, TRectD &)> f) {
    using namespace plugin::utils;
    TRasterFx *trasterfx =
        dynamic_cast<TRasterFx *>(reinterpret_cast<TFx *>(fxnode));
    if (!trasterfx) return flare_ERROR_INVALID_HANDLE;
    const TRenderSettings &renderSettings = restore_render_settings(rs);
    TRectD rc                             = restore_rect(rect);
    *ret                                  = f(trasterfx, renderSettings, rc);
    copy_rect(rect, rc);
    return flare_OK;
  }
};

int fxnode_get_bbox(flare_fxnode_handle_t fxnode,
                    const flare_rendering_setting_t *rendering_setting,
                    double frame, flare_rect_t *iorect, int *ret) {
  fxnode_dipatch disp;
  return disp(fxnode, rendering_setting, iorect, ret,
              [&](TRasterFx *fx, const TRenderSettings &rs, TRectD &rect) {
                return fx->getBBox(frame, rect, rs);
              });
}

int fxnode_can_handle(flare_fxnode_handle_t fxnode,
                      const flare_rendering_setting_t *rendering_setting,
                      double frame, int *ret) {
  TRasterFx *trasterfx =
      dynamic_cast<TRasterFx *>(reinterpret_cast<TFx *>(fxnode));
  if (!trasterfx) {
    return flare_ERROR_INVALID_HANDLE;
  }
  const TRenderSettings &renderSettings =
      restore_render_settings(rendering_setting);
  *ret = trasterfx->canHandle(renderSettings, frame);
  return flare_OK;
}

int fxnode_get_input_port_count(flare_fxnode_handle_t fxnode, int *count) {
  if (!fxnode) {
    return flare_ERROR_INVALID_HANDLE;
  }
  TFx *tfx = reinterpret_cast<TFx *>(fxnode);
  *count   = tfx->getInputPortCount();
  return flare_OK;
}

int fxnode_get_input_port(flare_fxnode_handle_t fxnode, int index,
                          flare_port_handle_t *port) {
  if (!fxnode) {
    return flare_ERROR_INVALID_HANDLE;
  }
  TFx *tfx              = reinterpret_cast<TFx *>(fxnode);
  flare_port_handle_t p = nullptr;
  p                     = tfx->getInputPort(index);
  if (!p) return flare_ERROR_NOT_FOUND;
  *port = p;
  return flare_OK;
}

