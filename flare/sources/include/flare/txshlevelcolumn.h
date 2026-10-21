#pragma once

#ifndef TXSHLEVELCOLUMN_INCLUDED
#define TXSHLEVELCOLUMN_INCLUDED

#include "flare/txshcolumn.h"
#include "flare/flashinstance.h"

#undef DVAPI
#undef DVVAR
#ifdef FLARELIB_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

// Forward declarations
class TLevelColumnFx;
class TXshCell;
class TXshLevel;
class FlashInstanceEdit;

//=============================================================================
//! The TXshLevelColumn class provides a column of levels in xsheet and allows
//! its management.
/*!
Inherits \b TXshCellColumn.

The class defines column of levels getLevelColumn(), more than \b TXshCellColumn
has a pointer to \b TLevelColumnFx getLevelColumnFx().

Note: Previously had an icon identification string for level icon management,
but it has been removed in the current implementation.
*/
//=============================================================================

class DVAPI TXshLevelColumn final : public TXshCellColumn {
  PERSIST_DECLARATION(TXshLevelColumn)

  TLevelColumnFx *m_fx;
  bool m_iconVisible;
  std::vector<FlashInstanceData> m_flashInstances;
  std::vector<FlashPlacementData> m_flashPlacements;
  quint64 m_flashRevision = 0;
  struct FlashCheck {
    quint64 revision         = ~quint64(0);
    int frameCount           = -1;
    bool linked              = false;
    quint64 compiledRevision = ~quint64(0);
    int compiledFrameCount   = -1;
    bool compiledMatch       = false;
  };
  mutable std::vector<FlashCheck> m_flashChecks;

public:
  // Icon visibility management
  bool isIconVisible() const { return m_iconVisible; }
  void setIconVisible(bool visible) { m_iconVisible = visible; }

  // Construction/Destruction
  TXshLevelColumn();
  ~TXshLevelColumn() override;

  // Type information
  TXshColumn::ColumnType getColumnType() const override;
  // A fully deleted imported span can leave a detached source tombstone. Keep
  // that column in the native scene even when it has no visible cells.
  bool isEmpty() const override;

  // Cell validation
  bool canSetCell(const TXshCell &cell) const override;

  // Retained source metadata is not proof of active Flash semantics. Use
  // isFlashInstanceLinked before enabling rule-based controls in the UI.
  const std::vector<FlashInstanceData> &flashInstances() const {
    return m_flashInstances;
  }
  void addFlashInstance(const FlashInstanceData &instance);
  // Metadata-only restoration for native cell undo commands. Cells continue
  // to determine whether the restored rules can be edited.
  void replaceFlashInstances(std::vector<FlashInstanceData> instances);
  const std::vector<FlashPlacementData> &flashPlacements() const {
    return m_flashPlacements;
  }
  // Captures the existing native exposure span and returns a fresh identity.
  QString addFlashPlacement(const FlashPlacementData &placement);
  // Snapshot restoration for native cell-edit undo; detached records remain
  // traceable but do not verify as linked until their cells are restored.
  void replaceFlashPlacements(std::vector<FlashPlacementData> placements);
  bool isFlashPlacementLinked(int index,
                              quint64 maxCellsToCheck = 2000000) const;
  int flashInstanceIndexAt(int row) const;
  bool isFlashInstanceLinked(int index) const;
  bool canEditFlashInstance(int index) const;
  bool canRefreshFlashInstance(int index) const;
  bool setFlashInstanceTiming(int index, const Flash::GraphicTiming &timing);
  bool refreshFlashInstance(int index);

  // Existing exposure edits remain native edits. Validation notices changes,
  // and restoring their original cells (including undo) restores linkage.
  bool setCell(int row, const TXshCell &cell) override;
  bool setCells(int row, int rowCount, const TXshCell cells[]) override;
  void insertEmptyCells(int row, int rowCount = 1) override;
  void removeCells(int row, int rowCount = 1) override;
  void clearCells(int row, int rowCount = 1) override;

  // Column type casting
  TXshLevelColumn *getLevelColumn() override { return this; }

  // Cloning
  TXshColumn *clone() const override;

  // Serialization
  void loadData(TIStream &is) override;
  void saveData(TOStream &os) override;

  // Effects access
  TLevelColumnFx *getLevelColumnFx() const;
  TFx *getFx() const override;

  // Cell operations
  // Used in TCellData::getNumbers
  // reservedLevel can be nonzero if the preferences option
  // "LinkColumnNameWithLevel" is ON and the column name is the same as some
  // level in the scene cast.
  bool setNumbers(int row, int rowCount, const TXshCell cells[],
                  TXshLevel *reservedLevel = nullptr);

  // Disable copying and moving
  TXshLevelColumn(const TXshLevelColumn &)            = delete;
  TXshLevelColumn &operator=(const TXshLevelColumn &) = delete;

  TXshLevelColumn(TXshLevelColumn &&)            = delete;
  TXshLevelColumn &operator=(TXshLevelColumn &&) = delete;

private:
  friend class FlashInstanceEdit;
  void flashCellsChanged();
  bool flashCellsMatch(const FlashInstanceData &instance,
                       int frameCount = -1) const;
  bool materializeFlashInstance(int index, const Flash::GraphicTiming &timing,
                                int symbolFrameCount, bool honorLock = true);
};

#ifdef _WIN32
template class DV_EXPORT_API TSmartPointerT<TXshLevelColumn>;
#endif

// Smart pointer typedef for TXshLevelColumn
typedef TSmartPointerT<TXshLevelColumn> TXshLevelColumnP;

#endif  // TXSHLEVELCOLUMN_INCLUDED

