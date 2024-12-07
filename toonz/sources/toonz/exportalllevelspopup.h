#pragma once

#ifndef EXPORTALLLEVELSPOPUP_H
#define EXPORTALLLEVELSPOPUP_H

#include "exportlevelpopup.h"

class ExportAllLevelsPopup final : public ExportLevelPopup {
  Q_OBJECT

public:
  ExportAllLevelsPopup();
  ~ExportAllLevelsPopup();
  bool isexport_all;    // set to false when "Export All" be dis-checked
  bool execute() override;  // overrride FileBrowserPopup::execute()

private:
  int level_exported = 0;
  ToonzScene *scene;

  // Widgets
private:
  DVGui::CheckBox *m_exportall;
  QPushButton *m_skipButton;

private slots:
  void GetSelectedSimpLevels() override;
  void showEvent(QShowEvent *se) override;
  void updateOnSelection() override;
  void onExportAll(bool toggled);
  void skip();

  QString getLevelTypeName(int type);
};

#endif  // EXPORTALLLEVELSPOPUP_H