#pragma once

#include "tcommon.h"
#include "timage.h"
#include <QJsonArray>
#include <QString>
#include <QtGlobal>

#undef DVAPI
#ifdef TOONZLIB_EXPORTS
#define DVAPI DV_EXPORT_API
#else
#define DVAPI DV_IMPORT_API
#endif

class ToonzScene;
class TXsheet;

namespace FlashNative {

// An import report describes where source objects were first placed. It is
// deliberately not an immutable object identity: native edits can invalidate
// names, rows and columns. Only report a structural match after checking the
// current scene; never treat it as proof of unmodified source artwork.
struct SourceBindingCheck {
  quint64 layoutMatches = 0;
  // Persisted level and placement IDs remain traceable after native renames
  // and supported placement moves. Cell digests cannot certify unchanged
  // artwork or stage transforms.
  quint64 identityMatches = 0;
  quint64 relocated       = 0;
  quint64 stale           = 0;
  QJsonArray diagnostics;
  // Palette indices are locations: a binding resolves when its recorded
  // palette style index is still present and paged in the level palette.
  quint64 styleBindings           = 0;
  quint64 styleSlotsPresent       = 0;
  quint64 styleSlotsMissing       = 0;
  quint64 styleSlotsUnpaged       = 0;
  quint64 styleIdentityMatches    = 0;
  quint64 styleIdentityStale      = 0;
  quint64 styleIdentityUnverified = 0;
  QJsonArray styleDiagnostics;
  // A matching native ID does not imply that the drawing still contains its
  // imported artwork. These counters check a bounded image-content baseline.
  quint64 contentMatches    = 0;
  quint64 contentChanged    = 0;
  quint64 contentUnverified = 0;
  QJsonArray contentDiagnostics;
  // Stage placement is checked independently of level/placement identity and
  // drawing bytes. A changed effective placement is distinct from an
  // unverified key/parent history or a missing bounded baseline.
  quint64 transformMatches    = 0;
  quint64 transformChanged    = 0;
  quint64 transformUnverified = 0;
};

struct StageTransformBaseline {
  QString sampledPlacementSha256;
  QString stageKeysSha256;
};

// Samples effective stage placement at frame and half-frame offsets, and
// fingerprints native key/parent semantics. Empty fields mean the bounded
// inspection cannot certify this span.
DVAPI StageTransformBaseline stageTransformBaseline(TXsheet *sheet, int column,
                                                    int firstRow, int rowCount);

// Version 1 hashes native vector strokes, region topology/styles, or raster
// pixel bytes. Empty means unsupported image or the byte budget was exceeded.
DVAPI QString drawingContentDigest(const TImageP &image,
                                   quint64 maxBytes     = 512ull * 1024 * 1024,
                                   quint64 *bytesHashed = nullptr);

DVAPI SourceBindingCheck inspectSourceBindings(const ToonzScene &scene,
                                               const QJsonArray &bindings);

}  // namespace FlashNative
