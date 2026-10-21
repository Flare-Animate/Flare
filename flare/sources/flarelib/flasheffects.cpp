#include "flasheffects.h"

#include "flare/fxdag.h"
#include "flare/tcolumnfxset.h"
#include "flare/txsheet.h"
#include "tdoubleparam.h"
#include "tfx.h"
#include "tnotanimatableparam.h"
#include "tparamcontainer.h"
#include "tparamset.h"

#include <cmath>

namespace FlashNative {

namespace {

TParam *param(TFx *fx, const char *name) {
  TParam *found = fx->getParams() ? fx->getParams()->getParam(name) : nullptr;
  if (!found)
    throw Flash::Error(QString::fromStdString(
        "The native " + fx->getFxType() + " has no parameter " + name + "."));
  return found;
}

template <class T>
T *typed(TFx *fx, const char *name) {
  T *result = dynamic_cast<T *>(param(fx, name));
  if (!result)
    throw Flash::Error(QString::fromStdString(
        "Unexpected native " + fx->getFxType() + " parameter " + name + "."));
  return result;
}

using State = EffectState;
using Param = EffectChain::Param;

void color(TFx *fx, const char *name, QVector<Param> &params,
           std::function<Flash::Color(const State &)> value) {
  auto *pixel = typed<TPixelParam>(fx, name);
  pixel->enableMatte(true);
  TDoubleParam *channels[] = {
      pixel->getRed().getPointer(), pixel->getGreen().getPointer(),
      pixel->getBlue().getPointer(), pixel->getMatte().getPointer()};
  for (int channel = 0; channel < 4; ++channel)
    params.push_back(
        {channels[channel], [value, channel](const State &s) {
           const Flash::Color c    = value(s);
           const unsigned char v[] = {c.red, c.green, c.blue, c.alpha};
           return v[channel] / 255.0;
         }});
}

bool fadeExpressible(const Flash::ColorTransform &t, bool alphaInColor) {
  if (alphaInColor) return false;
  const double m = t.multiplier[0];
  if (t.multiplier[1] != m || t.multiplier[2] != m || m < 0 || m > 1)
    return false;
  for (int channel = 0; channel < 3; ++channel) {
    const double o = t.offset[channel];
    if (m == 1 ? o != 0 : o < -1e-9 || o > 255 * (1 - m) + 1e-9) return false;
  }
  return true;
}

}  // namespace

QVector<Flash::Filter> neutralFilters(const QVector<Flash::Filter> &kinds) {
  QVector<Flash::Filter> result = kinds;
  for (auto &filter : result) {
    filter.blurX = filter.blurY = 0;
    filter.strength             = 0;
    filter.brightness = filter.contrast = filter.saturation = filter.hue = 0;
    filter.knockout = filter.hideObject = false;
  }
  return result;
}

EffectChain::EffectChain(const QVector<EffectState> &keys, double unit,
                         bool alphaInColor) {
  if (keys.isEmpty()) return;
  using Kind               = Flash::Filter::Kind;
  const EffectState &first = keys.front();
  for (int slot = 0; slot < first.filters.size(); ++slot) {
    const Flash::Filter &filter = first.filters[slot];
    bool used                   = false;
    for (const auto &key : keys) used |= key.filters.value(slot).effective();
    if (!used) continue;
    Stage stage;
    stage.sourceId = filter.sourceId;
    const double u = unit;
    switch (filter.kind) {
    case Kind::Blur:
      stage.fxType    = "STD_blurFx";
      stage.name      = "blur";
      stage.configure = [slot, u](TFx *fx, QVector<Param> &params) {
        params.push_back(
            {typed<TDoubleParam>(fx, "value"), [slot, u](const State &s) {
               const auto &f = s.filters[slot];
               return Flash::blurRadius(f.blurX, f.blurY, f.quality) * u /
                      s.scale;
             }});
      };
      break;
    case Kind::DropShadow:
    case Kind::Glow:
    case Kind::GradientGlow: {
      stage.fxType = "STD_dropShadowFx";
      stage.name   = filter.kind == Kind::DropShadow ? "drop shadow" : "glow";
      const bool shadow = filter.kind != Kind::Glow;
      stage.configure   = [slot, u, filter, shadow](TFx *fx,
                                                  QVector<Param> &params) {
        typed<TIntParam>(fx, "quality")->setValue(filter.quality);
        typed<TBoolParam>(fx, "inner")
            ->setValue(filter.kind == Kind::GradientGlow
                             ? filter.type == "inner"
                             : filter.inner);
        typed<TBoolParam>(fx, "knockout")->setValue(filter.knockout);
        typed<TBoolParam>(fx, "hide_object")
            ->setValue(filter.kind == Kind::DropShadow && filter.hideObject);
        // A gradient glow shows its strongest color (the last entry).
        color(fx, "color", params, [slot](const State &s) {
          const auto &f = s.filters[slot];
          return f.kind == Kind::GradientGlow && !f.gradient.isEmpty()
                       ? f.gradient.back().color
                       : f.color;
        });
        params.push_back(
              {typed<TDoubleParam>(fx, "blur_x"), [slot, u](const State &s) {
               return s.filters[slot].blurX * u / s.scale;
             }});
        params.push_back(
              {typed<TDoubleParam>(fx, "blur_y"), [slot, u](const State &s) {
               return s.filters[slot].blurY * u / s.scale;
             }});
        params.push_back(
              {typed<TDoubleParam>(fx, "strength"),
             [slot](const State &s) { return s.filters[slot].strength; }});
        params.push_back(
              {typed<TDoubleParam>(fx, "angle"),
             [slot](const State &s) { return s.filters[slot].angle; }});
        params.push_back({typed<TDoubleParam>(fx, "distance"),
                          [slot, u, shadow](const State &s) {
                            return shadow
                                         ? s.filters[slot].distance * u / s.scale
                                         : 0.0;
                          }});
      };
      break;
    }
    case Kind::Bevel:
    case Kind::GradientBevel:
      stage.fxType    = "STD_bevelFx";
      stage.name      = "bevel";
      stage.configure = [slot, u, filter](TFx *fx, QVector<Param> &params) {
        typed<TIntParam>(fx, "quality")->setValue(filter.quality);
        typed<TIntEnumParam>(fx, "type")
            ->setValue(filter.type == "outer"  ? 1
                       : filter.type == "full" ? 2
                                               : 0);
        typed<TBoolParam>(fx, "knockout")->setValue(filter.knockout);
        // A gradient bevel lights with its last entry and shades with its
        // first one.
        color(fx, "highlight_color", params, [slot](const State &s) {
          const auto &f = s.filters[slot];
          return f.kind == Kind::GradientBevel && !f.gradient.isEmpty()
                     ? f.gradient.back().color
                     : f.highlight;
        });
        color(fx, "shadow_color", params, [slot](const State &s) {
          const auto &f = s.filters[slot];
          return f.kind == Kind::GradientBevel && !f.gradient.isEmpty()
                     ? f.gradient.front().color
                     : f.shadow;
        });
        params.push_back(
            {typed<TDoubleParam>(fx, "blur_x"), [slot, u](const State &s) {
               return s.filters[slot].blurX * u / s.scale;
             }});
        params.push_back(
            {typed<TDoubleParam>(fx, "blur_y"), [slot, u](const State &s) {
               return s.filters[slot].blurY * u / s.scale;
             }});
        params.push_back(
            {typed<TDoubleParam>(fx, "strength"),
             [slot](const State &s) { return s.filters[slot].strength; }});
        params.push_back(
            {typed<TDoubleParam>(fx, "angle"),
             [slot](const State &s) { return s.filters[slot].angle; }});
        params.push_back(
            {typed<TDoubleParam>(fx, "distance"), [slot, u](const State &s) {
               return s.filters[slot].distance * u / s.scale;
             }});
      };
      break;
    case Kind::AdjustColor:
      stage.fxType    = "STD_colorMatrixFx";
      stage.name      = "adjust color";
      stage.configure = [slot](TFx *fx, QVector<Param> &params) {
        static const char *rows[]    = {"red", "green", "blue", "alpha"};
        static const char *columns[] = {"red", "green", "blue", "alpha",
                                        "offset"};
        for (int index = 0; index < 20; ++index) {
          const std::string name =
              std::string(rows[index / 5]) + "_" + columns[index % 5];
          params.push_back({typed<TDoubleParam>(fx, name.c_str()),
                            [slot, index](const State &s) {
                              const auto &f = s.filters[slot];
                              return Flash::adjustColorMatrix(
                                  f.brightness, f.contrast, f.saturation,
                                  f.hue)[index];
                            }});
        }
      };
      break;
    }
    m_stages.push_back(stage);
  }

  // The color transform, after the filters.
  bool hasColor = false, fade = true;
  for (const auto &key : keys) {
    hasColor |= !key.color.colorIdentity() ||
                (alphaInColor &&
                 (key.color.multiplier[3] != 1 || key.color.offset[3] != 0));
    fade &= fadeExpressible(key.color, alphaInColor);
  }
  if (!hasColor) return;
  Stage stage;
  if (fade) {
    // RGB Fade: c * (1 - t) + C * t with t = 1 - multiplier, C = offset / t.
    // Where a key has no tint, the color of a tinted key is kept, so a tint
    // fading in or out keeps one color.
    Flash::Color tint;
    for (const auto &key : keys)
      if (key.color.multiplier[0] < 1) {
        const double t = 1 - key.color.multiplier[0];
        auto channel   = [&](int c) {
          return (unsigned char)std::lround(
                std::max(0.0, std::min(255.0, key.color.offset[c] / t)));
        };
        tint = {channel(0), channel(1), channel(2), 255};
        break;
      }
    stage.fxType    = "STD_rgbmFadeFx";
    stage.name      = "tint";
    stage.configure = [tint](TFx *fx, QVector<Param> &params) {
      auto *pixel              = typed<TPixelParam>(fx, "color");
      TDoubleParam *channels[] = {pixel->getRed().getPointer(),
                                  pixel->getGreen().getPointer(),
                                  pixel->getBlue().getPointer()};
      for (int c = 0; c < 3; ++c)
        params.push_back({channels[c], [tint, c](const State &s) {
                            const double t = 1 - s.color.multiplier[0];
                            const unsigned char fallback[] = {
                                tint.red, tint.green, tint.blue};
                            if (t < 1e-9) return fallback[c] / 255.0;
                            return std::max(
                                0.0,
                                std::min(1.0, s.color.offset[c] / t / 255.0));
                          }});
      params.push_back(
          {typed<TDoubleParam>(fx, "intensity"),
           [](const State &s) { return (1 - s.color.multiplier[0]) * 100; }});
    };
  } else {
    stage.fxType    = "STD_colorMatrixFx";
    stage.name      = "color";
    stage.configure = [alphaInColor](TFx *fx, QVector<Param> &params) {
      static const char *rows[] = {"red", "green", "blue", "alpha"};
      for (int row = 0; row < 4; ++row) {
        if (row == 3 && !alphaInColor) break;
        const std::string diagonal = std::string(rows[row]) + "_" + rows[row];
        const std::string offset   = std::string(rows[row]) + "_offset";
        params.push_back(
            {typed<TDoubleParam>(fx, diagonal.c_str()),
             [row](const State &s) { return s.color.multiplier[row]; }});
        params.push_back(
            {typed<TDoubleParam>(fx, offset.c_str()),
             [row](const State &s) { return s.color.offset[row]; }});
      }
    };
  }
  m_stages.push_back(stage);
}

TFx *EffectChain::build(TXsheet *xsheet, TFx *input, const QString &label,
                        QVector<EffectIssue> &issues) {
  FxDag *dag  = xsheet->getFxDag();
  TFx *output = input;
  for (const Stage &stage : m_stages) {
    TFxP fx(TFx::create(stage.fxType));
    if (!fx || fx->getFxType() != stage.fxType) {
      issues.push_back({stage.name == "tint" || stage.name == "color"
                            ? "unmapped-color-effect"
                            : "unmapped-filter",
                        stage.sourceId,
                        "The " + QString::fromStdString(stage.fxType) +
                            " FX is not available; the " + stage.name +
                            " is left out."});
      continue;
    }
    stage.configure(fx.getPointer(), m_params);
    if (!fx->connect(fx->getInputPortName(0), output))
      throw Flash::Error("Cannot connect the native " +
                         QString::fromStdString(stage.fxType) + ".");
    dag->assignUniqueId(fx.getPointer());
    fx->setName((label + " [Flash " + stage.name + "]").toStdWString());
    dag->getInternalFxs()->addFx(fx.getPointer());
    dag->removeFromXsheet(output);
    dag->addToXsheet(fx.getPointer());
    output = fx.getPointer();
  }
  return output;
}

TFx *blendFx(TXsheet *xsheet, TFx *input, const QString &mode,
             const QString &label) {
  const QString type = Flash::blendFxType(mode);
  TFxP fx(TFx::create(type.toStdString()));
  if (!fx || !fx->connect("Source1", input))
    throw Flash::Error("Cannot connect the native blend FX " + type + ".");
  // Darken keeps what is below where the column is transparent.
  if (mode == "darken")
    typed<TBoolParam>(fx.getPointer(), "matte")->setValue(false);
  FxDag *dag = xsheet->getFxDag();
  dag->assignUniqueId(fx.getPointer());
  fx->setName((label + " [Flash " + mode + "]").toStdWString());
  dag->getInternalFxs()->addFx(fx.getPointer());
  dag->removeFromXsheet(input);
  dag->addToXsheet(fx.getPointer());
  return fx.getPointer();
}

}  // namespace FlashNative
