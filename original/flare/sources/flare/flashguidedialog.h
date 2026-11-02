#pragma once

#ifndef FLASHGUIDEDIALOG_H
#define FLASHGUIDEDIALOG_H

#include "flareqt/dvdialog.h"

class FlashGuideDialog : public DVGui::Dialog {
  Q_OBJECT
public:
  explicit FlashGuideDialog(QWidget *parent = nullptr);
};

#endif  // FLASHGUIDEDIALOG_H
