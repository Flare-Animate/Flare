#pragma once

#ifndef XSHTOOLBAR_H
#define XSHTOOLBAR_H

#include <memory>

"flare/txsheet.h"
#include "commandbar.h"
"flareqt/keyframenavigator.h"

#include <QToolBar>

//-----------------------------------------------------------------------------

// forward declaration
class XsheetViewer;
class QAction;

//-----------------------------------------------------------------------------

namespace XsheetGUI {

//=============================================================================
// XSheet Toolbar
//-----------------------------------------------------------------------------

class XSheetToolbar final : public CommandBar {
  Q_OBJECT

  XsheetViewer *m_viewer;

public:
  XSheetToolbar(XsheetViewer *parent = 0, Qt::WindowFlags flags = Qt::WindowFlags(),
                bool isCollapsible = false);
  static void toggleXSheetToolbar();
  void showToolbar(bool show);

protected:
  void showEvent(QShowEvent *e) override;
  void contextMenuEvent(QContextMenuEvent *event) override;

protected slots:
  void doCustomizeCommandBar();
};

}  // namespace XsheetGUI

#endif  // XSHTOOLBAR_H

