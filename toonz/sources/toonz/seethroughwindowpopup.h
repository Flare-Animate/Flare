#pragma once

#include "toonzqt/dvdialog.h"

#include <QLabel>
#include <QSlider>
#include <QPushButton>

class SeeThroughWindowPopup final : public DVGui::Dialog {
  Q_OBJECT

  int m_bckValue;

  QLabel *m_opacityLabel;
  QSlider *m_opacitySlider;
  QPushButton *m_opacityBtn;
  QPushButton *m_closeBtn;

  QIcon m_seeThroughIcon_off;
  QIcon m_seeThroughIcon_on;

public:
  SeeThroughWindowPopup();

  void toggleMode();

private:
  void showEvent(QShowEvent *e) override;
  void hideEvent(QHideEvent *e) override;

  int getOpacitySlider();
  int setOpacitySlider(int opacity);

private slots:
  void sliderChanged(int value);
  void opacityToggle();
};
