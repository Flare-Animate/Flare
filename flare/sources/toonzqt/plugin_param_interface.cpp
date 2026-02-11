
#include <ttonecurveparam.h>
#include <flareqt/fxsettings.h>

#include "plugin_param_interface.h"
#include "plugin_utilities.h"
#include "tparamset.h"
#include "plugin_param_traits.h"

/* 公開インターフェイスからは削除 */
enum flare_param_value_type_enum {
  /* deprecated */
  flare_PARAM_VALUE_TYPE_CHAR   = 1,
  flare_PARAM_VALUE_TYPE_INT    = 4,
  flare_PARAM_VALUE_TYPE_DOUBLE = 8,

  flare_PARAM_VALUE_TYPE_NB,
  flare_PARAM_VALUE_TYPE_MAX = 0x7FFFFFFF
};

enum flare_value_unit_enum {
  flare_PARAM_UNIT_NONE,
  flare_PARAM_UNIT_LENGTH,
  flare_PARAM_UNIT_ANGLE,
  flare_PARAM_UNIT_SCALE,
  flare_PARAM_UNIT_PERCENTAGE,
  flare_PARAM_UNIT_PERCENTAGE2,
  flare_PARAM_UNIT_SHEAR,
  flare_PARAM_UNIT_COLOR_CHANNEL,

  flare_PARAM_UNIT_NB,
  flare_PARAM_UNIT_MAX = 0x7FFFFFFF
};

static int set_value_unit(TDoubleParamP param, flare_value_unit_enum unit);

int hint_default_value(flare_param_handle_t param, int size_in_bytes,
                       const void *default_value) {
  if (Param *_ = reinterpret_cast<Param *>(param)) {
    TParamP param = _->param();

    if (TDoubleParamP p = param) {
      if (size_in_bytes != sizeof(double)) {
        return flare_ERROR_INVALID_SIZE;
      }
      const double *values = reinterpret_cast<const double *>(default_value);
      p->setDefaultValue(values[0]);
    } else if (TRangeParamP p = param) {
      if (size_in_bytes != sizeof(double) * 2) {
        return flare_ERROR_INVALID_SIZE;
      }
      const double *values = reinterpret_cast<const double *>(default_value);
      p->setDefaultValue(std::make_pair(values[0], values[1]));
    } else if (TPixelParamP p = param) {
      if (size_in_bytes != sizeof(double) * 4) {
        return flare_ERROR_INVALID_SIZE;
      }
      const double *values = reinterpret_cast<const double *>(default_value);
      p->setDefaultValue(
          toPixel32(TPixelD(values[0], values[1], values[2], values[3])));
    } else if (TPointParamP p = param) {
      if (size_in_bytes != sizeof(double) * 2) {
        return flare_ERROR_INVALID_SIZE;
      }
      const double *values = reinterpret_cast<const double *>(default_value);
      p->setDefaultValue(TPointD(values[0], values[1]));
    } else if (TIntEnumParamP p = param) {
      if (size_in_bytes != sizeof(int)) {
        return flare_ERROR_INVALID_SIZE;
      }
      const int *values = reinterpret_cast<const int *>(default_value);
      p->setDefaultValue(values[0]);
    } else if (TIntParamP p = param) {
      if (size_in_bytes != sizeof(int)) {
        return flare_ERROR_INVALID_SIZE;
      }
      const int *values = reinterpret_cast<const int *>(default_value);
      p->setDefaultValue(values[0]);
    } else if (TBoolParamP p = param) {
      if (size_in_bytes != sizeof(int)) {
        return flare_ERROR_INVALID_SIZE;
      }
      const int *values = reinterpret_cast<const int *>(default_value);
      p->setDefaultValue(values[0]);
    } else if (TSpectrumParamP p = param) {
      const double *values = reinterpret_cast<const double *>(default_value);
      const int count      = size_in_bytes / (sizeof(double) * 5);
      std::vector<TSpectrum::ColorKey> keys(count);
      for (int i = 0; i < count; i++) {
        keys[i].first = values[5 * i + 0];
        keys[i].second =
            toPixel32(TPixelD(values[5 * i + 1], values[5 * i + 2],
                              values[5 * i + 3], values[5 * i + 4]));
      }
      p->setDefaultValue(TSpectrum(count, keys.data()));
    } else if (TStringParamP p = param) {
      if (size_in_bytes < 1) {
        return flare_ERROR_INVALID_SIZE;
      }
      const char *values = reinterpret_cast<const char *>(default_value);
      p->setDefaultValue(QString::fromStdString(values).toStdWString());
    } else if (TToneCurveParamP p = param) {
      const double *values = reinterpret_cast<const double *>(default_value);
      const int count      = size_in_bytes / (sizeof(double) * 2);
      QList<TPointD> list;
      for (int i = 0; i < count; ++i) {
        list.push_back(TPointD(values[2 * i + 0], values[2 * i + 1]));
      }
      p->setDefaultValue(list);
    } else {
      return flare_ERROR_NOT_IMPLEMENTED;
    }
  } else {
    return flare_ERROR_INVALID_HANDLE;
  }

  return flare_OK;
}

