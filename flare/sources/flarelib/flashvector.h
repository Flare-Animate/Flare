#pragma once

#include "tvectorimage.h"
#include "traster.h"
#include <QVector>
#include <QString>
#include <functional>

class TPalette;
namespace Flash {
struct Element;
struct Affine;
struct Paint;
}  // namespace Flash

#undef DVAPI
#ifdef FLARELIB_EXPORTS
#define DVAPI DV_EXPORT_API
#else
#define DVAPI DV_IMPORT_API
#endif

namespace FlashNative {

// A source style used by one converted drawing. Palette indices may be shared
// when distinct source paints have identical RGBA; keep a record for each
// authored fill/stroke style so that this many-to-one mapping is explicit.
struct VectorStyleBinding {
  QString role;  // "fill" or "stroke"
  QString sourceStyleId;
  QString sourcePaintId;
  int sourceStyleIndex        = 0;
  int nativePaletteStyleIndex = 0;
};

// Import-time location of one drawable source contour in a PLI drawing.
// The source Edge can have several contours and the same Shape can also have
// cubic-only Edge records. Stroke indexes are locations, not persistent stroke
// identities after a native edit; the source package retains the exact XML.
struct VectorStrokeBinding {
  QString sourceEdgeId;
  QString sourceEdgeXmlId;
  int sourceEdgeIndex    = -1;
  int sourceContourIndex = -1;
  int sourceFirstSegment = -1;
  int sourceSegmentCount = 0;
  int nativeStrokeIndex  = -1;
  int nativeFirstChunk   = 0;
  int nativeChunkCount   = 0;
  int nativeControlCount = 0;
};

// A source-certified paint sample in the drawing's local native coordinates.
// The face number records identity within the unsaved image, rather than an
// unstable native region pointer or an assumed order after PLI reopening.
struct VectorPaintWitness {
  TPointD point;
  int sourceFillIndex         = 0;
  int nativePaletteStyleIndex = 0;
  int nativeFace = -1;  // -1 means no native face at this source point.
};

struct VectorFrameAudit {
  QString sourceId;
  QVector<VectorPaintWitness> paint;
  // Flattened native face order at conversion, including child faces. Keep
  // authored fill IDs independently of point samples so consistent relabeling
  // of every witness cannot erase a source paint identity.
  QVector<int> sourceFillByFace;
  // Recorded independently of the witness list so a dropped boundary sample
  // cannot silently weaken the save/reopen check.
  int expectedPaintWitnesses = 0;
  // Best-effort fallback: faces were filled by sampling one interior point
  // each, without the exact topology certificate. Not save/reopen audited.
  bool approximatedFills = false;
  QString approximationReason;  // why the exact certificate failed
};

// Maps an XFL gradient paint matrix to the TFlashGradientStyle field of a
// drawing whose local units are stageUnitsPerPixel per source pixel (Y up).
DVAPI TAffine gradientFieldToLocal(const Flash::Affine &matrix,
                                   double stageUnitsPerPixel);

// The bitmap of a BitmapFill paint (its bitmapPath library item): straight
// RGBA with bottom-up rows, and whether the item allows smoothing. An empty
// raster when it cannot be decoded; the resolver reports why.
struct VectorBitmap {
  TRaster32P pixels;
  bool smooth = false;
};
using VectorBitmapResolver = std::function<VectorBitmap(const Flash::Paint &)>;

// Adds the palette styles vectorImage() would create for this element, in
// the same order, so conversions can then run concurrently on that palette.
// Pass the same stageUnitsPerPixel as the later vectorImage() call: gradient
// styles are placed in drawing coordinates.
// Bitmap fills take their bitmaps from `bitmaps`; without one (or when it
// cannot decode a bitmap) they keep a reported solid fallback.
DVAPI void registerVectorPaints(const Flash::Element &element,
                                TPalette *palette, double stageUnitsPerPixel,
                                const VectorBitmapResolver &bitmaps = {});

// Converts local drawing geometry. The scene builder owns instance placement
// and applies the source matrix separately, preserving its editable controls.
// Paints without a faithful native mapping fail in strict mode; with
// approximatePaint they keep a reported approximation.
DVAPI TVectorImageP
vectorImage(const Flash::Element &element, TPalette *palette,
            double stageUnitsPerPixel, const std::function<void()> &check = {},
            QVector<VectorStyleBinding> *sourceStyles   = nullptr,
            QVector<VectorStrokeBinding> *sourceStrokes = nullptr,
            VectorFrameAudit *frameAudit                = nullptr,
            quint64 maxPaintWitnesses = 4000000, bool approximatePaint = false);

// Called on every staged PLI frame with an exact fill certificate after the
// native writer closes. The original in-memory image remains available until
// the bundle is committed.
// This checks the editable vector structure and each certified source paint
// point against the frame that Flare actually reopened.
DVAPI void verifyVectorFramePersistence(
    const TVectorImageP &original, const TVectorImageP &reopened,
    const VectorFrameAudit &audit, const std::function<void()> &check = {});

}  // namespace FlashNative
