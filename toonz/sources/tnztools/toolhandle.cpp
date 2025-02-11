

#include "tools/toolhandle.h"
#include "toonz/stage2.h"
#include "tools/tool.h"
#include "tools/toolcommandids.h"
#include "timage.h"
// #include "tapp.h"
#include "toonzqt/menubarcommand.h"
#include "toonz/preferences.h"
#include <QGuiApplication>
#include <QAction>
#include <QMap>
#include <QDebug>

//=============================================================================
// ToolHandle
//-----------------------------------------------------------------------------

ToolHandle::ToolHandle()
    : m_tool(0)
    , m_toolName("")
    , m_toolTargetType(TTool::NoTarget)
    , m_storedToolName("")
    , m_toolIsBusy(false) {}

//-----------------------------------------------------------------------------

ToolHandle::~ToolHandle() {}

//-----------------------------------------------------------------------------

TTool *ToolHandle::getTool() const { return m_tool; }

//-----------------------------------------------------------------------------

void ToolHandle::setTool(QString name) {
  m_oldToolName = m_toolName = name;

  TTool *tool = TTool::getTool(m_toolName.toStdString(),
                               (TTool::ToolTargetType)m_toolTargetType);
  if (tool == m_tool) return;

  if (m_tool) m_tool->onDeactivate();

  // Camera test uses the automatically activated CameraTestTool
  if (name != "T_CameraTest" && CameraTestCheck::instance()->isEnabled())
    CameraTestCheck::instance()->setIsEnabled(false);

  m_tool = tool;

  if (name != T_Hand && CleanupPreviewCheck::instance()->isEnabled()) {
    // When using a tool, you have to exit from cleanup preview mode
    QAction *act = CommandManager::instance()->getAction("MI_CleanupPreview");
    if (act) CommandManager::instance()->execute(act);
  }

  if (m_tool)  // Should always enter
  {
    m_tool->onActivate();
    emit toolSwitched();
  }
}

//-----------------------------------------------------------------------------

void ToolHandle::setSpacePressed(bool pressed) {
  if (m_navState.spacePressed == pressed) return;

  m_navState.spacePressed = pressed;

  if (pressed) {
    // Enter navigation mode
    if (!m_navState.active) {
      m_navState.active       = true;
      m_navState.originalTool = m_toolName;
    }

    /**
     * Check actual keyboard state to detect "buffered" modifier keys
     * ______________________________________________________________
     *
     * Normally, we track modifier keys using 'setShiftPressed()' and
     * 'setCtrlPressed()' when the user presses them. However, there's a case
     * where a user [holds Shift or Ctrl before pressing Space]. In this case:
     *
     * - The Shift/Ctrl state might already be active 'before' space was pressed
     * - If we don't check the keyboard state here, we'd assume Shift/Ctrl off
     *  - This would result in the wrong tool being selected (e.g., T_Hand
     * instead of T_Rotate)
     *
     * Example scenario:
     * 1. User 'holds Shift' but 'setShiftPressed(true)' is not yet called
     * 2. Pressed 'Space' -> navigation activates
     * 3. If we don't check the keyboard here, we don't recognize Shift is held
     * 4. The tool defaults to 'T_Hand' instead of 'T_Rotate' (incorrect!)
     *
     * This prevents that issue by explicitly checking what modifier keys are
     * actually held.
     */
    m_navState.shiftPressed =
        (QGuiApplication::queryKeyboardModifiers() & Qt::ShiftModifier) != 0;
    m_navState.ctrlPressed =
        (QGuiApplication::queryKeyboardModifiers() & Qt::ControlModifier) != 0;

    updateNavigationTool(); // Select the correct tool based on modifiers
  } else {
    exitNavigation();
  }
}

void ToolHandle::setShiftPressed(bool pressed) {
  if (m_navState.shiftPressed == pressed) return;

  m_navState.shiftPressed = pressed;

  // If we're in navigation mode, update the tool selection
  if (m_navState.active) {
    updateNavigationTool();
  }
}

void ToolHandle::setCtrlPressed(bool pressed) {
  if (m_navState.ctrlPressed == pressed) return;

  m_navState.ctrlPressed = pressed;

  // If we're in navigation mode, update the tool selection
  if (m_navState.active) {
    updateNavigationTool();
  }
}

void ToolHandle::updateNavigationTool() {
  if (!m_navState.active) return;  // Only update if navigation mode is active

  // Priority: Ctrl > Shift > Default (T_Hand)
  if (m_navState.ctrlPressed) {
    setTool("T_Zoom");
  } else if (m_navState.shiftPressed) {
    setTool("T_Rotate");
  } else {
    setTool("T_Hand");
  }
}

void ToolHandle::exitNavigation() {
  if (!m_navState.active) return;

  // Restore the original tool before navigation mode was activated
  if (!m_navState.originalTool.isEmpty()) {
    setTool(m_navState.originalTool);
    m_storedToolName = m_navState.originalTool;
  }

  // Reset navigation mode but PRESERVE modifier states for buffering
  m_navState.active       = false;
  m_navState.spacePressed = false;
}

//-----------------------------------------------------------------------------

void ToolHandle::storeTool() {
  m_storedToolName = m_toolName;
  m_storedToolTime.start();
}

//-----------------------------------------------------------------------------

void ToolHandle::restoreTool() {
  // qDebug() << m_storedToolTime.elapsed();
  if (m_storedToolName != m_toolName && m_storedToolName != "" &&
      m_storedToolTime.elapsed() >
          Preferences::instance()->getTempToolSwitchTimer()) {
    setTool(m_storedToolName);
  }
}

//-----------------------------------------------------------------------------

void ToolHandle::setPseudoTool(QString name) {
  QString oldToolName = m_oldToolName;
  setTool(name);
  m_oldToolName = oldToolName;
}

//-----------------------------------------------------------------------------

void ToolHandle::unsetPseudoTool() {
  if (m_toolName != m_oldToolName) setTool(m_oldToolName);
}

//-----------------------------------------------------------------------------

void ToolHandle::setToolBusy(bool value) {
  m_toolIsBusy = value;
  if (!m_toolIsBusy) emit toolEditingFinished();
}

//-----------------------------------------------------------------------------

QIcon currentIcon;

static QIcon getCurrentIcon() { return currentIcon; }

//-----------------------------------------------------------------------------

/*
void ToolHandle::changeTool(QAction* action)
{
}
*/

//-----------------------------------------------------------------------------

void ToolHandle::onImageChanged(TImage::Type imageType) {
  TTool::ToolTargetType targetType = TTool::EmptyTarget;

  switch (imageType) {
  case TImage::RASTER:
    targetType = TTool::RasterImage;
    break;
  case TImage::TOONZ_RASTER:
    targetType = TTool::ToonzImage;
    break;
  case TImage::MESH:
    targetType = TTool::MeshImage;
    break;
  case TImage::META:
    targetType = TTool::MetaImage;
    break;
  case TImage::VECTOR:
    targetType = TTool::VectorImage;
    break;
  default:
    targetType = TTool::EmptyTarget;
    break;
  }

  if (targetType != m_toolTargetType) {
    m_toolTargetType = targetType;
    setTool(m_toolName);
  }

  if (m_tool) {
    m_tool->updateMatrix();
    m_tool->onImageChanged();
  }
}

//-----------------------------------------------------------------------------

void ToolHandle::updateMatrix() {
  if (m_tool) m_tool->updateMatrix();
}