int hint_value_range(flare_param_handle_t param, const void *minvalue,
                     const void *maxvalue) {
  if (Param *_ = reinterpret_cast<Param *>(param)) {
    TParamP param = _->param();

    if (TDoubleParamP p = param) {
      p->setValueRange(*reinterpret_cast<const double *>(minvalue),
                       *reinterpret_cast<const double *>(maxvalue));
    } else if (TRangeParamP p = param) {
      const double minv = *reinterpret_cast<const double *>(minvalue);
      const double maxv = *reinterpret_cast<const double *>(maxvalue);
      p->getMin()->setValueRange(minv, maxv);
      p->getMax()->setValueRange(minv, maxv);
    } else if (TPointParamP p = param) {
      const double minv = *reinterpret_cast<const double *>(minvalue);
      const double maxv = *reinterpret_cast<const double *>(maxvalue);
      p->getX()->setValueRange(minv, maxv);
      p->getY()->setValueRange(minv, maxv);
    } else if (TIntParamP p = param) {
      p->setValueRange(*reinterpret_cast<const int *>(minvalue),
                       *reinterpret_cast<const int *>(maxvalue));
    } else {
      return flare_ERROR_NOT_IMPLEMENTED;
    }
  } else {
    return flare_ERROR_INVALID_HANDLE;
  }

  return flare_OK;
}

int hint_unit(flare_param_handle_t param, int unit) {
  if (Param *_ = reinterpret_cast<Param *>(param)) {
    TParamP param = _->param();

    if (TDoubleParamP p = param) {
      return set_value_unit(p, flare_value_unit_enum(unit));
    } else if (TRangeParamP p = param) {
      if (const int retval =
              set_value_unit(p->getMin(), flare_value_unit_enum(unit))) {
        return retval;
      }
      return set_value_unit(p->getMax(), flare_value_unit_enum(unit));
    } else if (TPointParamP p = param) {
      if (const int retval =
              set_value_unit(p->getX(), flare_value_unit_enum(unit))) {
        return retval;
      }
      return set_value_unit(p->getY(), flare_value_unit_enum(unit));
    } else {
      return flare_ERROR_NOT_IMPLEMENTED;
    }
  } else {
    return flare_ERROR_INVALID_HANDLE;
  }
}

int hint_item(flare_param_handle_t param, int item, const char *caption) {
  if (Param *_ = reinterpret_cast<Param *>(param)) {
    TParamP param = _->param();

    if (TIntEnumParamP p = param) {
      p->addItem(item, caption);
    } else {
      return flare_ERROR_NOT_IMPLEMENTED;
    }
  } else {
    return flare_ERROR_INVALID_HANDLE;
  }

  return flare_OK;
}

int set_description(flare_param_handle_t param, const char *description) {
  if (Param *_ = reinterpret_cast<Param *>(param)) {
    TParamP param = _->param();
    param->setDescription(description);
  } else {
    return flare_ERROR_INVALID_HANDLE;
  }

  return flare_OK;
}

static int set_value_unit(TDoubleParamP param, flare_value_unit_enum unit) {
  switch (unit) {
  case flare_PARAM_UNIT_NONE:
    break;
  case flare_PARAM_UNIT_LENGTH:
    param->setMeasureName("fxLength");
    break;
  case flare_PARAM_UNIT_ANGLE:
    param->setMeasureName("angle");
    break;
  case flare_PARAM_UNIT_SCALE:
    param->setMeasureName("scale");
    break;
  case flare_PARAM_UNIT_PERCENTAGE:
    param->setMeasureName("percentage");
    break;
  case flare_PARAM_UNIT_PERCENTAGE2:
    param->setMeasureName("percentage2");
    break;
  case flare_PARAM_UNIT_SHEAR:
    param->setMeasureName("shear");
    break;
  case flare_PARAM_UNIT_COLOR_CHANNEL:
    param->setMeasureName("colorChannel");
    break;
  default:
    printf("invalid param unit");
    return flare_ERROR_INVALID_VALUE;
  }

  return flare_OK;
}

