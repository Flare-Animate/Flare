#pragma once

#if !defined(flare_PARAMS_H__)
#define flare_PARAMS_H__

#include <flare_plugin.h>
//#include <flare_hostif.h>

#if defined(_MSC_VER)
#pragma pack(push, 1)
#endif

#if defined(DEFAULT)
/* remove creepy macro in tcommon.h */
#undef DEFAULT
#endif

struct flare_param_traits_double_t_ {
#if defined(__cplusplus)
  static const int E       = flare_PARAM_TYPE_DOUBLE;
  static const int RANGED  = 1;
  static const int DEFAULT = 1;
  typedef double valuetype;
  typedef double iovaluetype;
#endif
  double def;
  double min, max;
  double reserved_;
} flare_PACK;

typedef flare_param_traits_double_t_ flare_param_traits_double_t;

struct flare_param_traits_int_t_ {
#if defined(__cplusplus)
  static const int E       = flare_PARAM_TYPE_INT;
  static const int RANGED  = 1;
  static const int DEFAULT = 1;
  typedef int valuetype;
  typedef int iovaluetype;
#endif
  int def;
  int min, max;
  int reserved_;
} flare_PACK;

typedef flare_param_traits_int_t_ flare_param_traits_int_t;

struct flare_param_range_t_ {
  double a, b;
} flare_PACK;
typedef flare_param_range_t_ flare_param_range_t;

struct flare_param_traits_range_t_ {
#if defined(__cplusplus)
  static const int E       = flare_PARAM_TYPE_RANGE;
  static const int RANGED  = 1;
  static const int DEFAULT = 1;
  typedef flare_param_range_t valuetype;
  typedef flare_param_range_t iovaluetype;
#endif
  flare_param_range_t def;
  flare_param_range_t minmax;
  double reserved_;
} flare_PACK;

typedef flare_param_traits_range_t_ flare_param_traits_range_t;

struct flare_param_color_t_ {
  int c0, c1, c2, m;
} flare_PACK;
typedef flare_param_color_t_ flare_param_color_t;

struct flare_param_traits_color_t_ {
#if defined(__cplusplus)
  static const int E       = flare_PARAM_TYPE_PIXEL;
  static const int RANGED  = 0;
  static const int DEFAULT = 1;
  typedef flare_param_color_t valuetype;
  typedef flare_param_color_t iovaluetype;
#endif
  flare_param_color_t def;
  double reserved_;
} flare_PACK;

typedef flare_param_traits_color_t_ flare_param_traits_color_t;

struct flare_param_point_t_ {
  double x, y;
} flare_PACK;
typedef flare_param_point_t_ flare_param_point_t;

struct flare_param_traits_point_t_ {
#if defined(__cplusplus)
  static const int E       = flare_PARAM_TYPE_POINT;
  static const int RANGED  = 1;
  static const int DEFAULT = 1;
  typedef flare_param_point_t valuetype;
  typedef flare_param_point_t iovaluetype;
#endif
  flare_param_point_t def;
  flare_param_point_t min;
  flare_param_point_t max;
  double reserved_;
} flare_PACK;

typedef flare_param_traits_point_t_ flare_param_traits_point_t;

struct flare_param_traits_enum_t_ {
#if defined(__cplusplus)
  static const int E       = flare_PARAM_TYPE_ENUM;
  static const int RANGED  = 0;
  static const int DEFAULT = 1;
  typedef int valuetype;
  typedef int iovaluetype;
#endif
  int def;
  int enums;
  const char **array;
  double reserved_;
} flare_PACK;

typedef flare_param_traits_enum_t_ flare_param_traits_enum_t;

struct flare_param_traits_bool_t_ {
#if defined(__cplusplus)
  static const int E       = flare_PARAM_TYPE_BOOL;
  static const int RANGED  = 0;
  static const int DEFAULT = 1;
  typedef int valuetype;
  typedef int iovaluetype;
#endif
  int def;
  int reserved_;
} flare_PACK;
typedef flare_param_traits_bool_t_ flare_param_traits_bool_t;

struct flare_param_spectrum_t_ {
  double w;
  double c0, c1, c2, m;
} flare_PACK;
typedef flare_param_spectrum_t_ flare_param_spectrum_t;

struct flare_param_traits_spectrum_t_ {
#if defined(__cplusplus)
  static const int E       = flare_PARAM_TYPE_SPECTRUM;
  static const int RANGED  = 0;
  static const int DEFAULT = 1;
  typedef flare_param_spectrum_t valuetype;
  typedef flare_param_spectrum_t iovaluetype;
#endif
  double def;
  int points;
  flare_param_spectrum_t *array;
  int reserved_;
} flare_PACK;
typedef flare_param_traits_spectrum_t_ flare_param_traits_spectrum_t;

struct flare_param_traits_string_t_ {
#if defined(__cplusplus)
  static const int E       = flare_PARAM_TYPE_STRING;
  static const int RANGED  = 0;
  static const int DEFAULT = 1;
  typedef const char *valuetype;
  typedef char iovaluetype;
#endif
  const char *def;
  double reserved_;
} flare_PACK;

typedef flare_param_traits_string_t_ flare_param_traits_string_t;

struct flare_param_tonecurve_point_t_ {
  double x, y;
  double c0, c1, c2, c3;
} flare_PACK;
typedef flare_param_tonecurve_point_t_ flare_param_tonecurve_point_t;

struct flare_param_tonecurve_value_t_ {
  double x;
  double y;
  int channel;
  int interp;
} flare_PACK;
typedef flare_param_tonecurve_value_t_ flare_param_tonecurve_value_t;

struct flare_param_traits_tonecurve_t_ {
#if defined(__cplusplus)
  static const int E       = flare_PARAM_TYPE_TONECURVE;
  static const int RANGED  = 0;
  static const int DEFAULT = 0;
  typedef flare_param_tonecurve_point_t valuetype;
  typedef flare_param_tonecurve_value_t iovaluetype;
#endif
  double reserved_;
} flare_PACK;

typedef flare_param_traits_tonecurve_t_ flare_param_traits_tonecurve_t;

#define flare_PARAM_DESC_TYPE_PARAM (0)
#define flare_PARAM_DESC_TYPE_PAGE (1)
#define flare_PARAM_DESC_TYPE_GROUP (2)

struct flare_param_base_t_ {
  flare_if_version_t ver;
  int type;
  const char *label;
} flare_PACK;

struct flare_param_desc_t_ {
  struct flare_param_base_t_ base;
  const char *key;
  const char *note;
  void *reserved_[2];
  int traits_tag;
  union {
    flare_param_traits_double_t d;
    flare_param_traits_int_t i;
    flare_param_traits_enum_t e;
    flare_param_traits_range_t rd;
    flare_param_traits_bool_t b;
    flare_param_traits_color_t c;
    flare_param_traits_point_t p;
    flare_param_traits_spectrum_t g;
    flare_param_traits_string_t s;
    flare_param_traits_tonecurve_t t;
  } traits;
} flare_PACK;

typedef struct flare_param_desc_t_ flare_param_desc_t;

struct flare_param_group_t_ {
  struct flare_param_base_t_ base;
  int num;
  flare_param_desc_t *array;
} flare_PACK;

typedef struct flare_param_group_t_ flare_param_group_t;

struct flare_param_page_t_ {
  struct flare_param_base_t_ base;
  int num;
  flare_param_group_t *array;
} flare_PACK;

typedef struct flare_param_page_t_ flare_param_page_t;

#if defined(_MSC_VER)
#pragma pack(pop)
#endif

#endif

