#pragma once

#ifndef flareIMAGE_UTILS_INCLUDED
#define flareIMAGE_UTILS_INCLUDED

#include "tpalette.h"
#include "trasterfx.h"
#include <set>

#undef DVAPI
#undef DVVAR
#ifdef flareLIB_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

class TStroke;
class TVectorImageP;
class TColorStyle;
class TWidget;
class TFilePath;
class TTileSetCM32;
class TVectorPalette;
class TflareImageP;
class TXshLevel;

namespace flareImageUtils {

DVAPI TRect convertWorldToRaster(const TRectD area, const TflareImageP ti);
DVAPI TRectD convertRasterToWorld(const TRect area, const TflareImageP ti);

// Disegna una ink stroke sulla flareImage con il colore inkId.
// Se e' vero selective, disegna solo sulle zone senza altri ink.
// Se e' vero filled, riempe le parti chiuse (sempre con inkId).
// ritorna la bounding box (coordinate image) dell'area modificata
DVAPI TRect addInkStroke(const TflareImageP &ti, TStroke *stroke, int inkId,
                         bool selective, bool filled, TRectD clip = TRectD(),
                         bool doAntialiasing = true);

//-------------------------------------------------------------------

struct ChangeColorStrokeSettings {
  TStroke *stroke;
  int colorId;
  int maskPaintId;
  bool changeInk;
  bool changePaint;
  TRectD clip;
};

//-------------------------------------------------------------------

DVAPI TRect changeColorStroke(const TflareImageP &ti,
                              const ChangeColorStrokeSettings &settings);

//-------------------------------------------------------------------

// Cancella gli ink in corrispondenza della stroke.
// Se maskInkId!=-1, cancella solo dove il vecchio ink e' uguale a maskInkId.
// void eraseInkStroke(const TflareImageP &ti, TStroke *stroke, int maskInkId,
// TRectD clip=TRectD());

// Cancella l'ink e/o il paint in corrispondenza della stroke, se
// maskStyleId!=-1, solo se
// sono uguali a maskStykeId (indipendentemente).
// DVAPI  void eraseInkAndPaint(const TflareImageP &ti, TStroke *stroke,
//                               int maskId, bool inkOnly, bool paintOnly,
//                               TRectD clip=TRectD());

// Cambia tutti gli ink della clip area con inkId.
// void changeInkRect    (const TflareImageP &ti, const TRectD &clip, int
// inkId);

// cancella r ink, paint e inkAndPaint.
// Se lo styleId!=-1, cancella solo se sono uguali a questo stile
DVAPI TRect eraseRect(const TflareImageP &ti, const TRectD &area, int maskId,
                      bool onInk, bool onPaint);

DVAPI std::vector<TRect> paste(const TflareImageP &ti,
                               const TTileSetCM32 *tileSet);

// DVAPI  void updateRas32(const TflareImageP &img, TRect clipRect=TRect());
// DVAPI  void updateRas32(const TflareImageP &img, const TTileSetCM32
// *tileSet);

const TVectorPalette *getTCheckPalette();

DVAPI TflareImageP vectorToflareImage(
    const TVectorImageP &vi, const TAffine &aff, TPalette *palette,
    const TPointD &outputPos, const TDimension &outputSize,
    const std::vector<TRasterFxRenderDataP> *fxs = 0,
    bool transformThickness                      = false);

DVAPI TPalette *loadTzPalette(const TFilePath &pltFile);

DVAPI void getUsedStyles(std::set<int> &styles, const TflareImageP &ti);
DVAPI void scrambleStyles(const TflareImageP &ti,
                          std::map<int, int> styleTable);
DVAPI std::string premultiply(const TFilePath &levelPath);
// DVAPI bool convertToTlv(const TFilePath& levelPathIn);
DVAPI void eraseImage(const TflareImageP &ti, const TRaster32P &image,
                      const TPoint &pos, bool invert, bool eraseInk,
                      bool erasePaint, bool selective, int styleId);
}

#endif

