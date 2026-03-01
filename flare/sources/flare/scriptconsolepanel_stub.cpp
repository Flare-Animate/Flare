#include "scriptconsolepanel.h"
#include "tapp.h"
"flareqt/tselectionhandle.h"
#include <QLabel>

ScriptConsolePanel::ScriptConsolePanel(QWidget *parent, Qt::WindowFlags flags)
    : TPanel(parent), m_scriptConsole(nullptr) {
  setPanelType("ScriptConsole");
  setIsMaximizable(false);
  setWindowTitle(QObject::tr("Script Console (disabled)"));

  // Inform the user that the scripting subsystem is not available
  QLabel *lbl = new QLabel(QObject::tr("Script support is disabled (QtScript not available)."), this);
  lbl->setAlignment(Qt::AlignCenter);
  setWidget(lbl);

  setMinimumHeight(80);
  allowMultipleInstances(false);
  resize(600, 200);
}

ScriptConsolePanel::~ScriptConsolePanel() {}

void ScriptConsolePanel::executeCommand(const QString & /*cmd*/) {
  // no-op when scripting subsystem is not present
}

void ScriptConsolePanel::selectNone() {
  TApp::instance()->getCurrentSelection()->setSelection(0);
}

