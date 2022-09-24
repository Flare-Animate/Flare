#pragma once

#include "toonzqt/dvdialog.h"

#include <QLabel>
#include <QSlider>
#include <QPushButton>
#include <QBoxLayout>

class SeeThroughWindowPopup final : public DVGui::Dialog {
  Q_OBJECT

  int m_bckValue;
  QBoxLayout *m_layout;

  QSlider *m_opacitySlider;
  QPushButton *m_opacityBtn;
  QPushButton *m_closeBtn;
  QString m_suffixTxtSlider;

  QIcon m_seeThroughIcon_off;
  QIcon m_seeThroughIcon_on;

public:
  SeeThroughWindowPopup();

  void toggleMode();

private:
  void showEvent(QShowEvent *e) override;
  void hideEvent(QHideEvent *e) override;
  void resizeEvent(QResizeEvent *) override;

  int getOpacitySlider();
  int setOpacitySlider(int opacity);

private slots:
  void sliderChanged(int value);
  void opacityToggle();
};
