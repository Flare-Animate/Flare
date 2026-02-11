#pragma once

#ifndef SCRIPTBINDING_flare_RASTER_CONVERTER_H
#define SCRIPTBINDING_flare_RASTER_CONVERTER_H

#include "flare/scriptbinding.h"

class flareScene;
class TXshSimpleLevel;

namespace TScriptBinding {

class DVAPI flareRasterConverter final : public Wrapper {
  Q_OBJECT
  bool m_flatSource;

public:
  flareRasterConverter();
  ~flareRasterConverter();

  Q_INVOKABLE QScriptValue toString();
  WRAPPER_STD_METHODS(flareRasterConverter)

  Q_PROPERTY(bool flatSource READ getFlatSource WRITE setFlatSource)
  bool getFlatSource() const { return m_flatSource; }
  void setFlatSource(bool v) { m_flatSource = v; }

  // Q_INVOKABLE QScriptValue convert(QScriptValue img);
  Q_INVOKABLE int foo(int x) { return x * 2; }

  QScriptValue convert(QScriptContext *context, QScriptEngine *engine);
};

}  // namespace TScriptBinding

Q_DECLARE_METATYPE(TScriptBinding::flareRasterConverter *)

#endif

