#include "flashinstancecontrols.h"
#include "flare/txshlevelcolumn.h"
#include "flare/txshchildlevel.h"
#include "flare/txshcell.h"
#include "texception.h"
#include <QComboBox>
#include <QFormLayout>
#include <QHBoxLayout>
#include <QLabel>
#include <QPushButton>
#include <QSignalBlocker>
#include <QSpinBox>
#include <QVBoxLayout>
#include <algorithm>
#include <utility>

namespace {
QLabel *label(QWidget *parent, const char *name) {
  auto *result = new QLabel(parent);
  result->setObjectName(name);
  result->setTextFormat(Qt::PlainText);
  result->setWordWrap(true);
  auto policy = result->sizePolicy();
  policy.setVerticalPolicy(QSizePolicy::Minimum);
  result->setSizePolicy(policy);
  return result;
}
}  // namespace

FlashInstanceControls::FlashInstanceControls(QWidget *parent)
    : QWidget(parent) {
  setObjectName("FlashInstanceControls");
  m_symbol = label(this, "flashSymbolName");
  m_symbol->setTextInteractionFlags(Qt::TextSelectableByMouse);
  m_span       = label(this, "flashInstanceSpan");
  m_status     = label(this, "flashInstanceStatus");
  m_pivot      = label(this, "flashSourcePivot");
  m_frameLabel = label(this, "flashFirstFrameLabel");
  // This is a form caption, not a wrapping description. Its complete text
  // determines the label-column width when playback switches to a pose.
  m_frameLabel->setWordWrap(false);
  m_frameLabel->setSizePolicy(QSizePolicy::Minimum, QSizePolicy::Fixed);
  m_playback = new QComboBox(this);
  m_playback->setObjectName("flashPlaybackMode");
  m_playback->addItem(tr("Loop"), int(Flash::PlaybackMode::Loop));
  m_playback->addItem(tr("Play Once"), int(Flash::PlaybackMode::PlayOnce));
  m_playback->addItem(tr("Single Frame"),
                      int(Flash::PlaybackMode::SingleFrame));
  m_playback->addItem(tr("Loop Reverse"),
                      int(Flash::PlaybackMode::LoopReverse));
  m_playback->addItem(tr("Play Once Reverse"),
                      int(Flash::PlaybackMode::PlayOnceReverse));
  m_playback->setToolTip(
      tr("Playback applies to this instance. Other instances "
         "continue to share the same symbol artwork."));
  m_firstFrame = new QSpinBox(this);
  m_firstFrame->setObjectName("flashFirstFrame");
  m_firstFrame->setKeyboardTracking(false);
  m_firstFrame->setMinimum(1);
  m_firstFrame->setToolTip(
      tr("Symbol frame numbers start at 1, as in the "
         "Flare timeline."));
  m_lastFrame = new QSpinBox(this);
  m_lastFrame->setObjectName("flashLastFrame");
  m_lastFrame->setKeyboardTracking(false);
  m_lastFrame->setMinimum(0);
  m_lastFrame->setSpecialValueText(tr("Whole symbol"));
  m_lastFrame->setToolTip(
      tr("Limit playback to the first..last frame range. Reverse modes "
         "count down from the first frame to the last frame."));
  m_apply = new QPushButton(tr("Apply"), this);
  m_apply->setObjectName("flashApplyPlayback");
  m_refresh = new QPushButton(tr("Refresh Playback"), this);
  m_refresh->setObjectName("flashRefreshPlayback");
  m_refresh->setToolTip(
      tr("Regenerate this instance's exposures after the "
         "shared symbol's timeline length changes."));
  m_open = new QPushButton(tr("Edit Shared Symbol"), this);
  m_open->setObjectName("flashOpenSymbol");
  m_open->setToolTip(
      tr("Open the symbol as a sub-xsheet. Artwork changes "
         "affect every instance that uses this symbol."));

  auto *layout = new QVBoxLayout(this);
  layout->setContentsMargins(10, 10, 10, 10);
  auto *form = new QFormLayout;
  form->addRow(tr("Symbol:"), m_symbol);
  form->addRow(tr("Exposure:"), m_span);
  form->addRow(tr("Playback:"), m_playback);
  form->addRow(m_frameLabel, m_firstFrame);
  form->addRow(tr("Last frame:"), m_lastFrame);
  layout->addLayout(form);
  layout->addWidget(m_status);
  layout->addWidget(m_pivot);
  auto *buttons = new QHBoxLayout;
  buttons->addWidget(m_apply);
  buttons->addWidget(m_refresh);
  layout->addLayout(buttons);
  layout->addWidget(m_open);
  layout->addStretch();

  connect(m_playback, QOverload<int>::of(&QComboBox::currentIndexChanged), this,
          [this] { updateDraft(); });
  connect(m_firstFrame, QOverload<int>::of(&QSpinBox::valueChanged), this,
          [this] { updateDraft(); });
  connect(m_lastFrame, QOverload<int>::of(&QSpinBox::valueChanged), this,
          [this] { updateDraft(); });
  connect(m_apply, &QPushButton::clicked, this, [this] { apply(); });
  connect(m_refresh, &QPushButton::clicked, this, [this] { refresh(); });
  connect(m_open, &QPushButton::clicked, this,
          &FlashInstanceControls::openSymbolRequested);
  connect(TUndoManager::manager(), &TUndoManager::historyChanged, this,
          &FlashInstanceControls::updateControls);
  updateControls();
}

