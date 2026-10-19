#pragma once

#include "flashvector.h"
#include "tvectorimage.h"
#include "tgeometry.h"
#include "tpixel.h"
#include <QStringList>
#include <QVector>

class TPalette;
namespace Flash {
struct Element;
}

#undef DVAPI
#ifdef TOONZLIB_EXPORTS
#define DVAPI DV_EXPORT_API
#else
#define DVAPI DV_IMPORT_API
#endif

namespace FlashNative {

// Lays out static, dynamic or input text (its initial characters) with Qt
// and converts the glyph outlines into an editable drawing: one closed
// zero-thickness stroke per glyph contour, and the enclosed faces filled with
// the run colors (palette styles are shared by color). Coordinates are
// element-local, like vectorImage(); the builder applies the element matrix.
// Details that are drawn differently (a substituted font, for example) are
// appended to `approximations`. Throws Flash::Error when no font is usable.
// The optional audit records one paint witness per filled face (source fill
// = run index + 1) for the PLI save/reopen verification.
DVAPI TVectorImageP textImage(const Flash::Element &element, TPalette *palette,
                              double stageUnitsPerPixel,
                              QStringList *approximations = nullptr,
                              VectorFrameAudit *audit     = nullptr);

// One editable Text FX (Iwa_TextFx, "Transform with Column" on) drawing part
// of a text element. Its column is placed with the element matrix times
// scale(columnScale); every value below is in that column's space, in FX
// length units (stage units, y up). The FX centers the laid-out text on
// `center` and wraps it at `width`; `height` is its box height.
struct TextFxItem {
  QString text;  // Lines separated by '\n'
  QString font;  // QFont::toString(); its pixel size is in FX length units
  TPixel32 color;
  int alignment = 0;  // Qt::AlignLeft, AlignHCenter, AlignRight or AlignJustify
  TPointD center;
  double width = 0, height = 0;
  // Font pixel sizes are integers: this scale makes the drawn size exact.
  double columnScale = 1;
  // Layout options of the FX (their defaults draw text as the stock FX).
  // Flash text never shrinks to fit its box.
  bool kerning = true, shrinkToFit = true;
  double letterSpacing = 0, lineSpacing = 0, indent = 0;
  int firstRun = 0, lastRun = 0;  // Source runs drawn by this FX
};

// Maps a text element to Text FX parameters, laid out like textImage(): one
// FX for single-style text (one font, size and color), otherwise one FX per
// run piece of each line, positioned from the line layout. Kerning, letter
// spacing, line spacing and first-line indent become FX layout options. The
// FX layout is checked against Flash's: text whose FX glyphs would lie more
// than half a pixel from the Flash glyphs (other line breaks, glyphs,
// spacing) is noted in `approximations`. Throws Flash::Error when no font is
// usable.
DVAPI QVector<TextFxItem> textFxItems(const Flash::Element &element,
                                      double stageUnitsPerPixel,
                                      QStringList *approximations = nullptr);

}  // namespace FlashNative
