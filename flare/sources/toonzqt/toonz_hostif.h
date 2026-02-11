#pragma once

#ifndef flare_HOSTIF_H__
#define flare_HOSTIF_H__

#include <stdint.h>
#include <stddef.h>
#include "flare_plugin.h"
#include "flare_params.h"

#define flare_PLUGIN_CLASS_MODIFIER_MASK (0xffUL << (32 - 8))
#define flare_PLUGIN_CLASS_MODIFIER_GEOMETRIC (1UL << (32 - 1))
#define flare_PLUGIN_CLASS_POSTPROCESS_ (0x10UL)
#define flare_PLUGIN_CLASS_POSTPROCESS_SLAB                                    \
  (flare_PLUGIN_CLASS_POSTPROCESS_ | 0UL)

#define flare_PORT_TYPE_RASTER (0)

#if defined(_MSC_VER)
#pragma pack(push, 1)
#endif

static const flare_UUID flare_uuid_node__ = {0xCC14EA21, 0x13D8, 0x4A3B, 0x9375,
                                             0xAA4F68C9DDDD};
static const flare_UUID flare_uuid_port__ = {0x2F89A423, 0x1D2D, 0x433F, 0xB93E,
                                             0xCFFD83745F6F};
static const flare_UUID flare_uuid_tile__ = {0x882BD525, 0x937E, 0x427C, 0x9D68,
                                             0x4ECA651F6562};
static const flare_UUID flare_uuid_fx_node__ = {0x26F9FC53, 0x632B, 0x422F,
                                                0x87A0, 0x8A4547F55474};
static const flare_UUID flare_uuid_param__ = {0x2E3E4A55, 0x8539, 0x4520,
                                              0xA266, 0x15D32189EC4D};
static const flare_UUID flare_uuid_setup__ = {0xcfde9107, 0xc59d, 0x414c,
                                              0xae4a, 0x3d115ba97933};

#define flare_UUID_NODE (&flare_uuid_node__)
#define flare_UUID_PORT (&flare_uuid_port__)
#define flare_UUID_TILE (&flare_uuid_tile__)
#define flare_UUID_FXNODE (&flare_uuid_fx_node__)
#define flare_UUID_PARAM (&flare_uuid_param__)
#define flare_UUID_SETUP (&flare_uuid_setup__)

/* Host Interface */
struct flare_host_interface_t_ {
  flare_if_version_t ver;
  int (*query_interface)(const flare_UUID *, void **);
  void (*release_interface)(void *);
} flare_PACK;
typedef flare_host_interface_t_ flare_host_interface_t;

struct flare_rect_t_ {
  double x0;
  double y0;
  double x1;
  double y1;
} flare_PACK;

typedef struct flare_rect_t_ flare_rect_t;

typedef void *flare_param_handle_t;

struct flare_param_interface_t_ {
  flare_if_version_t ver;
  int (*get_type)(flare_param_handle_t param, double frame, int *type,
                  int *counts);
  int (*get_value)(flare_param_handle_t param, double frame, int *pcounts,
                   void *pvalue);
  int (*set_value)(flare_param_handle_t param, double frame, int counts,
                   const void *pvalue);
  int (*get_string_value)(flare_param_handle_t param, int *wholesize,
                          int rcvbufsize, char *pvalue);
  int (*get_spectrum_value)(flare_param_handle_t param, double frame, double x,
                            flare_param_spectrum_t *pvalue);
} flare_PACK;

typedef flare_param_interface_t_ flare_param_interface_t;

typedef void *flare_fxnode_handle_t;

typedef void *flare_port_handle_t;

struct flare_port_interface_t_ {
  flare_if_version_t ver;
  int (*is_connected)(flare_port_handle_t port, int *is_connected);
  int (*get_fx)(flare_port_handle_t port, flare_fxnode_handle_t *fxnode);
} flare_PACK;

typedef flare_port_interface_t_ flare_port_interface_t;

typedef void *flare_tile_handle_t;

