#pragma once

#ifndef PLUGIN_FXNODE_INTERFACE
#define PLUGIN_FXNODE_INTERFACE

#include "flare_hostif.h"

int fxnode_get_bbox(flare_fxnode_handle_t fxnode,
                    const flare_rendering_setting_t *, double frame,
                    flare_rect_t *rect, int *ret);
int fxnode_can_handle(flare_fxnode_handle_t fxnode,
                      const flare_rendering_setting_t *, double frame,
                      int *ret);
int fxnode_get_input_port_count(flare_fxnode_handle_t fxnode, int *count);
int fxnode_get_input_port(flare_fxnode_handle_t fxnode, int index,
                          flare_port_handle_t *port);
int fxnode_compute_to_tile(flare_fxnode_handle_t fxnode,
                           const flare_rendering_setting_t *rendering_setting,
                           double frame, const flare_rect_t *rect,
                           flare_tile_handle_t intile,
                           flare_tile_handle_t tile);

#endif