void FlashInstanceControls::setContext(TXsheet *xsheet, int column, int row,
                                       FlashInstanceEdit::Notify notify) {
  m_xsheet = xsheet;
  m_column = column;
  m_row    = row;
  m_notify = std::move(notify);
  updateControls();
}

TXshLevelColumn *FlashInstanceControls::column() const {
  if (!m_xsheet || m_column < 0 || m_column >= m_xsheet->getColumnCount())
    return nullptr;
  auto *native = m_xsheet->getColumn(m_column);
  return native ? native->getLevelColumn() : nullptr;
}

void FlashInstanceControls::updateControls() {
  const QSignalBlocker playbackBlock(m_playback), frameBlock(m_firstFrame),
      lastBlock(m_lastFrame);
  auto *native       = column();
  const int index    = native ? native->flashInstanceIndexAt(m_row) : -1;
  const bool present = index >= 0;
  m_sourceId.clear();
  m_playback->setEnabled(false);
  m_firstFrame->setEnabled(false);
  m_lastFrame->setEnabled(false);
  m_apply->setEnabled(false);
  m_refresh->setEnabled(false);
  m_open->setEnabled(false);
  m_pivot->clear();
  if (!present) {
    m_symbol->setText(tr("No Flash instance selected"));
    m_symbol->setToolTip(QString());
    m_span->clear();
    m_status->setText(
        tr("Select an imported graphic symbol instance in the "
           "Xsheet or Timeline."));
    m_playback->setCurrentIndex(0);
    m_firstFrame->setRange(1, 1);
    m_firstFrame->setValue(1);
    m_lastFrame->setRange(0, 0);
    m_lastFrame->setValue(0);
    m_frameLabel->setText(tr("First frame:"));
    return;
  }
  const auto &instance = native->flashInstances()[index];
  m_sourceId           = instance.sourceId;
  const int count      = instance.level->getChildLevel()->getFrameCount();
  const bool editable  = native->canEditFlashInstance(index);
  m_symbol->setText(QString::fromStdWString(instance.level->getName()));
  m_symbol->setToolTip(instance.libraryName);
  m_span->setText(tr("Frames %1 to %2 (%3 symbol frames)")
                      .arg(instance.timing.start + 1)
                      .arg(instance.timing.start + instance.timing.duration)
                      .arg(count));
  m_playback->setCurrentIndex(m_playback->findData(int(instance.timing.mode)));
  m_firstFrame->setRange(1, std::max(1, count));
  m_firstFrame->setValue(instance.timing.firstFrame + 1);
  m_lastFrame->setRange(0, std::max(1, count));
  m_lastFrame->setValue(
      instance.timing.lastFrame ? *instance.timing.lastFrame + 1 : 0);
  m_playback->setEnabled(editable);
  m_firstFrame->setEnabled(editable);
  m_lastFrame->setEnabled(editable);
  m_refresh->setEnabled(native->canRefreshFlashInstance(index));
  m_open->setEnabled(native->getCell(m_row).m_level == instance.level);
  if (native->isLocked())
    m_status->setText(
        tr("The column is locked. Unlock it to change playback."));
  else if (count <= 0)
    m_status->setText(
        tr("The shared symbol's timeline is empty. Add frames "
           "before changing playback."));
  else if (native->isFlashInstanceLinked(index))
    m_status->setText(tr("Playback is linked to the shared symbol."));
  else if (editable && count <= instance.timing.firstFrame)
    m_status->setText(
        tr("The original first frame no longer exists. Choose "
           "a valid frame and apply playback."));
  else if (editable)
    m_status->setText(
        tr("The symbol's timeline length changed. Refresh "
           "playback or apply new settings."));
  else
    m_status->setText(
        tr("Native exposures no longer match the imported rule. "
           "Restore those exposures to use playback controls."));
  if (instance.hasSourcePivot)
    m_pivot->setText(tr("Source pivot (pixels): %1, %2")
                         .arg(instance.sourcePivot.x)
                         .arg(instance.sourcePivot.y));
  updateDraft();
}

