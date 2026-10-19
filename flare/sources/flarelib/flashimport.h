#pragma once

#include "flashscene.h"

#include <functional>

namespace FlashNative {

struct ImportOptions {
  // A new, nonexisting absolute directory whose parent already exists.
  QString destinationDirectory;
  std::shared_ptr<TProject> project;
  double dpi                = 120;
  bool includeUnusedSymbols = false;
  // Keep everything that maps natively and report approximations/omissions
  // (see BuildOptions::bestEffort). Strict imports reject on any issue.
  bool bestEffort       = true;
  bool showSymbolGuides = false;  // see BuildOptions
  Flash::Limits limits;
  // Independent bounded streaming budget for exact original source files.
  quint64 maxSourceSnapshotBytes = 2ull * 1024 * 1024 * 1024;
  // Called on the importing thread. Cancellation is checked after callbacks.
  std::function<void(const QString &, int, int)> progress;
  // Decodes embedded video (see BuildOptions::decodeVideo). Unset: FFmpeg
  // from Preferences > Import/Export, the PATH or the application folder.
  std::function<void(const Flash::VideoPayload &, const QString &,
                     const QString &, const std::function<void(int)> &)>
      decodeVideo;
  // Decodes compressed embedded sound (see BuildOptions::decodeSound).
  // Unset: FFmpeg found as for video.
  std::function<void(Flash::SoundPayload &)> decodeSound;
};

struct ImportResult {
  TFilePath scenePath;
  QString reportPath;
  QJsonObject report;
};

// Reads the source and writes an independent native scene bundle through the
// existing PLI/TNZ writers. Source compatibility is checked before creating
// native assets, and the bounded report is preserved in import-report.json.
// Every successful bundle also retains the original archive bytes or all safe
// uncompressed package files under source/, with a SHA-256 manifest. Native
// construction reads that verified snapshot. Source snapshots retain data;
// they do not establish editable native support for unsupported source fields.
// The caller's active scene is never changed. Native
// scene assembly stays on the calling thread; applications call this on their
// GUI thread. Before commit, failures/cancellation remove only owned staging
// files. Commit never replaces an existing destination, including an empty
// directory or symlink. No callbacks or cancellation checks run after commit.
DVAPI ImportResult importDocument(const QString &sourcePath,
                                  const ImportOptions &options);

// Sets `build`'s video and sound decoders from `options`, or FFmpeg-backed
// ones when the document has videos or sounds and FFmpeg is found, as
// importDocument() does. Inspection uses it so its findings match the import.
DVAPI void attachMediaDecoders(BuildOptions &build,
                               const Flash::DocumentInfo &info,
                               const ImportOptions &options);

// Overall import progress in [0, 1] for a progress callback's stage and
// counts. Stage spans are weighted by each stage's typical share of the
// import time.
DVAPI double importProgressFraction(const QString &stage, int current,
                                    int total);

}  // namespace FlashNative
