#pragma once

#include "flashinstancecontrols.h"

class FlashInstancePane final : public FlashInstanceControls {
  Q_OBJECT

public:
  explicit FlashInstancePane(QWidget *parent = nullptr);

protected:
  void showEvent(QShowEvent *event) override;
  void hideEvent(QHideEvent *event) override;

private:
  void syncContext();
};
