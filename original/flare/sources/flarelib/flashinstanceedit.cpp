#include "flare/flashinstanceedit.h"
#include "flare/txshlevelcolumn.h"
#include "historytypes.h"
#include <utility>
#include <algorithm>
#include <limits>
#include <QSet>

namespace {
TXshLevelColumn *levelColumn(TXsheet *xsheet, int column) {
  if (!xsheet || column < 0 || column >= xsheet->getColumnCount())
    return nullptr;
  auto *native = xsheet->getColumn(column);
  return native ? native->getLevelColumn() : nullptr;
}

bool sameTiming(const Flash::GraphicTiming &a, const Flash::GraphicTiming &b) {
  return a.start == b.start && a.duration == b.duration &&
         a.firstFrame == b.firstFrame && a.mode == b.mode &&
         a.lastFrame == b.lastFrame;
}
}  // namespace

FlashInstanceEdit::FlashInstanceEdit(TXsheet *xsheet, int column,
                                     const FlashInstanceData &before,
                                     Notify notify)
    : m_xsheet(xsheet)
    , m_column(column)
    , m_before(before)
    , m_after(before)
    , m_notify(std::move(notify)) {}

std::unique_ptr<FlashInstanceEdit> FlashInstanceEdit::changeTiming(
    TXsheet *xsheet, int column, int row, const Flash::GraphicTiming &timing,
    Notify notify) {
  auto *native = levelColumn(xsheet, column);
  if (!native) return {};
  const int index = native->flashInstanceIndexAt(row);
  if (!native->canEditFlashInstance(index)) return {};
  const auto &before = native->flashInstances()[index];
  std::unique_ptr<FlashInstanceEdit> edit(
      new FlashInstanceEdit(xsheet, column, before, std::move(notify)));
  if (!native->setFlashInstanceTiming(index, timing)) return {};
  edit->m_after = native->flashInstances()[index];
  if (sameTiming(edit->m_before.timing, edit->m_after.timing) &&
      edit->m_before.symbolFrameCount == edit->m_after.symbolFrameCount)
    return {};
  xsheet->updateFrameCount();
  return edit;
}

std::unique_ptr<FlashInstanceEdit> FlashInstanceEdit::refresh(TXsheet *xsheet,
                                                              int column,
                                                              int row,
                                                              Notify notify) {
  auto *native = levelColumn(xsheet, column);
  if (!native) return {};
  const int index = native->flashInstanceIndexAt(row);
  if (!native->canRefreshFlashInstance(index)) return {};
  std::unique_ptr<FlashInstanceEdit> edit(new FlashInstanceEdit(
      xsheet, column, native->flashInstances()[index], std::move(notify)));
  if (!native->refreshFlashInstance(index)) return {};
  edit->m_after   = native->flashInstances()[index];
  edit->m_refresh = true;
  xsheet->updateFrameCount();
  return edit;
}

void FlashInstanceEdit::changed() const {
  if (m_notify) m_notify();
}

void FlashInstanceEdit::restore(const FlashInstanceData &state) const {
  auto *native = levelColumn(m_xsheet.getPointer(), m_column);
  if (!native) return;
  const int index = native->flashInstanceIndexAt(state.timing.start);
  if (index < 0) return;
  const auto &current = native->flashInstances()[index];
  // Undo order restores column positions and exposure spans. Refuse to apply
  // a stale command to an unrelated column if that invariant is violated.
  if (current.sourceId != state.sourceId || current.level != state.level ||
      current.timing.start != state.timing.start ||
      current.timing.duration != state.timing.duration)
    return;
  // Undo remains available if a column was subsequently locked. Use the
  // captured count, not the current symbol length, to recover the exact cells.
  if (native->materializeFlashInstance(index, state.timing,
                                       state.symbolFrameCount, false)) {
    m_xsheet->updateFrameCount();
    changed();
  }
}

void FlashInstanceEdit::undo() const { restore(m_before); }
void FlashInstanceEdit::redo() const { restore(m_after); }
void FlashInstanceEdit::onAdd() { changed(); }

int FlashInstanceEdit::getSize() const {
  return int(sizeof(*this) +
             2 * (m_before.sourceId.size() + m_before.libraryName.size() +
                  m_after.sourceId.size() + m_after.libraryName.size()));
}

