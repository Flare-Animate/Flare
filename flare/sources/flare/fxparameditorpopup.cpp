

#include "fxparameditorpopup.h"
"flareqt/menubarcommand.h"
#include "menubarcommandids.h"
#include "tapp.h"
"flare/tfxhandle.h"
"flare/tscenehandle.h"
"flare/tframehandle.h"
"flare/txsheethandle.h"
"flare/txshlevelhandle.h"

#include "tfxattributes.h"

"flareqt/fxsettings.h"

"flare/tcolumnfx.h"
"flare/scenefx.h"
"flare/preferences.h"

#include <QMainWindow>

using namespace DVGui;

//=============================================================================
/*! \class FxParamEditorPopup

                Inherits \b Dialog.
*/
//-----------------------------------------------------------------------------

FxParamEditorPopup::FxParamEditorPopup()
    : QDialog(TApp::instance()->getMainWindow()) {
  setWindowTitle(tr("Fx Settings"));
  setMinimumSize(20, 20);

  TApp *app            = TApp::instance();
  TSceneHandle *hScene = app->getCurrentScene();
  TPixel32 col1, col2;
  Preferences::instance()->getChessboardColors(col1, col2);

  FxSettings *fxSettings = new FxSettings(this, col1, col2);
  fxSettings->setSceneHandle(hScene);
  fxSettings->setFxHandle(app->getCurrentFx());
  fxSettings->setFrameHandle(app->getCurrentFrame());
  fxSettings->setXsheetHandle(app->getCurrentXsheet());
  fxSettings->setLevelHandle(app->getCurrentLevel());
  fxSettings->setObjectHandle(app->getCurrentObject());

  fxSettings->setCurrentFx();

  QVBoxLayout *mainLayout = new QVBoxLayout();
  mainLayout->setContentsMargins(0, 0, 0, 0);
  mainLayout->setSpacing(10);
  { mainLayout->addWidget(fxSettings); }
  setLayout(mainLayout);

  move(parentWidget()->geometry().center() - rect().bottomRight() / 2.0);
}

//=============================================================================
/*
OpenPopupCommandHandler<FxParamEditorPopup> openFxParamEditorPopup(
    MI_FxParamEditor);
*/
