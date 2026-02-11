#include "trasterfx.h"
#include "plugin_port_interface.h"

int is_connected(flare_port_handle_t port, int *is_connected) {
  if (!port) {
    return flare_ERROR_INVALID_HANDLE;
  }
  if (!is_connected) {
    return flare_ERROR_NULL;
  }
  *is_connected = reinterpret_cast<TRasterFxPort *>(port)->isConnected();
  return flare_OK;
}

int get_fx(flare_port_handle_t port, flare_fxnode_handle_t *fxnode) {
  if (!port) {
    return flare_ERROR_INVALID_HANDLE;
  }
  if (!fxnode) {
    return flare_ERROR_NULL;
  }
  *fxnode = reinterpret_cast<TRasterFxPort *>(port)->getFx();
  return flare_OK;
}