struct flare_tile_interface_t_ {
  flare_if_version_t ver;
  int (*get_raw_address_unsafe)(flare_tile_handle_t handle, void **);
  int (*get_raw_stride)(flare_tile_handle_t handle, int *stride);
  int (*get_element_type)(flare_tile_handle_t handle, int *element);
  int (*copy_rect)(flare_tile_handle_t handle, int left, int top, int width,
                   int height, void *dst, int dststride);

  int (*create_from)(flare_tile_handle_t handle,
                     flare_tile_handle_t *newhandle);
  int (*create)(flare_tile_handle_t *newhandle);
  int (*destroy)(flare_tile_handle_t handle);

  int (*get_rectangle)(flare_tile_handle_t handle, flare_rect_t *rect);
  int (*safen)(flare_tile_handle_t handle);
} flare_PACK;

/*	Pixel types of tile
        Correspond to TRaster32P/64P/GR8P/GR16P/GRDP/YUV422P
*/
enum {
  flare_TILE_TYPE_NONE,
  flare_TILE_TYPE_32P,
  flare_TILE_TYPE_64P,
  flare_TILE_TYPE_GR8P,
  flare_TILE_TYPE_GR16P,
  flare_TILE_TYPE_GRDP,
  flare_TILE_TYPE_YUV422P,
};

typedef void *flare_node_handle_t;

struct flare_node_interface_t_ {
  flare_if_version_t ver;
  int (*get_input_port)(flare_node_handle_t node, const char *name,
                        flare_port_handle_t *port);

  int (*get_rect)(flare_rect_t *rect, double *x0, double *y0, double *x1,
                  double *y1);
  int (*set_rect)(flare_rect_t *rect, double x0, double y0, double x1,
                  double y1);

  int (*get_param)(flare_node_handle_t node, const char *name,
                   flare_param_handle_t *param);

  int (*set_user_data)(flare_node_handle_t node, void *user_data);
  int (*get_user_data)(flare_node_handle_t node, void **user_data);

} flare_PACK;

typedef flare_node_interface_t_ flare_node_interface_t;

typedef void *flare_handle_t;
typedef flare_tile_interface_t_ flare_tile_interface_t;
struct flare_affine_t_ {
  double a11;
  double a12;
  double a13;
  double a21;
  double a22;
  double a23;
} flare_PACK;

typedef flare_affine_t_ flare_affine_t;

typedef void *flare_node_handle_t;

struct flare_rendering_setting_t_ {
  flare_if_version_t ver;
  /** コピー元の TRenderSettings のスタック上アドレス */
  const void *context;
  /** エフェクトが描画された後に適用されるアフィン変換 */
  flare_affine_t affine;
  /**	出力フレームのためのガンマ修飾子 */
  double gamma;
  /** 入力の fps を変動させる変数 */
  double time_stretch_from;
  /** 出力の fps を変動させる変数 */
  double time_stretch_to;
  /** stereoscpy(ステレオ投影?)のためのカメラのx軸方向のシフト量 (単位は inch)
   */
  double stereo_scopic_shift;
  /**
  出力フレームで要求される bits per pixel の値。
  この値は適切なタイプのタイルと共に付随しなければならない。
  */
  int bpp;
  /**
  描画プロセスの間のタイルのキャッシュされる最大サイズ(MegaByte単位)。
  タイルのFxの計算の再分割のために予測キャッシュマネージャに用いられる(?)
  */
  int max_tile_size;
  /** アフィン変換におけるフィルタの品質 */
  int quality;
  /** ??? */
  int field_prevalence;
  /** ステレオ投影(3D効果)が用いられているかどうか */
  int stereoscopic;
  /** 描画インスタンスが swatch viewer から発生したものかどうか */
  int is_swatch;
  /** 描画リクエストの際にユーザーが手動でキャッシュするかどうか */
  int user_cachable;
  /** 縮小を必ず考慮しなければならないかどうか */
  int apply_shrink_to_viewer;
  /** カメラサイズ */
  flare_rect_t camera_box;
  /** 途中で Preview の計算がキャンセルされた時のフラグ */
  volatile int *is_canceled;
} flare_PACK;

