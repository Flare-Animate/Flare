#pragma once

#include "../flash/effects.h"

#include <QStringList>
#include <QVector>
#include <functional>
#include <string>

class TFx;
class TXsheet;
class TDoubleParam;

namespace FlashNative {

// What a native column shows at one frame: the color effect and filters of
// its Flash instance (interpolated in a classic tween) and the column's own
// scale. Flash keeps filter lengths in stage pixels whatever the instance's
// scale, while FX lengths scale with the column, so they are divided by it.
struct EffectState {
  Flash::ColorTransform color;
  QVector<Flash::Filter> filters;
  double scale = 1;
};

struct EffectIssue {
  QString code, sourceId, message;
};

// The native FX of a column's Flash filters and color effect, in the order
// Flash applies them (the color transform acts on the filtered image):
//
//   column -> filter FX, in the source order -> color FX -> xsheet
//
// Every key state must list filters of the same kinds (see
// neutralFilters()). Blur becomes the stock Blur FX with the same spread,
// drop shadow and glow the Drop Shadow FX, bevels the Bevel FX, adjust color
// the Color Matrix FX. A tint or brightness (the same multiplier on red,
// green and blue) becomes the stock RGB Fade FX, exactly; other color
// transforms, and alpha when `alphaInColor`, the Color Matrix FX.
class EffectChain {
public:
  struct Param {
    TDoubleParam *param = nullptr;
    std::function<double(const EffectState &)> value;
  };

  EffectChain(const QVector<EffectState> &keys, double unit, bool alphaInColor);
  // Creates the FX after `input` (a column FX in `xsheet`'s FX DAG) and
  // returns the chain's output. FX types that are not available (stdfx not
  // loaded) are left out and reported.
  TFx *build(TXsheet *xsheet, TFx *input, const QString &label,
             QVector<EffectIssue> &issues);
  // Animatable parameters of the built FX, to be keyed per frame.
  const QVector<Param> &params() const { return m_params; }

  struct Stage {
    std::string fxType;
    QString name;      // FX node label suffix
    QString sourceId;  // filter or color effect
    std::function<void(TFx *, QVector<Param> &)> configure;
  };

private:
  QVector<Stage> m_stages;
  QVector<Param> m_params;
};

// The filters of `kinds` with no effect: what a keyframe without filters
// tweens from or to.
QVector<Flash::Filter> neutralFilters(const QVector<Flash::Filter> &kinds);

// Connects `input` to a new blend FX (Flash::blendFxType()) in place of
// `input` at the Xsheet node.
TFx *blendFx(TXsheet *xsheet, TFx *input, const QString &mode,
             const QString &label);

}  // namespace FlashNative
