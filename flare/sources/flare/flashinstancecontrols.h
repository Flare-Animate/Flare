#pragma once

#include "flare/flashinstanceedit.h"
#include <QWidget>

class QComboBox;
class QLabel;
class QPushButton;
class QSpinBox;
class TXshLevelColumn;

// The editor works on native scene objects. The application pane supplies the
// current selection and lifetime-safe notifications.
class FlashInstanceControls : public QWidget {
  Q_OBJECT

public:
  explicit FlashInstanceControls(QWidget *parent = nullptr);
  void setContext(TXsheet *xsheet, int column, int row,
                  FlashInstanceEdit::Notify notify = {});

signals:
  void openSymbolRequested();

public slots:
  void updateControls();

private:
  TXsheetP m_xsheet;
  int m_column = -1, m_row = -1;
  QString m_sourceId;
  FlashInstanceEdit::Notify m_notify;
  QLabel *m_symbol, *m_span, *m_status, *m_frameLabel, *m_pivot;
  QComboBox *m_playback;
  QSpinBox *m_firstFrame, *m_lastFrame;
  std::optional<int> selectedLastFrame() const;
  QPushButton *m_apply, *m_refresh, *m_open;

  TXshLevelColumn *column() const;
  void updateDraft();
  void apply();
  void refresh();
  void commit(std::unique_ptr<FlashInstanceEdit> edit);
};
