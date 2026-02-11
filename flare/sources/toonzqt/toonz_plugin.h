#pragma once

#ifndef flare_PLUGIN_H__
#define flare_PLUGIN_H__

#include <stdint.h>

#ifdef _WIN32
#define flare_EXPORT __declspec(dllexport)
#define flare_PACK
#else
#define flare_EXPORT
#define flare_PACK __attribute__((packed))
#endif

#if defined(_MSC_VER)
#pragma pack(push, 1)
#endif

struct flare_UUID_ {
  uint32_t uid0;
  uint16_t uid1;
  uint16_t uid2;
  uint16_t uid3;
  uint64_t uid4;
} flare_PACK;

typedef flare_UUID_ flare_UUID;

struct flare_if_version_t_ {
  int32_t major;
  int32_t minor;
} flare_PACK;

typedef flare_if_version_t_ flare_if_version_t;

struct flare_plugin_version_t_ {
  int32_t major;
  int32_t minor;
} flare_PACK;

typedef flare_plugin_version_t_ flare_plugin_version_t;

struct flare_nodal_rasterfx_handler_t_;

struct flare_plugin_probe_t_ {
  flare_if_version_t ver;            /* version of the structure */
  flare_plugin_version_t plugin_ver; /* version of the plugin */
  const char *name;
  const char *vendor;
  const char *id;
  const char *note;
  const char *helpurl;
  void *reserved_ptr_[3];
  uint32_t clss;
  uint32_t reserved_int_[7];
  flare_nodal_rasterfx_handler_t_ *handler;
  void *reserved_ptr_trail_[3];
} flare_PACK;

typedef flare_plugin_probe_t_ flare_plugin_probe_t;

struct flare_plugin_probe_list_t_ {
  flare_if_version_t ver; /* a version of flare_plugin_probe_t */
  struct flare_plugin_probe_t_ *begin;
  struct flare_plugin_probe_t_ *end;
} flare_PACK;

typedef flare_plugin_probe_list_t_ flare_plugin_probe_list_t;

#if defined(_MSC_VER)
#pragma pack(pop)
#endif

#if defined(__cplusplus)
namespace flare {
typedef flare_UUID UUID;
typedef flare_plugin_probe_t plugin_probe_t;
typedef flare_plugin_probe_list_t plugin_probe_list_t;
}
#endif

#define flare_IF_VER(major, minor)                                             \
  { (major), (minor) } /* interface or type version */
#define flare_PLUGIN_VER(major, minor)                                         \
  { (major), (minor) } /* plugin version */

#define flare_PLUGIN_PROBE_BEGIN(ver)                                          \
  static const flare_if_version_t flare_plugin_probe_ver_ = ver;               \
  struct flare_plugin_probe_t_ flare_plugin_info_begin_[] = {
#define flare_PLUGIN_PROBE_DEFINE(plugin_ver, nm, vendor, ident, note,         \
                                  helpurl, cls, handler)                       \
  {                                                                            \
    flare_plugin_probe_ver_, plugin_ver, (nm), (vendor), (ident), (note),      \
        (helpurl), {0}, /* reserved null ptrs */                               \
        ((cls)), {0},   /* reserved 32bit-integers */                          \
        (handler), {                                                           \
      0                                                                        \
    } /* reserved null ptrs */                                                 \
  }

#define flare_PLUGIN_PROBE_END                                                 \
  , { 0 } /* delim */                                                          \
  }                                                                            \
  ;                                                                            \
  flare_EXPORT struct flare_plugin_probe_list_t_ flare_plugin_info_list = {    \
      flare_plugin_probe_ver_, flare_plugin_info_begin_,                       \
      &flare_plugin_info_begin_[sizeof(flare_plugin_info_begin_) /             \
                                    sizeof(struct flare_plugin_probe_t_) -     \
                                1]};

/*! エラーコード */
#define flare_OK (0)             /*!< 成功 */
#define flare_ERROR_UNKNOWN (-1) /*!< 下記以外の不明なエラー */
#define flare_ERROR_NOT_IMPLEMENTED (-2) /*!< 未実装 */
#define flare_ERROR_VERSION_UNMATCH (-3) /*!< バージョン不整合 */
#define flare_ERROR_INVALID_HANDLE                                             \
  (-4) /*!< 型が異なるなどの無効なハンドルが渡された */
#define flare_ERROR_NULL (-5) /*!< NULLが許容されていない引数がNULL */
#define flare_ERROR_POLLUTED                                                   \
  (-6) /*!< 0でなければならない予約フィールドが0ではない */
#define flare_ERROR_OUT_OF_MEMORY (-7) /*!< メモリ不足 */
#define flare_ERROR_INVALID_SIZE                                               \
  (-8) /*!< 引数で指定されたサイズが間違っている */
#define flare_ERROR_INVALID_VALUE (-9) /*!< 定義されてない値 */
#define flare_ERROR_BUSY (-10) /*!< 要求されたリソースが既に使用されている */
#define flare_ERROR_NOT_FOUND (-11) /*!< 指定されたものが見つからなかった */
#define flare_ERROR_FAILED_TO_CREATE (-12) /*!< オブジェクト等の作成に失敗 */
#define flare_ERROR_PREREQUISITE                                               \
  (-13) /*!< 事前に他の関数を呼ぶなどの前提条件が満たされていない */

#define flare_PARAM_ERROR_NONE (0)
#define flare_PARAM_ERROR_VERSION (1 << 0) /* version unmatched */
#define flare_PARAM_ERROR_LABEL (1 << 1)   /* the label is null */
#define flare_PARAM_ERROR_TYPE                                                 \
  (1 << 2) /* type code does not match to structure */

#define flare_PARAM_ERROR_PAGE_NUM (1 << 3)
#define flare_PARAM_ERROR_GROUP_NUM (1 << 4)
#define flare_PARAM_ERROR_TRAITS (1 << 5) /* traits is unknown*/

#define flare_PARAM_ERROR_NO_KEY (1 << 8) /* the key is null */
#define flare_PARAM_ERROR_KEY_DUP                                              \
  (1 << 9) /* the key must be unique in the plugin */
#define flare_PARAM_ERROR_KEY_NAME                                             \
  (1 << 10) /* the key must be formed as '[:alpha:_][:alpha::number:_]* */
#define flare_PARAM_ERROR_POLLUTED                                             \
  (1 << 11) /* reserved field must be zero. for future release */

#define flare_PARAM_ERROR_MIN_MAX (1 << 12)
#define flare_PARAM_ERROR_ARRAY_NUM (1 << 13)
#define flare_PARAM_ERROR_ARRAY (1 << 14)
#define flare_PARAM_ERROR_UNKNOWN (1 << 31) /* */

enum flare_param_type_enum {
  flare_PARAM_TYPE_DOUBLE,
  flare_PARAM_TYPE_RANGE,
  flare_PARAM_TYPE_PIXEL,
  flare_PARAM_TYPE_POINT,
  flare_PARAM_TYPE_ENUM,
  flare_PARAM_TYPE_INT,
  flare_PARAM_TYPE_BOOL,
  flare_PARAM_TYPE_SPECTRUM,
  flare_PARAM_TYPE_STRING,
  flare_PARAM_TYPE_TONECURVE,

  flare_PARAM_TYPE_NB,
  flare_PARAM_TYPE_MAX = 0x7FFFFFFF
};

#endif

