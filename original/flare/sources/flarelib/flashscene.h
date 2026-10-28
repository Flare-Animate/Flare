#pragma once

#include "flare/toonzscene.h"
#include "../flash/document.h"
#include "../flash/geometry.h"
#include "../flash/timing.h"
#include "flashvector.h"

#include <QJsonArray>
#include <QMap>
#include <QSet>
#include <functional>
#include <memory>

class TXshSimpleLevel;

namespace Flash {
struct VideoPayload;
struct SoundPayload;
}  // namespace Flash

namespace FlashNative {

struct BuildOptions {
  // The caller supplies a fresh project asset directory. Building is in
  // memory, apart from decoded video frames (decodeVideo writes them to the
  // asset directory); no active application scene is changed.
  QString assetDirectory;
  std::shared_ptr<TProject> project;
  double dpi       = 120;
  quint64 maxCells = 20000000;
  // Shared bitmap library items count once, even if placed many times.
  // Individual payloads also have the format reader's 64M-pixel limit.
  quint64 maxBitmapPixels = 128ull * 1024 * 1024;
  int maxLevels = 200000, maxColumns = 200000;
  int maxProvenanceEntries = 1000000;
  // One drawable contour becomes one native PLI stroke. This exceeds the
  // decoded segment count of large documents, with headroom.
  quint64 maxStrokeBindings = 2000000;
  // Every certified source paint point is retained until its PLI frame has
  // been reopened. Exceeding this cap rejects instead of dropping evidence.
  quint64 maxPaintWitnesses = 4000000;
  // Unused library symbols are not part of the authored animation. Import
  // them only on request, as extra native levels.
  bool includeUnusedSymbols = false;
  // Strict builds reject the document on its first unmapped detail. A
  // best-effort build keeps everything it can map, approximates or omits the
  // rest (see Compatibility::handling) and lists each case in the report.
  bool bestEffort = false;
  // Flash shows guide layers inside symbols (often notes) only while that
  // symbol is edited. They are always imported; this makes them visible in
  // the editor wherever the symbol is placed.
  bool showSymbolGuides = false;
  // Called on the building thread while shapes convert: (done, total).
  std::function<void(int, int)> progress;
  // Writes an embedded video's frames as <directory>/<baseName>.0001.png ...
  // (one PNG per video frame, at its pixel size), calling `progress` with
  // the frames written; throws Flash::Error on failure. Importers run FFmpeg
  // (Flash::decodeVideoFrames). Unset: video instances are omitted and
  // reported as "video-decoder-unavailable".
  std::function<void(const Flash::VideoPayload &, const QString &directory,
                     const QString &baseName,
                     const std::function<void(int)> &progress)>
      decodeVideo;
  // Called on the building thread while a video decodes: (frames, total).
  std::function<void(int, int)> videoProgress;
  // Decodes a compressed (MP3) embedded sound into its `pcm`, sampleCount
  // samples of its format; throws Flash::Error on failure. Importers run
  // FFmpeg (Flash::decodeMp3Sound). Unset: MP3 sounds are omitted and
  // reported as "sound-decoder-unavailable".
  std::function<void(Flash::SoundPayload &)> decodeSound;
};

struct BuildResult {
  std::unique_ptr<ToonzScene> scene;
  // Stable source IDs to native level/column/frame identities at import time.
  // The original source members are retained separately in the import bundle.
  QJsonArray nativeSourceMap;
  // Native sound, video and shape tween counts for the import report.
  QJsonObject sourceReport;
  // Import-only certificates for all PLI frames; the bundle retains source
  // provenance separately after its staged files pass verification.
  QMap<TXshSimpleLevel *, QMap<int, VectorFrameAudit>> vectorFrameAudits;
  quint64 paintWitnesses = 0;
  // Best-effort builds: each approximated or omitted source detail, bounded,
  // plus complete per-code counts.
  QJsonArray degradations;
  QJsonObject degradationCounts;
  quint64 approximated = 0, omitted = 0;
  // Milliseconds spent in the main construction steps.
  QJsonObject profileMs;
  quint64 cells = 0;
  int columns = 0, drawingLevels = 0, childLevels = 0;
  // Raster levels of decoded video frames: their PNG files are already in
  // the asset directory, so importers verify them instead of writing them.
  QSet<TXshSimpleLevel *> videoLevels;
};

// Builds shared native symbols, editable drawing levels and per-placement
// cells/transform keys. Strict builds fail on unsupported features with a
// source location; best-effort builds degrade them as reported. Graphic
// instance bindings are stored in native columns, where the instance controls
// and their undo commands operate.
DVAPI BuildResult buildScene(Flash::Package &package,
                             const Flash::DocumentInfo &info,
                             const BuildOptions &options);

}  // namespace FlashNative