int get_value_type(flare_param_handle_t param, int *pvalue_type) {
  if (!pvalue_type) {
    return flare_ERROR_NULL;
  }

  if (Param *_ = reinterpret_cast<Param *>(param)) {
    TParamP param = _->param();

    if (TDoubleParamP p = param) {
      *pvalue_type = flare_PARAM_VALUE_TYPE_DOUBLE;
    } else if (TRangeParamP p = param) {
      *pvalue_type = flare_PARAM_VALUE_TYPE_DOUBLE;
    } else if (TPixelParamP p = param) {
      *pvalue_type = flare_PARAM_VALUE_TYPE_DOUBLE;
    } else if (TPointParamP p = param) {
      *pvalue_type = flare_PARAM_VALUE_TYPE_DOUBLE;
    } else if (TIntEnumParamP p = param) {
      *pvalue_type = flare_PARAM_VALUE_TYPE_INT;
    } else if (TIntParamP p = param) {
      *pvalue_type = flare_PARAM_VALUE_TYPE_INT;
    } else if (TBoolParamP p = param) {
      *pvalue_type = flare_PARAM_VALUE_TYPE_INT;
    } else if (TSpectrumParamP p = param) {
      *pvalue_type = flare_PARAM_VALUE_TYPE_DOUBLE;
    } else if (TStringParamP p = param) {
      *pvalue_type = flare_PARAM_VALUE_TYPE_CHAR;
    } else if (TToneCurveParamP p = param) {
      *pvalue_type = flare_PARAM_VALUE_TYPE_DOUBLE;
    } else {
      return flare_ERROR_NOT_IMPLEMENTED;
    }
  } else {
    return flare_ERROR_INVALID_HANDLE;
  }

  return flare_OK;
}

int get_type(flare_param_handle_t param, double frame, int *ptype,
             int *pcount) {
  /* size はほとんどの型で自明なので返さなくてもいいよね? */
  if (!ptype || !pcount) return flare_ERROR_NULL;

  if (Param *p = reinterpret_cast<Param *>(param)) {
    flare_param_type_enum e = flare_param_type_enum(p->desc()->traits_tag);
    if (e >= flare_PARAM_TYPE_NB) return flare_ERROR_NOT_IMPLEMENTED;
    size_t v;
    if (parameter_type_check(p->param().getPointer(), p->desc(), v)) {
      *ptype = p->desc()->traits_tag;
      if (e == flare_PARAM_TYPE_STRING) {
        TStringParam *r =
            reinterpret_cast<TStringParam *>(p->param().getPointer());
        const std::string str =
            QString::fromStdWString(r->getValue()).toStdString();
        *pcount = str.length() + 1;
      } else if (e == flare_PARAM_TYPE_TONECURVE) {
        TToneCurveParam *r =
            reinterpret_cast<TToneCurveParam *>(p->param().getPointer());
        auto lst = r->getValue(frame);
        *pcount  = lst.size();
      } else {
        *pcount = 1;  // static_cast< int >(v);
      }
      return flare_OK;
    } else
      return flare_ERROR_NOT_IMPLEMENTED;
  }
  return flare_ERROR_INVALID_HANDLE;
}

int get_value(flare_param_handle_t param, double frame, int *psize_in_bytes,
              void *pvalue) {
  if (!psize_in_bytes) {
    return flare_ERROR_NULL;
  }

  if (!pvalue) {
    int type = 0;
    return get_type(param, frame, &type, psize_in_bytes);
  }

  if (Param *p = reinterpret_cast<Param *>(param)) {
    flare_param_type_enum e = flare_param_type_enum(p->desc()->traits_tag);
    if (e >= flare_PARAM_TYPE_NB) return flare_ERROR_NOT_IMPLEMENTED;
    size_t v;
    int icounts = *psize_in_bytes;
    if (parameter_read_value(p->param().getPointer(), p->desc(), pvalue, frame,
                             icounts, v)) {
      *psize_in_bytes = v;
      return flare_OK;
    } else
      return flare_ERROR_NOT_IMPLEMENTED;
  }
  return flare_ERROR_INVALID_HANDLE;
}

int get_string_value(flare_param_handle_t param, int *wholesize, int rcvbufsize,
                     char *pvalue) {
  if (!pvalue) return flare_ERROR_NULL;
  if (Param *p = reinterpret_cast<Param *>(param)) {
    const flare_param_desc_t *desc = p->desc();
    flare_param_type_enum e        = flare_param_type_enum(desc->traits_tag);
    size_t vsz;
    TParam *pp = p->param().getPointer();
    if (param_type_check_<tpbind_str_t>(pp, desc, vsz)) {
      size_t isize = rcvbufsize;
      size_t osize = 0;
      if (param_read_value_<tpbind_str_t>(pp, desc, pvalue, 0, isize, osize)) {
        if (wholesize) *wholesize = static_cast<int>(osize);
        return flare_OK;
      }
    }
  }
  return flare_ERROR_INVALID_HANDLE;
}

