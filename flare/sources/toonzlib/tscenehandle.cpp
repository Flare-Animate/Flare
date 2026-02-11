#include <QTimer>

#include "flare/tscenehandle.h"

#include "flare/flarescene.h"
#include "flare/tproject.h"

//=============================================================================
// TSceneHandle
//-----------------------------------------------------------------------------

TSceneHandle::TSceneHandle() : m_scene(0), m_dirtyFlag(false) {}

//-----------------------------------------------------------------------------

TSceneHandle::~TSceneHandle() {}

//-----------------------------------------------------------------------------

flareScene *TSceneHandle::getScene() const { return m_scene; }

//-----------------------------------------------------------------------------

void TSceneHandle::setScene(flareScene *scene) {
  if (m_scene == scene) return;
  flareScene *oldscene = m_scene;
  m_scene = scene;
  if (m_scene) emit sceneSwitched();

  // Prevent memory corruption caused by delayed signals writing into the
  // discarded old scene while that memory was freed.
  // That made OT had a chance of crashing when project or scene changed rapidly.
  // Note: This is not the best solution but "it just works"
  if (oldscene) {
    QTimer *delayedTimer = new QTimer(this);
    delayedTimer->setSingleShot(true);

    connect(delayedTimer, &QTimer::timeout, [=]() {
      delete oldscene;
      delayedTimer->deleteLater();
    });
    delayedTimer->start(3000);  // 1 sec was enough, but... dunno about toasters
  }
}

