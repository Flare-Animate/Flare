#include "flashhandoff.h"
#include "flashloadingrange.h"

#include "flare/levelproperties.h"
#include "flare/levelset.h"
#include "flare/txsheet.h"
#include "flare/txshcell.h"
#include "flare/txshchildlevel.h"
#include "flare/txshlevelcolumn.h"
#include "flare/txshleveltypes.h"
#include "flare/txshsimplelevel.h"
#include "flare/txshsoundcolumn.h"
#include "flare/txshsoundlevel.h"
#include "flare/txshzeraryfxcolumn.h"
#include "flare/tcolumnfx.h"
#include "tlevel_io.h"
#include "tpalette.h"
#include "trasterimage.h"
#include "tvectorimage.h"

#include <QDir>
#include <QDirIterator>
#include <QThread>
#include <QFileInfo>
#include <QSet>
#include <QVector>
#include <algorithm>
#include <atomic>
#include <chrono>
#include <thread>

namespace FlashNative {
namespace {

void checkCancelled(const ReadOptions &options) {
  if (options.cancelled && options.cancelled())
    throw Flash::Error("Opening imported scene canceled.");
}

void progress(const ReadOptions &options, int completed, int total) {
  checkCancelled(options);
  if (options.progress) options.progress(completed, total);
  checkCancelled(options);
}

QString nativeString(const TFilePath &path) {
  return QString::fromStdWString(path.getWideString());
}

void requireRelative(const QString &relative) {
  const auto components = relative.split('/');
  if (relative.isEmpty() || QDir::isAbsolutePath(relative) ||
      relative.contains(':') || relative.contains('\\') ||
      relative.contains(QChar(0)))
    throw Flash::Error("Imported resource path is outside its native bundle.");
  for (const auto &component : components)
    if (component.isEmpty() || component == "." || component == "..")
      throw Flash::Error(
          "Imported resource path is outside its native bundle.");
}

QString containedFile(const QString &root, const QString &relative) {
  requireRelative(relative);
  const QFileInfo file(QDir(root).filePath(relative));
  const QString canonical = file.canonicalFilePath();
  const QString resolved  = QDir(root).relativeFilePath(canonical);
  if (!file.isFile() || file.isSymLink() || canonical.isEmpty() ||
      resolved == ".." || resolved.startsWith("../") ||
      QDir::isAbsolutePath(resolved))
    throw Flash::Error("Imported resource is missing or outside its bundle: " +
                       relative);
  return canonical;
}

void validateCells(ToonzScene &scene, const ReadOptions &options) {
  QSet<TXshLevel *> levels;
  QVector<TXsheet *> sheets{scene.getTopXsheet()};
  for (int index = 0; index < scene.getLevelSet()->getLevelCount(); ++index) {
    auto *level = scene.getLevelSet()->getLevel(index);
    levels.insert(level);
    if (auto *child = level->getChildLevel())
      sheets.push_back(child->getXsheet());
  }
  QSet<TXsheet *> checked;
  for (auto *sheet : sheets) {
    checkCancelled(options);
    if (!sheet || checked.contains(sheet)) continue;
    checked.insert(sheet);
    sheet->updateFrameCount();
    for (int index = 0; index < sheet->getColumnCount(); ++index) {
      auto *column = sheet->getColumn(index);
      if (!column) continue;
      if (auto *sound = column->getSoundColumn()) {
        int first = 0, last = -1;
        sound->getRange(first, last);
        for (int row = first; row <= last; ++row) {
          if ((row & 1023) == 0) checkCancelled(options);
          const TXshCell cell = sound->getSoundCell(row);
          if (!cell.isEmpty() && !levels.contains(cell.m_level.getPointer()))
            throw Flash::Error("Imported sound references a missing level.");
        }
        continue;
      }
      // Text FX columns expose their own FX; their cells name no level.
      if (auto *fx = column->getZeraryFxColumn()) {
        if (!fx->getZeraryColumnFx()->getZeraryFx())
          throw Flash::Error("Imported FX column has no FX.");
        continue;
      }
      auto *cells = column->getLevelColumn();
      if (!cells)
        throw Flash::Error("Imported bundle contains an unsupported column.");
      int first = 0, last = -1;
      cells->getRange(first, last);
      for (int row = first; row <= last; ++row) {
        if ((row & 1023) == 0) checkCancelled(options);
        const auto &cell = cells->getCell(row);
        if (cell.isEmpty()) continue;
        auto *level = cell.m_level.getPointer();
        if (!levels.contains(level))
          throw Flash::Error("Imported exposure references a missing level.");
        if (auto *drawing = level->getSimpleLevel()) {
          if (!drawing->isFid(cell.m_frameId))
            throw Flash::Error(
                "Imported exposure references a missing drawing.");
        } else if (auto *child = level->getChildLevel()) {
          const int frame = cell.m_frameId.getNumber();
          if (frame < 1 || frame > child->getFrameCount() ||
              !cell.m_frameId.getLetter().isEmpty())
            throw Flash::Error(
                "Imported exposure references a missing symbol pose.");
        }
      }
    }
  }
}

// Decodes every frame of a raster level on all cores without caching it: a
// video level holds hundreds of full-size frames. Only the calling thread
// calls back into the application (cancellation).
void decodeRasterFrames(const TFilePath &path,
                        const std::vector<TFrameId> &frames,
                        const ReadOptions &options) {
  const int total = int(frames.size());
  std::atomic<int> next{0}, done{0};
  std::atomic<bool> stop{false}, failed{false};
  std::vector<std::thread> threads;
  const int workers =
      std::max(1, std::min({QThread::idealThreadCount(), 16, total}));
  for (int index = 0; index < workers && total; ++index)
    threads.emplace_back([&] {
      try {
        TLevelReaderP reader(path);
        reader->loadInfo();
        for (int job; !stop && (job = next++) < total; ++done)
          if (!TRasterImageP(reader->getFrameReader(frames[job])->load()))
            failed = stop = true;
      } catch (...) {
        failed = stop = true;
      }
    });
  const auto join = [&] {
    stop = true;
    for (auto &thread : threads) thread.join();
  };
  try {
    while (done < total && !failed) {
      checkCancelled(options);
      std::this_thread::sleep_for(std::chrono::milliseconds(30));
    }
  } catch (...) {
    join();
    throw;
  }
  join();
  if (failed)
    throw Flash::Error("Imported drawing frame could not be decoded.");
}

}  // namespace

std::unique_ptr<ToonzScene> readImportedScene(const TFilePath &scenePath,
                                              std::shared_ptr<TProject> project,
                                              const ReadOptions &options) {
  detail::LoadingRangeGuard loadingRange;
  checkCancelled(options);
  const QFileInfo sceneFile(nativeString(scenePath));
  if (sceneFile.suffix().compare("tnz", Qt::CaseInsensitive) != 0 ||
      !sceneFile.isFile() || sceneFile.isSymLink())
    throw Flash::Error("Imported native scene is missing or invalid.");
  const QString root = sceneFile.dir().canonicalPath();
  if (root.isEmpty()) throw Flash::Error("Imported native bundle is missing.");
  // Native palettes and optional hook files can refer to neighboring files.
  // Reject linked files/directories throughout this self-contained bundle.
  QDirIterator members(
      root,
      QDir::AllEntries | QDir::NoDotAndDotDot | QDir::Hidden | QDir::System,
      QDirIterator::Subdirectories);
  while (members.hasNext()) {
    checkCancelled(options);
    const QFileInfo entry(members.next());
    if (entry.isSymLink())
      throw Flash::Error("Imported native bundle contains a linked resource.");
  }
  auto scene = std::make_unique<ToonzScene>();
  if (project) scene->setProject(std::move(project));
  scene->setScenePath(scenePath);
  scene->setIsLoading(true);
  scene->loadTnzFile(scenePath);
  QVector<TXshSimpleLevel *> drawings;
  QVector<TXshSoundLevel *> sounds;
  for (int index = 0; index < scene->getLevelSet()->getLevelCount(); ++index) {
    checkCancelled(options);
    auto *level = scene->getLevelSet()->getLevel(index);
    if (level->getChildLevel()) continue;
    // Imported sounds are WAV files beside the drawings (checked below).
    if (auto *sound = level->getSoundLevel()) {
      sounds.push_back(sound);
      continue;
    }
    auto *drawing     = level->getSimpleLevel();
    const bool vector = drawing && drawing->getType() == PLI_XSHLEVEL &&
                        drawing->getPath().getType() == "pli";
    const bool bitmap = drawing && drawing->getType() == OVL_XSHLEVEL &&
                        drawing->getPath().getType() == "png" &&
                        drawing->getPath().getDots() == "..";
    if ((!vector && !bitmap) || !drawing->getScannedPath().isEmpty())
      throw Flash::Error(
          "Imported bundle contains an unsupported native resource.");
    QString path = nativeString(drawing->getPath());
    path.replace('\\', '/');
    if (!path.startsWith("$scenefolder/levels/"))
      throw Flash::Error(
          "Imported drawing does not use its native bundle path.");
    const QString relative = path.mid(13);
    requireRelative(relative);
    const TFilePath decoded = scene->decodeFilePath(drawing->getPath());
    const QString absoluteParent =
        QFileInfo(nativeString(decoded)).absolutePath();
    const QString expectedParent =
        QFileInfo(QDir(root).filePath(relative)).absolutePath();
    const QString canonicalParent =
        QFileInfo(absoluteParent).canonicalFilePath();
    const QString within = QDir(root).relativeFilePath(canonicalParent);
    if (canonicalParent.isEmpty() || within == ".." ||
        within.startsWith("../") || QDir::isAbsolutePath(within) ||
        canonicalParent != QFileInfo(expectedParent).canonicalFilePath() ||
        QFileInfo(absoluteParent).isSymLink() ||
        (vector && QFileInfo(nativeString(decoded)).canonicalFilePath() !=
                       containedFile(root, relative)))
      throw Flash::Error(
          "Imported drawing path did not resolve inside its bundle.");
    drawings.push_back(drawing);
  }
  progress(options, 0, drawings.size());
  int completed = 0;
  for (auto *drawing : drawings) {
    checkCancelled(options);
    const auto path = scene->decodeFilePath(drawing->getPath());
    TLevelReaderP reader(path);
    const auto info   = reader->loadInfo();
    const bool vector = drawing->getType() == PLI_XSHLEVEL;
    if (!info || info->isPartialLoad() || (vector && !info->getPalette()) ||
        info->getFrameCount() == 0)
      throw Flash::Error("Imported drawing could not be read completely.");
    if (vector && !info->getPalette()->getRefImgPath().isEmpty())
      throw Flash::Error("Imported drawing has an external palette reference.");
    if (!vector) {
      const QString prefix = "$scenefolder/";
      QString alias        = nativeString(drawing->getPath());
      alias.replace('\\', '/');
      for (auto frame = info->begin(); frame != info->end(); ++frame) {
        checkCancelled(options);
        QString relative =
            nativeString(TFilePath(alias.mid(prefix.size()).toStdWString())
                             .withFrame(frame->first));
        relative.replace('\\', '/');
        const QString actual = containedFile(root, relative);
        if (QFileInfo(nativeString(path.withFrame(frame->first)))
                .canonicalFilePath() != actual)
          throw Flash::Error("Imported bitmap frame escaped its bundle.");
      }
    }
    drawing->load();
    if (drawing->getProperties()->isForbidden() ||
        drawing->getFrameCount() != info->getFrameCount())
      throw Flash::Error("Imported drawing could not be loaded completely.");
    if (!vector) {
      std::vector<TFrameId> frames;
      for (auto frame = info->begin(); frame != info->end(); ++frame)
        frames.push_back(frame->first);
      decodeRasterFrames(path, frames, options);
    } else
      for (auto frame = info->begin(); frame != info->end(); ++frame) {
        checkCancelled(options);
        TImageP image = drawing->getFrame(frame->first, false);
        if (!TVectorImageP(image) || !image->getPalette())
          throw Flash::Error("Imported drawing frame could not be decoded.");
      }
    progress(options, ++completed, drawings.size());
  }
  for (auto *sound : sounds) {
    checkCancelled(options);
    QString path = nativeString(sound->getPath());
    path.replace('\\', '/');
    if (!path.startsWith("$scenefolder/levels/") ||
        !path.endsWith(".wav", Qt::CaseInsensitive))
      throw Flash::Error("Imported sound does not use its native bundle path.");
    const QString actual = containedFile(root, path.mid(13));
    if (QFileInfo(nativeString(scene->decodeFilePath(sound->getPath())))
            .canonicalFilePath() != actual)
      throw Flash::Error(
          "Imported sound path did not resolve inside its bundle.");
    sound->load();
    if (!sound->getSoundTrack())
      throw Flash::Error("Imported sound could not be read: " + path);
  }
  validateCells(*scene, options);
  scene->getTopXsheet()->updateFrameCount();
  scene->setIsLoading(false);
  checkCancelled(options);
  return scene;
}

}  // namespace FlashNative