void FlashInstanceControls::updateDraft() {
  const auto mode = Flash::PlaybackMode(m_playback->currentData().toInt());
  m_frameLabel->setText(mode == Flash::PlaybackMode::SingleFrame
                            ? tr("Pose frame:")
                            : tr("First frame:"));
  auto *native    = column();
  const int index = native ? native->flashInstanceIndexAt(m_row) : -1;
  bool changed    = false;
  if (index >= 0 && native->canEditFlashInstance(index)) {
    const auto &instance = native->flashInstances()[index];
    changed              = instance.timing.mode != mode ||
              instance.timing.firstFrame != m_firstFrame->value() - 1 ||
              instance.timing.lastFrame != selectedLastFrame() ||
              native->canRefreshFlashInstance(index);
  }
  m_apply->setEnabled(changed);
}

std::optional<int> FlashInstanceControls::selectedLastFrame() const {
  if (m_lastFrame->value() <= 0) return std::nullopt;
  return m_lastFrame->value() - 1;
}

void FlashInstanceControls::commit(std::unique_ptr<FlashInstanceEdit> edit) {
  if (edit) TUndoManager::manager()->add(edit.release());
  updateControls();
}

void FlashInstanceControls::apply() {
  auto *native    = column();
  const int index = native ? native->flashInstanceIndexAt(m_row) : -1;
  if (index < 0 || native->flashInstances()[index].sourceId != m_sourceId) {
    updateControls();
    return;
  }
  auto timing       = native->flashInstances()[index].timing;
  timing.mode       = Flash::PlaybackMode(m_playback->currentData().toInt());
  timing.firstFrame = m_firstFrame->value() - 1;
  timing.lastFrame  = selectedLastFrame();
  try {
    commit(FlashInstanceEdit::changeTiming(m_xsheet.getPointer(), m_column,
                                           m_row, timing, m_notify));
  } catch (const TException &error) {
    updateControls();
    m_status->setText(QString::fromStdWString(error.getMessage()));
  } catch (const std::exception &error) {
    updateControls();
    m_status->setText(QString::fromUtf8(error.what()));
  }
}

void FlashInstanceControls::refresh() {
  try {
    commit(FlashInstanceEdit::refresh(m_xsheet.getPointer(), m_column, m_row,
                                      m_notify));
  } catch (const TException &error) {
    updateControls();
    m_status->setText(QString::fromStdWString(error.getMessage()));
  } catch (const std::exception &error) {
    updateControls();
    m_status->setText(QString::fromUtf8(error.what()));
  }
}
