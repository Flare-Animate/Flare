#pragma once

#include "flare/flashinstance.h"
#include "flare/txsheet.h"
#include "tundo.h"
#include <functional>
#include <memory>

#undef DVAPI
#ifdef TOONZLIB_EXPORTS
#define DVAPI DV_EXPORT_API
#else
#define DVAPI DV_IMPORT_API
#endif

// A control edit has already been applied when these factories return an undo.
// The caller must add it to TUndoManager. Notifications run on add, undo and
// redo. Capturing both compiled frame counts makes undo independent of later
// edits to the shared symbol, without storing a second copy of every native
// cell.
class DVAPI FlashInstanceEdit final : public TUndo {
public:
  using Notify = std::function<void()>;

  static std::unique_ptr<FlashInstanceEdit> changeTiming(
      TXsheet *xsheet, int column, int row, const Flash::GraphicTiming &timing,
      Notify notify = {});
  static std::unique_ptr<FlashInstanceEdit> refresh(TXsheet *xsheet, int column,
                                                    int row,
                                                    Notify notify = {});

  void undo() const override;
  void redo() const override;
  void onAdd() override;
  int getSize() const override;
  QString getHistoryString() override;
  int getHistoryType() override;

private:
  TXsheetP m_xsheet;
  int m_column;
  FlashInstanceData m_before, m_after;
  Notify m_notify;
  bool m_refresh = false;

  FlashInstanceEdit(TXsheet *xsheet, int column,
                    const FlashInstanceData &before, Notify notify);
  void restore(const FlashInstanceData &state) const;
  void changed() const;
};

// Supplemental metadata snapshots for existing native insert/delete/cut undo
// commands. The native command owns cell editing, selection and notifications.
// Construct before editing cells, then call redo/undo after the cell operation.
// Captured spans make complete deletions and prefix trims exactly reversible.
class DVAPI FlashInstanceTimelineEdit final {
public:
  FlashInstanceTimelineEdit(TXsheet *xsheet, int firstColumn, int lastColumn,
                            int row, int rowCount, bool insertion);
  void undo(TXsheet *xsheet) const;
  void redo(TXsheet *xsheet) const;
  int getSize() const;

private:
  struct Column {
    int index;
    std::vector<FlashInstanceData> before, after;
    std::vector<FlashPlacementData> placementsBefore, placementsAfter;
  };
  struct LabelPosition {
    QString id;
    int before, after;
    bool beforeDetached, afterDetached;
  };
  TXsheetP m_xsheet;
  std::vector<Column> m_columns;
  std::vector<LabelPosition> m_labelPositions;
  void restore(TXsheet *xsheet, bool before) const;
};