QString FlashInstanceEdit::getHistoryString() {
  return (m_refresh ? QObject::tr("Refresh Flash Instance : Col%1")
                    : QObject::tr("Flash Instance Playback : Col%1"))
      .arg(m_column + 1);
}

int FlashInstanceEdit::getHistoryType() { return HistoryType::Xsheet; }

FlashInstanceTimelineEdit::FlashInstanceTimelineEdit(TXsheet *xsheet,
                                                     int firstColumn,
                                                     int lastColumn, int row,
                                                     int rowCount,
                                                     bool insertion) {
  // This snapshot supplements an ordinary native cell command and must never
  // block it. Columns without Flash metadata make it a no-op. Metadata that
  // cannot follow the edit keeps its old timing, so verification reports it
  // as unlinked exactly like after a manual exposure edit.
  if (!xsheet || row < 0 || rowCount <= 0) return;
  const int first = std::max(0, firstColumn);
  const int last  = std::min(lastColumn, xsheet->getColumnCount() - 1);
  if (first > last) return;
  bool hasMetadata = false;
  for (const auto &label : xsheet->flashFrameLabels())
    if (!label.detached && label.nativeColumn >= first &&
        label.nativeColumn <= last) {
      hasMetadata = true;
      break;
    }
  for (int col = first; !hasMetadata && col <= last; ++col) {
    auto *native = levelColumn(xsheet, col);
    hasMetadata  = native && (!native->flashInstances().empty() ||
                             !native->flashPlacements().empty());
  }
  if (!hasMetadata) return;
  m_xsheet                = xsheet;
  const qint64 removedEnd = qint64(row) + rowCount;
  for (const auto &label : xsheet->flashFrameLabels()) {
    if (label.detached || label.nativeColumn < first ||
        label.nativeColumn > last || label.row < row)
      continue;
    auto *column = xsheet->getColumn(label.nativeColumn);
    if (!column || column->isLocked()) continue;
    int after          = label.row;
    bool afterDetached = false;
    if (insertion) {
      // A label pushed beyond the supported row range becomes a hidden
      // tombstone; undo restores its exact position.
      if (qint64(label.row) + rowCount > 10000000)
        afterDetached = true;
      else
        after += rowCount;
    } else if (qint64(label.row) >= removedEnd) {
      after -= rowCount;
    } else {
      // The authored keyframe was deleted. Keep its source lineage as a
      // hidden tombstone; undo restores the exact original position.
      afterDetached = true;
    }
    m_labelPositions.push_back(
        {label.nativeLabelId, label.row, after, false, afterDetached});
  }
  if (!m_labelPositions.empty()) {
    std::vector<FlashFrameLabelPosition> shifted;
    shifted.reserve(m_labelPositions.size());
    for (const auto &label : m_labelPositions)
      shifted.push_back({label.id, label.after, label.afterDetached});
    try {
      xsheet->validateFlashFrameLabelPositions(shifted);
    } catch (...) {
      // Labels that cannot follow the edit stay where they are.
      m_labelPositions.clear();
    }
  }
  for (int col = first; col <= last; ++col) {
    auto *native = levelColumn(xsheet, col);
    if (!native || native->isEmpty() || native->isLocked() ||
        (native->flashInstances().empty() && native->flashPlacements().empty()))
      continue;
    Column column;
    column.index            = col;
    column.before           = native->flashInstances();
    column.placementsBefore = native->flashPlacements();
    for (const auto &record : column.before) {
      auto instance       = record;
      const auto original = instance.timing;
      const qint64 end    = qint64(original.start) + original.duration;
      bool shifted        = true;
      if (insertion) {
        qint64 start = original.start, duration = original.duration;
        if (row <= original.start)
          start += rowCount;
        else if (row < end)
          duration += rowCount;
        if (start > std::numeric_limits<int>::max() ||
            duration > std::numeric_limits<int>::max()) {
          shifted = false;
        } else {
          instance.timing.start    = int(start);
          instance.timing.duration = int(duration);
        }
      } else if (row < end) {
        if (removedEnd <= original.start) {
          instance.timing.start -= rowCount;
        } else {
          const int overlap = int(std::min<qint64>(end, removedEnd) -
                                  std::max<qint64>(original.start, row));
          if (overlap == original.duration) continue;
          instance.timing.duration -= overlap;
          if (row <= original.start) {
            const auto firstFrame = original.sourceFrameAt(
                int(removedEnd), instance.symbolFrameCount);
            if (firstFrame) {
              instance.timing.start      = row;
              instance.timing.firstFrame = *firstFrame;
            } else
              shifted = false;
          }
        }
      }
      if (shifted) {
        try {
          instance.toJson();
        } catch (...) {
          shifted = false;
        }
      }
      // An unshiftable rule keeps its old timing: its cells no longer match,
      // so the column reports it as unlinked instead of failing the edit.
      column.after.push_back(shifted ? std::move(instance) : record);
    }
    // Generic occurrence IDs have no playback rule to regenerate altered
    // exposures. Keep their source lineage after intersecting edits, but mark
    // them detached so the cells are never treated as original art.
    for (auto placement : column.placementsBefore) {
      const qint64 end = qint64(placement.start) + placement.duration;
      if (insertion) {
        if (row <= placement.start) {
          if (qint64(placement.start) + rowCount >
              std::numeric_limits<int>::max())
            placement.detached = true;
          else
            placement.start += rowCount;
        } else if (row < end) {
          placement.detached = true;
        }
      } else if (row < end) {
        if (removedEnd <= placement.start)
          placement.start -= rowCount;
        else
          placement.detached = true;
      }
      column.placementsAfter.push_back(std::move(placement));
    }
    std::stable_sort(
        column.placementsAfter.begin(), column.placementsAfter.end(),
        [](const FlashPlacementData &a, const FlashPlacementData &b) {
          return a.start < b.start;
        });
    m_columns.push_back(std::move(column));
  }
}

