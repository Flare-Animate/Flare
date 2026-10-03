// SWFShape -- the vector geometry of DefineShape, DefineShape2, DefineShape3 and
// DefineShape4.
//
// This is the SWF counterpart to XFLShape, which reads the same outlines from an
// .fla's <DOMShape edges="...">. An SWF stores them as a bit-packed stream of
// shape records instead of text, so the two decoders share nothing but the output
// type: contours in twips, which toSvgPath() turns into SVG at 1/20 px per twip.
//
// The record grammar, from the SWF specification and cross-checked against
// Ruffle's reader (swf/src/read.rs, read_shape_record):
//
//   bit 0 set            -> an edge record
//   bit 0 clear, bits 4-0 zero -> end of shape
//   bit 0 clear, bits 4-0 set  -> a style change
//
//   EdgeRecord:
//     StraightFlag = 0: a straight segment. NumBits(4)+2, then GeneralDeltaFlag.
//       GeneralDelta: DeltaX(SNumBits), DeltaY(SNumBits).
//       Axis-aligned: VerticalFlag. A vertical edge stores only X; a horizontal
//       edge only Y. The other component is zero -- not "the same as before",
//       which is the mistake that produces plausible but wrong geometry.
//     StraightFlag = 1: a quadratic segment -- a control point and an anchor,
//       four SNumBits values, both relative to the current point.
//
//   StyleChangeRecord:
//     StateNewStyles(1) StateLineStyle(1) StateFillStyle1(1) StateFillStyle0(1)
//     StateMoveTo(1), then 6 reserved bits.
//     StateMoveTo -> MoveBits(5), MoveDeltaX(SMoveBits), MoveDeltaY(SMoveBits).
//     The move is absolute, not relative to the current point.
//
// The styles ahead of the records decide NumFillBits and NumLineBits, which give
// the width of the fill and line indices -- so they must be read even though this
// decoder does not use the styles, and a malformed style array must still be
// walked or every subsequent record is read at the wrong bit.
//
// Not handled, and stated rather than left to look supported: DefineMorphShape
// (46), whose records are a start/end pair per shape and need both endpoints at
// once; and the shape-with-style variants of DefineShape3/4 beyond the geometry.
// Only the outline is produced. Fills and strokes are indices into the style
// arrays, which this reader does not decode, so a contour here has no paint.

#ifndef TFLASH_SWFSHAPE_H
#define TFLASH_SWFSHAPE_H

#include "tcommon.h"

#include <QPointF>
#include <QString>
#include <QVector>

// The bodies live in tnzcore, which is built with TFLASH_EXPORTS, so the API has
// to be imported from the DLL's side. Without this the functions are internal to
// tnzcore and every caller fails to link -- which is the mistake that left
// As3Bridge's free functions unmarkable rather than exported.
#undef DVAPI
#if defined(TFLASH_EXPORTS)
#define DVAPI DV_EXPORT_API
#else
#define DVAPI DV_IMPORT_API
#endif

namespace SWF {

// One segment of a contour, in twips. `control` is meaningful only when
// `curve` is true, and `end` always is.
struct Segment {
  QPointF control;
  QPointF end;
  bool curve = false;
};

// A contour: a start point and the segments that follow it. Closure is implicit
// -- the format does not store a closing segment, and Flash closes every contour
// it fills.
struct Contour {
  QPointF start;
  QVector<Segment> segments;
};

// Everything one shape tag yielded. `ok` is false when the tag could not be read
// to completion, and `error` then says why -- a caller must not treat a partial
// outline as a whole one.
struct Shape {
  bool ok = false;
  QString error;

  int id = 0;              // CharacterID
  int version = 0;         // 1 = DefineShape, 2 = Shape2, 3 = Shape3, 4 = Shape4
  QPointF boundsMin;       // ShapeBounds, in twips
  QPointF boundsMax;
  QPointF edgeMin;         // EdgeBounds: DefineShape4 carries it separately
  QPointF edgeMax;
  bool hasEdgeBounds = false;

  int numFillStyles = 0;
  int numLineStyles = 0;
  int records = 0;         // shape records consumed, edges and style changes
  QVector<Contour> contours;

  DVAPI int segmentCount() const;
};

// Decode one DefineShape{,2,3,4} body. `data`/`size` are the tag body exactly
// as it appears in the tag stream -- no tag header, and for a CWS file the
// *decompressed* bytes. Returns a Shape with ok=false and an error rather than
// guessing when the stream does not decode; a partially-read outline is worse
// than none, because it draws something wrong rather than nothing.
//
// `version` selects the tag variant: 1 for DefineShape, 2 for DefineShape2,
// 3 for DefineShape3, 4 for DefineShape4.
DVAPI Shape decodeShape(const unsigned char *data, int size, int version);

// The same, taking the bytes as a QByteArray.
DVAPI Shape decodeShape(const QByteArray &data, int version);

// Outline as an SVG path in a viewBox of the shape's own bounds, in twips.
// Empty when the shape did not decode. Coordinates are written with at most
// three decimals -- a twip is 1/20 px, so 1/1000 px is finer than the data can
// express, and rounding at the fourth decimal produces digits that mean nothing.
DVAPI QString toSvgPath(const Shape &shape);

// The outline as SVG, wrapped in an <svg> so a file can be dropped on disk.
DVAPI QString toSvgDocument(const Shape &shape);

}  // namespace SWF

#endif
