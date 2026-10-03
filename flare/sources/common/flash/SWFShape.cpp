#include "SWFShape.h"

#include <QObject>
#include <QStringList>
#include <cmath>

namespace SWF {

namespace {

// A big-endian bit reader, MSB first, which is the order the SWF specification
// states: "the bit order within bytes is big-endian: the most significant bit is
// stored first".
class BitReader {
public:
  BitReader(const unsigned char *d, int size) : m_d(d), m_size(size) {}

  bool eof() const { return m_bit >= m_size * 8; }
  int bitPos() const { return m_bit; }
  int bytePos() const { return m_bit / 8; }
  int bitsLeft() const { return m_size * 8 - m_bit; }

  bool bit(bool &out) {
    if (eof()) return false;
    const int byte = m_bit >> 3;
    const int off = 7 - (m_bit & 7);
    out = ((m_d[byte] >> off) & 1) != 0;
    ++m_bit;
    return true;
  }

  bool bits(int n, int &out) {
    out = 0;
    for (int i = 0; i < n; ++i) {
      bool b;
      if (!bit(b)) return false;
      out = (out << 1) | (b ? 1 : 0);
    }
    return true;
  }

  // A two's-complement signed value of n bits.
  bool sbits(int n, int &out) {
    if (n <= 0) { out = 0; return true; }
    if (n > 32) return false;
    int v;
    if (!bits(n, v)) return false;
    const int signBit = 1 << (n - 1);
    out = (v & signBit) ? (v - (1 << n)) : v;
    return true;
  }

  // An unsigned byte-aligned value, for the parts of the format that are not bit
  // packed: the style arrays and the header fields ahead of the records.
  // Only usable where the format really is byte aligned. Anything past the
  // RECT is bit-packed and must go through bit()/bits(), because reading it a
  // byte at a time is what produced a style array that parsed "successfully"
  // while consuming the wrong bits.
  bool ub(int &out) {
    if (m_bit % 8 != 0) return false;
    const int byte = m_bit >> 3;
    if (byte >= m_size) return false;
    out = m_d[byte];
    m_bit += 8;
    return true;
  }

  // Little-endian, as every integer in the format is: "the least significant
  // byte is stored first". Only the bit order *within* a byte is big-endian, and
  // reading a CharacterID the other way round turns 1 into 256 -- a value that
  // looks entirely reasonable and is wrong.
  bool ub16(int &out) {
    int lo, hi;
    if (!ub(lo) || !ub(hi)) return false;
    out = (hi << 8) | lo;
    return true;
  }

  bool sbits_fixed(int n, double &out) {
    int v;
    if (!sbits(n, v)) return false;
    out = v;
    return true;
  }

  // RECT: Nbits(5) then four signed fields of Nbits, XMin XMax YMin YMax.
  bool rect(QPointF &mn, QPointF &mx) {
    int nbits;
    if (!bits(5, nbits)) return false;
    if (nbits == 0) {                       // an empty rect is legal
      mn = QPointF(0, 0);
      mx = QPointF(0, 0);
      return true;
    }
    if (bitsLeft() < nbits * 4) return false;
    double xmin, xmax, ymin, ymax;
    if (!sbits_fixed(nbits, xmin)) return false;
    if (!sbits_fixed(nbits, xmax)) return false;
    if (!sbits_fixed(nbits, ymin)) return false;
    if (!sbits_fixed(nbits, ymax)) return false;
    mn = QPointF(xmin, ymin);
    mx = QPointF(xmax, ymax);
    return true;
  }

