#include "flashinstancepane.h"
#include "floatingpanelcommand.h"
#include "menubarcommandids.h"
#include "pane.h"
#include "tapp.h"
#include "flare/tcolumnhandle.h"
#include "flare/tframehandle.h"
#include "flare/tscenehandle.h"
#include "flare/txsheethandle.h"
#include <QAction>
#include <QPointer>

FlashInstancePane::FlashInstancePane(QWidget *parent)
    : FlashInstanceControls(parent) {
  connect(this, &FlashInstanceControls::openSymbolRequested, this, [] {
    auto *action = CommandManager::instance()->getAction(MI_OpenChild);
    if (action) action->trigger();
  });
}

void FlashInstancePane::syncContext() {
  auto *app = TApp::instance();
  if (!app->getCurrentFrame()->isEditingScene()) {
    setContext(nullptr, -1, -1);
    return;
  }
  QPointer<TSceneHandle> sceneHandle(app->getCurrentScene());
  QPointer<TXsheetHandle> xsheetHandle(app->getCurrentXsheet());
  auto *scene = sceneHandle->getScene();
  setContext(
      xsheetHandle->getXsheet(), app->getCurrentColumn()->getColumnIndex(),
      app->getCurrentFrame()->getFrame(), [sceneHandle, xsheetHandle, scene] {
        // Commands outlive the panel and can target a nested xsheet
        // while another sheet is open. Avoid a dangling widget capture.
        if (!sceneHandle || sceneHandle->getScene() != scene) return;
        sceneHandle->setDirtyFlag(true);
        if (xsheetHandle) xsheetHandle->notifyXsheetChanged();
      });
}

void FlashInstancePane::showEvent(QShowEvent *event) {
  FlashInstanceControls::showEvent(event);
  auto *app = TApp::instance();
  connect(app->getCurrentScene(), &TSceneHandle::sceneSwitched, this,
          &FlashInstancePane::syncContext);
  connect(app->getCurrentScene(), &TSceneHandle::castChanged, this,
          &FlashInstancePane::syncContext);
  connect(app->getCurrentXsheet(), &TXsheetHandle::xsheetSwitched, this,
          &FlashInstancePane::syncContext);
  connect(app->getCurrentXsheet(), &TXsheetHandle::xsheetChanged, this,
          &FlashInstancePane::syncContext);
  connect(app->getCurrentColumn(), &TColumnHandle::columnIndexSwitched, this,
          &FlashInstancePane::syncContext);
  connect(app->getCurrentFrame(), &TFrameHandle::frameSwitched, this,
          &FlashInstancePane::syncContext);
  syncContext();
}

void FlashInstancePane::hideEvent(QHideEvent *event) {
  auto *app = TApp::instance();
  disconnect(app->getCurrentScene(), nullptr, this, nullptr);
  disconnect(app->getCurrentXsheet(), nullptr, this, nullptr);
  disconnect(app->getCurrentColumn(), nullptr, this, nullptr);
  disconnect(app->getCurrentFrame(), nullptr, this, nullptr);
  setContext(nullptr, -1, -1);
  FlashInstanceControls::hideEvent(event);
}

namespace {
class FlashInstanceFactory final : public TPanelFactory {
public:
  FlashInstanceFactory() : TPanelFactory("FlashInstancePanel") {}
  TPanel *createPanel(QWidget *parent) override {
    auto *panel = new TPanel(parent);
    panel->setWidget(new FlashInstancePane(panel));
    panel->setObjectName(getPanelType());
    panel->setWindowTitle(QObject::tr("Flash Instance"));
    panel->allowMultipleInstances(false);
    panel->setIsMaximizable(false);
    panel->setMinimumSize(300, 310);
    return panel;
  }
  void initialize(TPanel *) override {}
} flashInstanceFactory;

OpenFloatingPanel openFlashInstanceCommand(MI_OpenFlashInstancePanel,
                                           "FlashInstancePanel",
                                           QObject::tr("Flash Instance"));
}  // namespace