int get_spectrum_value(flare_param_handle_t param, double frame, double x,
                       flare_param_spectrum_t *pvalue) {
  if (!pvalue) return flare_ERROR_NULL;
  if (Param *p = reinterpret_cast<Param *>(param)) {
    const flare_param_desc_t *desc = p->desc();
    flare_param_type_enum e        = flare_param_type_enum(desc->traits_tag);
    size_t vsz;
    TParam *pp = p->param().getPointer();
    if (param_type_check_<tpbind_spc_t>(pp, desc, vsz)) {
      size_t isize = 1;
      size_t osize = 0;
      pvalue->w    = x;
      if (param_read_value_<tpbind_spc_t>(pp, desc, pvalue, frame, isize,
                                          osize)) {
        return flare_OK;
      }
    }
  }
  return flare_ERROR_INVALID_HANDLE;
}

int set_value(flare_param_handle_t param, double frame, int size_in_bytes,
              const void *pvalue) {
  if (Param *_ = reinterpret_cast<Param *>(param)) {
    TParamP param = _->param();

    if (TDoubleParamP p = param) {
      if (size_in_bytes != sizeof(double)) {
        return flare_ERROR_INVALID_SIZE;
      }
      p->setValue(frame, *reinterpret_cast<const double *>(pvalue));
    } else if (TRangeParamP p = param) {
      if (size_in_bytes != sizeof(double) * 2) {
        return flare_ERROR_INVALID_SIZE;
      }
      const double *const pvalues = reinterpret_cast<const double *>(pvalue);
      p->setValue(frame, std::make_pair(pvalues[0], pvalues[1]));
    } else if (TPixelParamP p = param) {
      if (size_in_bytes != sizeof(double) * 4) {
        return flare_ERROR_INVALID_SIZE;
      }
      const double *const pvalues = reinterpret_cast<const double *>(pvalue);
      p->setValueD(frame,
                   TPixelD(pvalues[0], pvalues[1], pvalues[2], pvalues[3]));
    } else if (TPointParamP p = param) {
      if (size_in_bytes != sizeof(double) * 2) {
        return flare_ERROR_INVALID_SIZE;
      }
      const double *const pvalues = reinterpret_cast<const double *>(pvalue);
      p->setValue(frame, TPointD(pvalues[0], pvalues[1]));
    } else if (TIntEnumParamP p = param) {
      if (size_in_bytes != sizeof(int)) {
        return flare_ERROR_INVALID_SIZE;
      }
      p->setValue(*reinterpret_cast<const int *>(pvalue));
    } else if (TIntParamP p = param) {
      if (size_in_bytes != sizeof(int)) {
        return flare_ERROR_INVALID_SIZE;
      }
      p->setValue(*reinterpret_cast<const int *>(pvalue));
    } else if (TBoolParamP p = param) {
      if (size_in_bytes != sizeof(int)) {
        return flare_ERROR_INVALID_SIZE;
      }
      p->setValue(*reinterpret_cast<const int *>(pvalue) != 0);
    } else if (TSpectrumParamP p = param) {
      const double *values = reinterpret_cast<const double *>(pvalue);
      const int count      = size_in_bytes / (sizeof(double) * 5);
      std::vector<TSpectrum::ColorKey> keys(count);
      for (int i = 0; i < count; i++) {
        keys[i].first = values[5 * i + 0];
        keys[i].second =
            toPixel32(TPixelD(values[5 * i + 1], values[5 * i + 2],
                              values[5 * i + 3], values[5 * i + 4]));
      }
      p->setValue(frame, TSpectrum(count, keys.data()));
    } else if (TStringParamP p = param) {
      if (size_in_bytes < 1) {
        return flare_ERROR_INVALID_SIZE;
      }
      const char *values = reinterpret_cast<const char *>(pvalue);
      p->setValue(QString::fromStdString(values).toStdWString());
    } else if (TToneCurveParamP p = param) {
      const double *values = reinterpret_cast<const double *>(pvalue);
      const int count      = size_in_bytes / (sizeof(double) * 2);
      QList<TPointD> list;
      for (int i = 0; i < count; ++i) {
        list.push_back(TPointD(values[2 * i + 0], values[2 * i + 1]));
      }
      p->setValue(frame, list);
    } else {
      return flare_ERROR_NOT_IMPLEMENTED;
    }
  } else {
    return flare_ERROR_INVALID_HANDLE;
  }

  return flare_OK;
}

