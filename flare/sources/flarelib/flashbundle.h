#pragma once

#include "flare/toonzscene.h"
#include <QJsonObject>
#include <QString>
#include <functional>
#include <vector>

class TXshLevel;

namespace FlashNative {

struct ResolvedSourceBundle {
  QString reportPath;
  QString manifestPath;
  QString sourceEntryPath;
  QJsonObject report;
  QString fieldAccountingPath;
  QJsonObject fieldAccounting;
  QString fieldBindingsPath;
  QJsonObject fieldBindings;
};

// Never interpret a source report merely because its path exists. The scene
// reference and every retained payload must match their recorded SHA-256.
// This full verification hashes every retained payload: it is an explicit,
// opt-in operation and is never run by an ordinary save.
DVAPI bool validSourceBundleReference(
    const ToonzScene::FlashSourceBundle &reference);
DVAPI ResolvedSourceBundle resolveSourceBundle(const ToonzScene &scene);

// Returns the bundle reference to write into the TNZ being saved. Never
// throws and never fails the save. A save into the scene's own folder keeps
// the reference without reading the bundle (only a missing report drops it).
// Save As into another folder copies the source/report into a unique sibling
// directory, hashing members as they stream; if that is impossible the
// reference is dropped, a warning is logged and returned through `warning`.
DVAPI ToonzScene::FlashSourceBundle prepareSourceBundleSave(
    const ToonzScene &scene, const TFilePath &oldScenePath,
    const TFilePath &newScenePath, QString *warning);
// Removes a copy made by prepareSourceBundleSave when the TNZ commit failed.
DVAPI void discardSourceBundleCopy(
    const ToonzScene::FlashSourceBundle &original,
    const ToonzScene::FlashSourceBundle &saved,
    const QString &newSceneDirectory);

// Levels the portable Save As relocates: Flash-imported PLI/PNG drawings in
// the scene's levels folder. Every other level is left to the standard Save
// As handling (scene-folder prompts, sound, TLV, palettes, external paths).
DVAPI std::vector<TXshLevel *> portableSaveAsLevels(const ToonzScene &scene);

// Save As for a Flash-imported scene into another folder. Writes the
// relocatable Flash drawings (one frame at a time) to a unique destination
// folder, switches their aliases and saves the TNZ. Other resources are not
// touched. Without relocatable drawings this is exactly ToonzScene::save.
// On failure nothing new is left on disk and the active scene is unchanged.
DVAPI void saveImportedSceneAs(ToonzScene &scene, const TFilePath &newScenePath,
                               bool saveSceneIcon);

// Bounded retry only for transient Windows sharing/access denials. Every
// attempt checks the source and unoccupied destination; a caller may also
// recheck staging ownership before each attempt.
DVAPI void commitDirectoryNoReplaceWithRetry(
    const QString &source, const QString &destination, const QString &kind,
    const std::function<void()> &beforeAttempt = {});

}  // namespace FlashNative
