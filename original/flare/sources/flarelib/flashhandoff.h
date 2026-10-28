#pragma once

#include "flashscene.h"

#include <functional>

namespace FlashNative {

struct ReadOptions {
  std::function<bool()> cancelled;
  std::function<void(int, int)> progress;
};

// Prepares an imported bundle independently of the application scene. Only the
// currently generated native PLI/child-level resources are accepted, and all
// drawing frames must decode before return. Paths stay inside the bundle;
// project association is supplied by the caller without switching projects.
// No global image-cache clear, active-scene change or undo reset occurs. The
// caller must retain the candidate's ImageManager bindings during activation.
DVAPI std::unique_ptr<ToonzScene> readImportedScene(
    const TFilePath &scenePath, std::shared_ptr<TProject> project,
    const ReadOptions &options = ReadOptions());

}  // namespace FlashNative
