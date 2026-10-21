#pragma once

#ifndef FLASHIMPORTPOPUP_H
#define FLASHIMPORTPOPUP_H

#include "flareqt/dvdialog.h"

#include <QElapsedTimer>
#include <functional>

class QCheckBox;
class QLabel;
class QLineEdit;
class QPlainTextEdit;
class QProgressDialog;
class QPushButton;

namespace DVGui {
class FileField;
}

// Uses the existing Flare file chooser, native scene importer and scene
// loader. Reading/conversion never replaces the current application scene.
class FlashImportPopup final : public DVGui::Dialog {
  Q_OBJECT

  QWidget *m_inputs               = nullptr;
  DVGui::FileField *m_source      = nullptr;
  DVGui::FileField *m_destination = nullptr;
  QLineEdit *m_folderName         = nullptr;
  QCheckBox *m_symbolGuides       = nullptr;
  QCheckBox *m_unusedSymbols      = nullptr;
  QLabel *m_destinationStatus     = nullptr;
  QLabel *m_summary               = nullptr;
  QPlainTextEdit *m_diagnostics   = nullptr;
  QPushButton *m_inspect          = nullptr;
  QPushButton *m_import           = nullptr;
  QPushButton *m_close            = nullptr;
  QProgressDialog *m_progress     = nullptr;
  QElapsedTimer m_eventTimer;
  QString m_inspectedSource;
  QString m_inspectedStamp;
  bool m_inspectedUnused = true;
  bool m_canImport       = false;
  bool m_busy            = false;
  bool m_cancelRequested = false;

public:
  FlashImportPopup();

protected:
  void reject() override;

private:
  QString sourcePath() const;
  QString destinationPath() const;
  QString destinationProblem() const;
  void sourceChanged();
  void invalidateInspection();
  void updateActions();
  void inspectSource();
  void importSource();
  void withProgress(const QString &title,
                    const std::function<void()> &operation);
  bool cancelled();
  void showFailure(const QString &message);
};

void openFlashImportPopup();

#endif
