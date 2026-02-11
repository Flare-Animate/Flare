#pragma once

#ifndef PLUGIN_PORT_INTERFACE
#define PLUGIN_PORT_INTERFACE

#include "flare_hostif.h"

int is_connected(flare_port_handle_t port, int *is_connected);
int get_fx(flare_port_handle_t port, flare_fxnode_handle_t *fxnode);

#endif

