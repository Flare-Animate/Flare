#pragma once

#ifndef BRUSHPRESETBRIDGE_H
#define BRUSHPRESETBRIDGE_H

#include <QString>
#include <TSmartPointer.h>
#include "tpalette.h"
#include "tcolorstyles.h"
#include "tundo.h"

class TPaletteHandle;

namespace BrushPresetBridge {

// Preset mode flags. Default to the "destructive overwrite" path (the
// pre-existing behaviour) when no external controller is driving them.
inline bool isSelectivePresetMode() { return false; }
inline bool isNonDestructiveMode() { return false; }

// Returns the palette index of the style that matches newStyle's tag id and
// parameters, or -1 when there is no match.
inline int findMatchingStyleInPalette(TPalette *palette, TColorStyle *newStyle) {
  if (!palette || !newStyle) return -1;
  for (int i = 0; i < palette->getStyleCount(); ++i) {
    TColorStyle *s = palette->getStyle(i);
    if (!s) continue;
    if (s->getTagId() == newStyle->getTagId() &&
        *s == *newStyle)
      return i;
  }
  return -1;
}

// Undo object that restores the previous style on undo and re-applies the new
// one on redo.
class StyleOverwriteUndo final : public TUndo {
  TPaletteP m_palette;
  TPaletteHandle *m_handle;
  int m_index;
  TColorStyleP m_old;
  TColorStyleP m_new;

public:
  StyleOverwriteUndo(TPalette *palette, int index, TColorStyle *oldStyle,
                     TColorStyle *newStyle, TPaletteHandle *handle)
    : m_palette(palette)
    , m_handle(handle)
    , m_index(index)
    , m_old(oldStyle)
    , m_new(newStyle) {}

  int getSize() const override { return sizeof(*this); }
  void undo() const override {
    if (m_palette) m_palette->setStyle(m_index, m_old->clone());
  }
  void redo() const override {
    if (m_palette) m_palette->setStyle(m_index, m_new->clone());
  }
};

inline TUndo *createStyleOverwriteUndo(TPalette *palette, int index,
                                       TColorStyle *oldStyle,
                                       TColorStyle *newStyle,
                                       TPaletteHandle *handle) {
  return new StyleOverwriteUndo(palette, index, oldStyle, newStyle, handle);
}

}  // namespace BrushPresetBridge

#endif