void FlashInstanceTimelineEdit::restore(TXsheet *xsheet, bool before) const {
  if (!xsheet || xsheet != m_xsheet.getPointer()) return;
  // Undo/redo must never throw. A label removed since this snapshot was taken
  // is skipped instead of failing the whole native command.
  QSet<QString> present;
  for (const auto &label : xsheet->flashFrameLabels())
    present.insert(label.nativeLabelId);
  std::vector<FlashFrameLabelPosition> labelPositions;
  labelPositions.reserve(m_labelPositions.size());
  for (const auto &label : m_labelPositions)
    if (present.contains(label.id))
      labelPositions.push_back(
          {label.id, before ? label.before : label.after,
           before ? label.beforeDetached : label.afterDetached});
  try {
    xsheet->setFlashFrameLabelPositions(labelPositions);
  } catch (...) {
  }
  for (const auto &column : m_columns) {
    auto *native = levelColumn(xsheet, column.index);
    // Follow the native cell command's locking and column-type policy.
    if (!native || native->isLocked()) continue;
    try {
      native->replaceFlashInstances(before ? column.before : column.after);
    } catch (...) {
    }
    try {
      native->replaceFlashPlacements(before ? column.placementsBefore
                                            : column.placementsAfter);
    } catch (...) {
    }
  }
}

void FlashInstanceTimelineEdit::undo(TXsheet *xsheet) const {
  restore(xsheet, true);
}

void FlashInstanceTimelineEdit::redo(TXsheet *xsheet) const {
  restore(xsheet, false);
}

int FlashInstanceTimelineEdit::getSize() const {
  size_t bytes = sizeof(*this) + m_columns.size() * sizeof(Column);
  for (const auto &label : m_labelPositions)
    bytes += sizeof(LabelPosition) + size_t(label.id.size()) * 2;
  for (const auto &column : m_columns)
    for (const auto *records : {&column.before, &column.after}) {
      bytes += records->size() * sizeof(FlashInstanceData);
      for (const auto &record : *records)
        bytes += size_t(record.sourceId.size() + record.libraryName.size()) * 2;
    }
  for (const auto &column : m_columns)
    for (const auto *records :
         {&column.placementsBefore, &column.placementsAfter}) {
      bytes += records->size() * sizeof(FlashPlacementData);
      for (const auto &record : *records)
        bytes += size_t(record.sourceId.size() + record.kind.size() +
                        record.nativePlacementId.size() +
                        record.framesSha256.size()) *
                 2;
    }
  return int(std::min<size_t>(bytes, std::numeric_limits<int>::max()));
}
