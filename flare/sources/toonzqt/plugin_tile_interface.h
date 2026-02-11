#pragma once

#ifndef PLUGIN_TILE_INTERFACE
#define PLUGIN_TILE_INTERFACE

#include "flare_hostif.h"

int tile_interface_get_raw_address_unsafe(flare_tile_handle_t handle,
                                          void **address);
int tile_interface_get_raw_stride(flare_tile_handle_t handle, int *stride);
int tile_interface_get_element_type(flare_tile_handle_t handle, int *element);
int tile_interface_copy_rect(flare_tile_handle_t handle, int left, int top,
                             int width, int height, void *dst, int dststride);
int tile_interface_create_from(flare_tile_handle_t handle,
                               flare_tile_handle_t *newhandle);
int tile_interface_create(flare_tile_handle_t *newhandle);
int tile_interface_destroy(flare_tile_handle_t handle);
int tile_interface_get_rectangle(flare_tile_handle_t handle, flare_rect_t *);
int tile_interface_safen(flare_tile_handle_t handle);

#endif

