

#include "flare/scriptbinding_flare_raster_converter.h"
#include "flare/scriptbinding_level.h"
#include "tropcm.h"
#include "convert2tlv.h"

namespace TScriptBinding {

flareRasterConverter::flareRasterConverter() : m_flatSource(false) {}

flareRasterConverter::~flareRasterConverter() {}

QScriptValue flareRasterConverter::ctor(QScriptContext *context,
                                        QScriptEngine *engine) {
  return engine->newQObject(new flareRasterConverter(),
                            QScriptEngine::AutoOwnership);
}

QScriptValue flareRasterConverter::toString() { return "flareRasterConverter"; }

QScriptValue flareRasterConverter::convert(QScriptContext *context,
                                           QScriptEngine *engine) {
  if (context->argumentCount() != 1)
    return context->throwError(
        "Expected one argument (a raster Level or a raster Image)");
  QScriptValue arg = context->argument(0);
  Level *level     = qscriptvalue_cast<Level *>(arg);
  Image *img       = qscriptvalue_cast<Image *>(arg);
  QString type;
  if (level) {
    type = level->getType();
    if (type != "Raster")
      return context->throwError(tr("Can't convert a %1 level").arg(type));
    if (level->getFrameCount() <= 0)
      return context->throwError(tr("Can't convert a level with no frames"));
  } else if (img) {
    type = img->getType();
    if (type != "Raster")
      return context->throwError(tr("Can't convert a %1 image").arg(type));
  } else {
    return context->throwError(
        tr("Bad argument (%1): should be a raster Level or a raster Image")
            .arg(arg.toString()));
  }
  RasterToflareRasterConverter converter;
  if (img) {
    TRasterImageP ri = img->getImg();
    TflareImageP ti  = converter.convert(ri);
    ti->setPalette(converter.getPalette().getPointer());
    return create(engine, new Image(ti));
  } else
    return QScriptValue();
}

}  // namespace TScriptBinding