  void alignToByte() {
    if (m_bit % 8) m_bit += 8 - (m_bit % 8);
  }

private:
  const unsigned char *m_d;
  int m_size;
  int m_bit = 0;
};

// Skip one fill style. The geometry decoder needs to walk past these to reach the
// records, and a wrong stride here puts every subsequent record at the wrong bit
// while still producing plausible coordinates -- which is why this follows the
// specification field by field rather than estimating.
// FillStyleArray and LineStyleArray sit inside the bit stream, after a bit-packed
// RECT, so every field here is read through the bit reader. Reading them as
// bytes is the mistake this file made first: the array "parsed", consumed the
// wrong number of bits, and every record after it was read at the wrong offset --
// producing coordinates that look like geometry.
bool skipFillStyle(BitReader &r, int version) {
  int type;
  if (!r.bits(8, type)) return false;
  auto rgb = [&r]() {
    int a, b, c;
    return r.bits(8, a) && r.bits(8, b) && r.bits(8, c);
  };
  auto rgba = [&r]() {
    int a, b, c, d;
    return r.bits(8, a) && r.bits(8, b) && r.bits(8, c) && r.bits(8, d);
  };
  auto matrix = [&r]() {
    bool hasScale;
    if (!r.bit(hasScale)) return false;
    if (hasScale) {
      int n;
      if (!r.bits(5, n)) return false;
      int v;
      if (!r.sbits(n, v)) return false;
    }
    bool hasRotate;
    if (!r.bit(hasRotate)) return false;
    if (hasRotate) {
      int n;
      if (!r.bits(5, n)) return false;
      int v;
      if (!r.sbits(n, v)) return false;
      if (!r.sbits(n, v)) return false;
    }
    int n;
    if (!r.bits(5, n)) return false;
    int v;
    for (int i = 0; i < 2; ++i)
      if (!r.sbits(n, v)) return false;
    return true;
  };
  auto gradient = [&r, &rgb, &rgba, version](bool focal) {
    int spread;
    if (!r.bits(8, spread)) return false;
    if (version >= 4 && focal) {
      int f;
      if (!r.sbits(4, f)) return false;
    }
    int nInterp;
    if (!r.bits(2, nInterp)) return false;
    if (version < 4) {
      int n;
      if (!r.bits(4, n)) return false;
      for (int i = 0; i < n; ++i) {
        int c;
        if (!r.sbits(8, c)) return false;     // focal point delta, always 0 for
        if (!r.sbits(8, c)) return false;      // linear; read two to advance
      }
    } else {
      int n, ratio;
      if (!r.bits(4, n)) return false;
      for (int i = 0; i < n; ++i)
        if (!r.bits(8, ratio)) return false;   // StartRatio, EndRatio
    }
    int n, v;
    if (!r.bits(4, n)) return false;
    for (int i = 0; i < n; ++i)
      if (!r.sbits(8, v)) return false;       // GradientRecord
    return true;
  };

  switch (type) {
    case 0x00: return version >= 3 ? rgba() : rgb();
    case 0x10: return gradient(false);
    case 0x12: return gradient(false);
    case 0x13: return gradient(true);
    case 0x20:
    case 0x21:
    case 0x22:
    case 0x23:
      if (!rgb()) return false;
      return matrix();
    case 0x40:
    case 0x41:
    case 0x42:
    case 0x43:
      if (version >= 3) {
        if (!rgba()) return false;
      } else {
        if (!rgb()) return false;
      }
      return matrix();
    case 0x06:
      return rgb() && matrix();
    default:
      return false;
  }
}

// Skip one line style. DefineShape2 and later add a 16-bit width in twips.
bool skipLineStyle(BitReader &r, int version) {
  int width;
  if (!r.bits(8, width)) return false;
  if (version >= 2) {
    int lo, hi;
    if (!r.bits(8, lo) || !r.bits(8, hi)) return false;   // Width, UI16
  }
  int rr, gg, bb;
  if (!r.bits(8, rr) || !r.bits(8, gg) || !r.bits(8, bb)) return false;
  if (version >= 3) {
    int a;
    if (!r.bits(8, a)) return false;
  }
  bool hasScale;
  if (!r.bit(hasScale)) return false;
  if (hasScale) {
    int n, v;
    if (!r.bits(5, n)) return false;
    if (!r.sbits(n, v)) return false;
  }
  bool hasRotate;
  if (!r.bit(hasRotate)) return false;
  if (hasRotate) {
    int n, v;
    if (!r.bits(5, n)) return false;
    if (!r.sbits(n, v)) return false;
    if (!r.sbits(n, v)) return false;
  }
  int n, v;
  if (!r.bits(5, n)) return false;
  for (int i = 0; i < 2; ++i)
    if (!r.sbits(n, v)) return false;
  return true;
}

// The style arrays, and the two bit widths the records use. Read even though the
// geometry does not need the styles: they are ahead of the records, and skipping
// them by estimate would shift every record.
bool readStyles(BitReader &r, int version, int &numFill, int &numLine,
                int &numFillBits, int &numLineBits) {
  int n;
  if (!r.bits(8, n)) return false;
  if (n == 0xFF && version >= 2) {
    int hi, lo;
    if (!r.bits(8, lo) || !r.bits(8, hi)) return false;   // UI16, little-endian
    n = (hi << 8) | lo;
  }
  numFill = n;
  for (int i = 0; i < numFill; ++i)
    if (!skipFillStyle(r, version)) return false;

  if (!r.bits(8, n)) return false;
  if (n == 0xFF && version >= 2) {
    int hi, lo;
    if (!r.bits(8, lo) || !r.bits(8, hi)) return false;
    n = (hi << 8) | lo;
  }
  numLine = n;
  for (int i = 0; i < numLine; ++i)
    if (!skipLineStyle(r, version)) return false;

  // One byte holding both widths: the high nibble the fill bits, the low nibble
  // the line bits.
  int both;
  if (!r.bits(8, both)) return false;
  numFillBits = both >> 4;
  numLineBits = both & 0x0F;
  return true;
}

// Format a twip coordinate: integral values plainly, fractions to at most three
// decimals with no trailing zeros.
QString num(double v) {
  const double r = std::round(v * 1000.0) / 1000.0;
  if (std::fabs(r - std::round(r)) < 1e-9)
    return QString::number(static_cast<qlonglong>(std::round(r)));
  return QString::number(r, 'f', 3);
}

}  // namespace

int Shape::segmentCount() const {
  int n = 0;
  for (const Contour &c : contours) n += c.segments.size();
  return n;
}

Shape decodeShape(const unsigned char *data, int size, int version) {
  Shape out;
  out.version = version;
  if (!data || size <= 0) {
    out.error = QObject::tr("empty shape tag");
    return out;
  }
  if (version < 1 || version > 4) {
    out.error = QObject::tr("unsupported shape version %1").arg(version);
    return out;
  }

  BitReader r(data, size);
  // Byte aligned: nothing precedes it in the tag. Still read through the bit
  // reader so there is one code path, and so the little-endian order is applied
  // in one place.
  if (!r.ub16(out.id)) {
    out.error = QObject::tr("truncated: no CharacterID");
    return out;
  }
  if (!r.rect(out.boundsMin, out.boundsMax)) {
    out.error = QObject::tr("truncated: no ShapeBounds");
    return out;
  }
  out.edgeMin = out.boundsMin;
  out.edgeMax = out.boundsMax;

  // DefineShape4 carries EdgeBounds separately, and a flag byte after it.
  if (version >= 4) {
    if (!r.rect(out.edgeMin, out.edgeMax)) {
      out.error = QObject::tr("truncated: no EdgeBounds");
      return out;
    }
    out.hasEdgeBounds = true;
    int flags;
    if (!r.bits(8, flags)) {
      out.error = QObject::tr("truncated: no Shape4 flag byte");
      return out;
    }
  }

  int numFillBits = 0, numLineBits = 0;
  if (!readStyles(r, version, out.numFillStyles, out.numLineStyles, numFillBits,
                  numLineBits)) {
    out.error = QObject::tr("truncated in the style arrays");
    return out;
  }

  // ---- the record stream --------------------------------------------------
  //
  // The current point, and the contour being built. A move starts a new one; an
  // edge extends the current one. Closure is implicit -- the format has no closing
  // segment and Flash closes every contour it fills.
  QPointF cur;
  Contour contour;
  bool haveContour = false;

  const auto flush = [&]() {
    if (haveContour && !contour.segments.isEmpty())
      out.contours.push_back(contour);
    contour = Contour();
    haveContour = false;
  };

  while (true) {
    if (r.bitsLeft() < 1) {
      // Ran out mid-stream. What has been read so far is a partial outline, and
      // drawing it would put a shape on screen that is not the one in the file,
      // so it is discarded rather than reported as a decode with a warning.
      out.error = QObject::tr("truncated in the shape records at bit %1")
                      .arg(r.bitPos());
      return out;
    }

    bool isEdge;
    if (!r.bit(isEdge)) {
      out.error = QObject::tr("truncated at bit %1").arg(r.bitPos());
      return out;
    }

    if (isEdge) {
      bool straight;
      if (!r.bit(straight)) {
        out.error = QObject::tr("truncated at bit %1").arg(r.bitPos());
        return out;
      }
      int numBitsRaw;
      if (!r.bits(4, numBitsRaw)) {
        out.error = QObject::tr("truncated at bit %1").arg(r.bitPos());
        return out;
      }
      const int numBits = numBitsRaw + 2;

      Segment seg;
      if (straight) {
        // The bit is GeneralDeltaFlag, and it means the opposite of what the
        // local name suggested: 1 means *general* (both components present), 0
        // means axis-aligned (only one present). Reading it as "axisAligned"
        // without the negation swapped the two, so every ordinary edge was
        // decoded as if it were axis-aligned -- which produced a segment at
        // (-224, 0) where the square wanted (400, 0), and still terminated
        // cleanly at the end-of-shape record.
        bool generalDelta;
        if (!r.bit(generalDelta)) {
          out.error = QObject::tr("truncated at bit %1").arg(r.bitPos());
          return out;
        }
        const bool axisAligned = !generalDelta;
        // Axis-aligned edges store only the component that changes. The other is
        // zero -- *not* "the same as the previous edge". Reading it as anything
        // else yields plausible-looking coordinates, which is how a decoder of
        // this format goes wrong quietly.
        bool vertical = false;
        if (axisAligned) {
          if (!r.bit(vertical)) {
            out.error = QObject::tr("truncated at bit %1").arg(r.bitPos());
            return out;
          }
        }
        int dx = 0, dy = 0;
        if (!axisAligned || !vertical) {
          if (!r.sbits(numBits, dx)) {
            out.error = QObject::tr("truncated at bit %1").arg(r.bitPos());
            return out;
          }
        }
        if (!axisAligned || vertical) {
          if (!r.sbits(numBits, dy)) {
            out.error = QObject::tr("truncated at bit %1").arg(r.bitPos());
            return out;
          }
        }
        seg.curve = false;
        seg.end = cur + QPointF(dx, dy);
      } else {
        // A quadratic segment: a control point and an anchor, both relative to
        // the current point.
        int cx, cy, ax, ay;
        if (!r.sbits(numBits, cx) || !r.sbits(numBits, cy) ||
            !r.sbits(numBits, ax) || !r.sbits(numBits, ay)) {
          out.error = QObject::tr("truncated curve at bit %1").arg(r.bitPos());
          return out;
        }
        seg.curve = true;
        seg.control = cur + QPointF(cx, cy);
        seg.end = cur + QPointF(ax, ay);
      }

      if (!haveContour) {
        // An edge with no preceding move: the contour starts at the current
        // point, which the specification initialises to (0, 0) and which a
        // well-formed file never relies on.
        contour.start = cur;
        haveContour = true;
      }
      cur = seg.end;
      contour.segments.push_back(seg);
      ++out.records;
      continue;
    }

    // A style change record: five state bits, then six reserved.
    int flagsRaw;
    if (!r.bits(5, flagsRaw)) {
      out.error = QObject::tr("truncated at bit %1").arg(r.bitPos());
      return out;
    }
    if (flagsRaw == 0) break;          // all five clear: end of shape

    const bool stateNewStyles = flagsRaw & 0x80;
    const bool stateLineStyle = flagsRaw & 0x40;
    const bool stateFillStyle1 = flagsRaw & 0x20;
    const bool stateFillStyle0 = flagsRaw & 0x10;
    const bool stateMoveTo = flagsRaw & 0x08;
    int reserved;
    if (!r.bits(6, reserved)) {
      out.error = QObject::tr("truncated at bit %1").arg(r.bitPos());
      return out;
    }

    if (stateMoveTo) {
      int moveBits;
      if (!r.bits(5, moveBits)) {
        out.error = QObject::tr("truncated at bit %1").arg(r.bitPos());
        return out;
      }
      int mx = 0, my = 0;
      if (moveBits > 0) {
        if (!r.sbits(moveBits, mx) || !r.sbits(moveBits, my)) {
          out.error = QObject::tr("truncated move at bit %1").arg(r.bitPos());
          return out;
        }
      }
      // A move is absolute, not relative to the current point.
      flush();
      cur = QPointF(mx, my);
      contour.start = cur;
      haveContour = true;
    }

    // The fill and line indices are read and discarded: they index the style
    // arrays, and this decoder produces geometry only. They still have to be
    // consumed or the next record starts at the wrong bit.
    if (stateFillStyle0 || stateFillStyle1) {
      int idx;
      if (!r.bits(numFillBits, idx)) {
        out.error = QObject::tr("truncated fill index at bit %1").arg(r.bitPos());
        return out;
      }
    }
    if (stateLineStyle) {
      int idx;
      if (!r.bits(numLineBits, idx)) {
        out.error = QObject::tr("truncated line index at bit %1").arg(r.bitPos());
        return out;
      }
    }
    if (stateNewStyles) {
      // A new style array, and new bit widths with it.
      int nf = 0, nl = 0, nfb = 0, nlb = 0;
      if (!readStyles(r, version, nf, nl, nfb, nlb)) {
        out.error = QObject::tr("truncated in a mid-shape style array at "
                                "bit %1")
                        .arg(r.bitPos());
        return out;
      }
      numFillBits = nfb;
      numLineBits = nlb;
    }

    ++out.records;
  }

  flush();
  out.ok = true;
  return out;
}

Shape decodeShape(const QByteArray &data, int version) {
  return decodeShape(reinterpret_cast<const unsigned char *>(data.constData()),
                     static_cast<int>(data.size()), version);
}

QString toSvgPath(const Shape &shape) {
  if (!shape.ok || shape.contours.isEmpty()) return QString();

  // The viewBox is the shape's own bounds. EdgeBounds is the outline's extent
  // and ShapeBounds includes the stroke width, so EdgeBounds is the tighter fit
  // where DefineShape4 provides it; for the earlier versions they are one value.
  const QPointF mn = shape.hasEdgeBounds ? shape.edgeMin : shape.boundsMin;
  const QPointF mx = shape.hasEdgeBounds ? shape.edgeMax : shape.boundsMax;

  QStringList d;
  for (const Contour &c : shape.contours) {
    d << QStringLiteral("M") + num(c.start.x()) + QLatin1Char(' ') +
           num(c.start.y());
    for (const Segment &s : c.segments) {
      if (s.curve) {
        d << QStringLiteral("Q") + num(s.control.x()) + QLatin1Char(' ') +
               num(s.control.y()) + QLatin1Char(' ') + num(s.end.x()) +
               QLatin1Char(' ') + num(s.end.y());
      } else {
        d << QStringLiteral("L") + num(s.end.x()) + QLatin1Char(' ') +
               num(s.end.y());
      }
    }
    // Z, not an explicit line back to the start: the format stores no closing
    // segment, so emitting one as a line would make the segment count disagree
    // with the record count that produced it.
    d << QStringLiteral("Z");
  }

  double w = mx.x() - mn.x(), h = mx.y() - mn.y();
  if (w <= 0) w = 1;
  if (h <= 0) h = 1;

  return QStringLiteral("viewBox=\"%1 %2 %3 %4\"")
             .arg(num(mn.x()), num(mn.y()), num(w), num(h)) +
         QStringLiteral(" d=\"") + d.join(QLatin1Char(' ')) +
         QStringLiteral("\"");
}

QString toSvgDocument(const Shape &shape) {
  if (!shape.ok || shape.contours.isEmpty()) return QString();
  const QString path = toSvgPath(shape);
  if (path.isEmpty()) return QString();

  // Stroked, not filled: this decoder reads outlines, not styles, so asserting a
  // fill would claim a paint it never looked at.
  //
  // The d= attribute appears once inside toSvgPath(); recover it for the <path>
  // body rather than printing the viewBox a second time as a path.
  const int dAt = path.indexOf(QLatin1String(" d=\""));
  const QString dAttr = dAt >= 0 ? path.mid(dAt + 4, path.size() - dAt - 5)
                                 : QString();

  return QStringLiteral(
             "<svg xmlns=\"http://www.w3.org/2000/svg\" %1 "
             "fill=\"none\" stroke=\"#000\" stroke-width=\"1\">"
             "<path d=\"%2\"/></svg>")
      .arg(path, dAttr);
}

}  // namespace SWF