typedef flare_rendering_setting_t_ flare_rendering_setting_t;

struct flare_nodal_rasterfx_handler_t_ {
  flare_if_version_t ver;
  void (*do_compute)(flare_node_handle_t node,
                     const flare_rendering_setting_t *, double frame,
                     flare_tile_handle_t tile);
  int (*do_get_bbox)(flare_node_handle_t node,
                     const flare_rendering_setting_t *, double frame,
                     flare_rect_t *rect);
  int (*can_handle)(flare_node_handle_t node, const flare_rendering_setting_t *,
                    double frame);
  size_t (*get_memory_requirement)(flare_node_handle_t node,
                                   const flare_rendering_setting_t *,
                                   double frame, const flare_rect_t *rect);

  void (*on_new_frame)(flare_node_handle_t node,
                       const flare_rendering_setting_t *, double frame);
  void (*on_end_frame)(flare_node_handle_t node,
                       const flare_rendering_setting_t *, double frame);

  int (*create)(flare_node_handle_t node);
  int (*destroy)(flare_node_handle_t node);
  int (*setup)(flare_node_handle_t node);
  int (*start_render)(flare_node_handle_t node);
  int (*end_render)(flare_node_handle_t node);

  void *reserved_ptr_[5];
} flare_PACK;

typedef flare_nodal_rasterfx_handler_t_ flare_nodal_rasterfx_handler_t;

struct flare_fxnode_interface_t_ {
  flare_if_version_t ver;
  /* TRasterFx の振る舞い */
  int (*get_bbox)(flare_fxnode_handle_t fxnode,
                  const flare_rendering_setting_t *, double frame,
                  flare_rect_t *rect, int *get_bbox);
  int (*can_handle)(flare_fxnode_handle_t fxnode,
                    const flare_rendering_setting_t *, double frame,
                    int *can_handle);
  /* TFx の振る舞い */
  int (*get_input_port_count)(flare_fxnode_handle_t fxnode, int *count);
  int (*get_input_port)(flare_fxnode_handle_t fxnode, int index,
                        flare_port_handle_t *port);
  int (*compute_to_tile)(flare_fxnode_handle_t fxnode,
                         const flare_rendering_setting_t *, double frame,
                         const flare_rect_t *rect, flare_tile_handle_t intile,
                         flare_tile_handle_t outtile);
} flare_PACK;

typedef struct flare_fxnode_interface_t_ flare_fxnode_interface_t;

struct flare_setup_interface_t_ {
  flare_if_version_t ver;
  int (*set_parameter_pages)(flare_node_handle_t node, int num,
                             flare_param_page_t *pages);
  int (*set_parameter_pages_with_error)(flare_node_handle_t node, int num,
                                        flare_param_page_t *pages, int *reason,
                                        void **position);
  int (*add_input_port)(flare_node_handle_t node, const char *name, int type);

} flare_PACK;
typedef struct flare_setup_interface_t_ flare_setup_interface_t;

#if defined(_MSC_VER)
#pragma pack(pop)
#endif

#if defined(__cplusplus)
namespace flare {
typedef flare_rendering_setting_t rendering_setting_t;
typedef flare_affine_t affine_t;
typedef flare_rect_t rect_t;

typedef flare_param_handle_t param_handle_t;

typedef flare_node_handle_t node_handle_t;
typedef flare_port_handle_t port_handle_t;
typedef flare_tile_handle_t tile_handle_t;
typedef flare_rendering_setting_t rendering_setting_t;
typedef flare_fxnode_handle_t fxnode_handle_t;

typedef flare_param_interface_t param_interface_t;

typedef flare_node_interface_t node_interface_t;
typedef flare_host_interface_t host_interface_t;
typedef flare_tile_interface_t tile_interface_t;
typedef flare_port_interface_t port_interface_t;
typedef flare_nodal_rasterfx_handler_t nodal_rasterfx_handler_t;
typedef flare_fxnode_interface_t fxnode_interface_t;
typedef flare_setup_interface_t setup_interface_t;
}
#endif

#endif

